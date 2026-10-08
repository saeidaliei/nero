use std::{env, fs, io::Write, path::{Path, PathBuf}, str::FromStr};

use age::{secrecy::ExposeSecret, Decryptor, Encryptor};

use crate::{backup, atomic_write, BackupManifest, BackupStats, NeroError, Result, Workspace};
use tempfile::{NamedTempFile, TempDir};

const CONFIG_ENV: &str = "NERO_CONFIG_DIR";
const IDENTITY_FILE: &str = "identity.txt";
const RECIPIENT_FILE: &str = "recipient.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupKeyInfo {
    pub identity_path: PathBuf,
    pub recipient: String,
}

impl Workspace {
    /// Returns Nero's user-level configuration directory.
    /// The private age identity is intentionally stored outside the workspace by default.
    pub fn config_dir() -> Result<PathBuf> {
        default_config_dir()
    }

    pub fn identity_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join(IDENTITY_FILE))
    }

    pub fn recipient_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join(RECIPIENT_FILE))
    }

    /// Generate the user's default age X25519 identity and store it outside the workspace.
    pub fn generate_backup_key(force: bool) -> Result<BackupKeyInfo> {
        let config = Self::config_dir()?;
        fs::create_dir_all(&config)?;
        let identity_path = config.join(IDENTITY_FILE);
        let recipient_path = config.join(RECIPIENT_FILE);

        if identity_path.exists() && !force {
            return Err(NeroError::Message(format!(
                "backup identity already exists at {}; use `nero key generate --force` to replace it",
                identity_path.display()
            )));
        }

        let identity = age::x25519::Identity::generate();
        let identity_text = identity.to_string();
        let secret = identity_text.expose_secret().to_owned();
        let recipient = identity.to_public().to_string();

        atomic_write(&identity_path, format!("{secret}\n").as_bytes())?;
        atomic_write(&recipient_path, format!("{recipient}\n").as_bytes())?;
        set_private_file_permissions(&identity_path)?;

        Ok(BackupKeyInfo { identity_path, recipient })
    }

    pub fn backup_key_info() -> Result<Option<BackupKeyInfo>> {
        let identity_path = Self::identity_path()?;
        if !identity_path.is_file() {
            return Ok(None);
        }
        let identity = load_identity(&identity_path)?;
        Ok(Some(BackupKeyInfo {
            identity_path,
            recipient: identity.to_public().to_string(),
        }))
    }

    /// Create an age-encrypted version of the standard Nero ZIP backup.
    /// The ZIP is streamed through age, so the full backup is not loaded into memory.
    pub fn create_encrypted_backup(&self, destination: impl AsRef<Path>) -> Result<(PathBuf, BackupManifest)> {
        self.create_encrypted_backup_with_identity(destination, Self::identity_path()?)
    }

    pub fn create_encrypted_backup_with_identity(
        &self,
        destination: impl AsRef<Path>,
        identity_path: impl AsRef<Path>,
    ) -> Result<(PathBuf, BackupManifest)> {
        let destination = normalize_destination(destination.as_ref());
        if backup_destination_inside_workspace(&destination, self.root()) {
            return Err(NeroError::Message(
                "backup destination must be outside the workspace to avoid backing up the backup itself".into(),
            ));
        }
        if let Some(parent) = destination.parent() { fs::create_dir_all(parent)?; }

        let identity = load_identity(identity_path.as_ref())?;
        let recipient = identity.to_public();
        let encryptor = Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|error| NeroError::Message(format!("could not create age encryptor: {error}")))?;

        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        let temp = NamedTempFile::new_in(parent)?;
        let output = temp.reopen()?;
        let encrypted = encryptor
            .wrap_output(output)
            .map_err(|error| NeroError::Message(format!("could not initialize encrypted backup: {error}")))?;
        let (encrypted, manifest) = backup::write_backup_archive(self, encrypted)?;
        encrypted
            .finish()
            .map_err(|error| NeroError::Message(format!("could not finalize encrypted backup: {error}")))?;
        temp.persist(&destination).map_err(|error| NeroError::Io(error.error))?;

        Ok((destination, manifest))
    }

    pub fn verify_encrypted_backup(
        archive: impl AsRef<Path>,
        identity_path: impl AsRef<Path>,
    ) -> Result<BackupStats> {
        let temp = NamedTempFile::new()?;
        decrypt_age_to_file(archive.as_ref(), identity_path.as_ref(), temp.path())?;
        Self::verify_backup(temp.path())
    }

    pub fn restore_encrypted_backup(
        archive: impl AsRef<Path>,
        destination: impl AsRef<Path>,
        identity_path: impl AsRef<Path>,
    ) -> Result<BackupStats> {
        let temp = NamedTempFile::new()?;
        decrypt_age_to_file(archive.as_ref(), identity_path.as_ref(), temp.path())?;
        Self::restore_backup(temp.path(), destination)
    }

    pub fn backup_is_encrypted(archive: impl AsRef<Path>) -> Result<bool> {
        is_age_backup(archive.as_ref())
    }

    pub fn recovery_test_backup(
        archive: impl AsRef<Path>,
        identity_path: Option<PathBuf>,
    ) -> Result<BackupStats> {
        let archive = archive.as_ref();
        let is_encrypted = is_age_backup(archive)?;
        let restore_dir = TempDir::new()?;

        if is_encrypted {
            let identity_path = identity_path.unwrap_or(Self::identity_path()?);
            Self::restore_encrypted_backup(archive, restore_dir.path(), identity_path)
        } else {
            Self::restore_backup(archive, restore_dir.path())
        }
    }
}

fn load_identity(path: &Path) -> Result<age::x25519::Identity> {
    let contents = fs::read_to_string(path).map_err(|error| {
        NeroError::Message(format!("could not read backup identity {}: {error}", path.display()))
    })?;
    age::x25519::Identity::from_str(contents.trim())
        .map_err(|error| NeroError::Message(format!("invalid age identity {}: {error}", path.display())))
}

fn decrypt_age_to_file(archive: &Path, identity_path: &Path, destination: &Path) -> Result<()> {
    let identity = load_identity(identity_path)?;
    let input = fs::File::open(archive)?;
    let decryptor = Decryptor::new(input)
        .map_err(|error| NeroError::Message(format!("invalid encrypted Nero backup: {error}")))?;
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|error| NeroError::Message(format!("could not decrypt Nero backup: {error}")))?;
    let mut output = fs::OpenOptions::new().write(true).truncate(true).open(destination)?;
    std::io::copy(&mut reader, &mut output)?;
    output.flush()?;
    output.sync_all()?;
    Ok(())
}

fn backup_destination_inside_workspace(destination: &Path, workspace: &Path) -> bool {
    let destination = destination.canonicalize().unwrap_or_else(|_| destination.to_path_buf());
    destination.starts_with(workspace)
}

fn default_config_dir() -> Result<PathBuf> {
    if let Some(path) = env::var_os(CONFIG_ENV) {
        return Ok(PathBuf::from(path));
    }

    #[cfg(windows)]
    {
        let base = env::var_os("APPDATA")
            .ok_or_else(|| NeroError::Message("APPDATA is not set; set NERO_CONFIG_DIR to choose Nero's key location".into()))?;
        return Ok(PathBuf::from(base).join("nero"));
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME")
            .ok_or_else(|| NeroError::Message("HOME is not set; set NERO_CONFIG_DIR to choose Nero's key location".into()))?;
        return Ok(PathBuf::from(home).join("Library").join("Application Support").join("Nero"));
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(path).join("nero"));
        }
        let home = env::var_os("HOME")
            .ok_or_else(|| NeroError::Message("HOME is not set; set NERO_CONFIG_DIR to choose Nero's key location".into()))?;
        Ok(PathBuf::from(home).join(".config").join("nero"))
    }
}

fn normalize_destination(destination: &Path) -> PathBuf {
    if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        env::current_dir().map(|cwd| cwd.join(destination)).unwrap_or_else(|_| destination.to_path_buf())
    }
}

fn is_age_backup(path: &Path) -> Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut header = [0u8; 32];
    let read = std::io::Read::read(&mut file, &mut header)?;
    Ok(header[..read].starts_with(b"age-encryption.org/v1\n"))
}

fn set_private_file_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("nero-encryption-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn encrypted_backup_round_trips_with_age_identity() {
        let root = temp_path("workspace");
        fs::create_dir_all(&root).unwrap();
        let workspace = Workspace::init(&root).unwrap();
        workspace.create_note("Secret Note").unwrap();
        fs::write(root.join("secret.md"), "# Secret\n\nDo not leak this.\n").unwrap();

        let key_dir = temp_path("keys");
        fs::create_dir_all(&key_dir).unwrap();
        let identity_path = key_dir.join("identity.txt");
        let identity = age::x25519::Identity::generate();
        fs::write(&identity_path, format!("{}\n", identity.to_string().expose_secret())).unwrap();

        let encrypted = root.parent().unwrap().join("nero-test.age");
        let (path, manifest) = workspace
            .create_encrypted_backup_with_identity(&encrypted, &identity_path)
            .unwrap();
        assert_eq!(path, encrypted);
        assert!(manifest.files.iter().any(|entry| entry.path == "secret.md"));

        let stats = Workspace::verify_encrypted_backup(&encrypted, &identity_path).unwrap();
        assert_eq!(stats.files, manifest.files.len());

        let restored = temp_path("restored");
        let restored_stats = Workspace::restore_encrypted_backup(&encrypted, &restored, &identity_path).unwrap();
        assert_eq!(restored_stats, stats);
        assert_eq!(fs::read_to_string(restored.join("secret.md")).unwrap(), "# Secret\n\nDo not leak this.\n");

        let _ = fs::remove_file(encrypted);
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(key_dir);
        let _ = fs::remove_dir_all(restored);
    }
}
