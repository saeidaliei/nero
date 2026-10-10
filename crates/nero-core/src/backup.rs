use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use age::{secrecy::SecretString, Decryptor, Encryptor};
use tempfile::{NamedTempFile, TempDir};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{NeroError, Result, Workspace};

const MANIFEST_NAME: &str = "manifest.json";
const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupManifest {
    pub format: String,
    pub version: u32,
    pub created_at_unix: u64,
    pub files: Vec<BackupEntry>,
    /// Present for complete application-home backups so absolute workspace paths can be rebased on restore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_home: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupStats {
    pub files: usize,
    pub bytes: u64,
}

impl BackupManifest {
    pub fn stats(&self) -> BackupStats {
        BackupStats {
            files: self.files.len(),
            bytes: self.files.iter().map(|entry| entry.size).sum(),
        }
    }
}

impl Workspace {
    /// Create a portable, unencrypted ZIP snapshot of the workspace.
    ///
    /// `.nero` and `.git` are intentionally excluded because they are derived/cache or
    /// version-control internals. Encryption is intentionally a separate layer and will
    /// wrap this archive rather than altering the live workspace.
    pub fn create_backup(&self, destination: impl AsRef<Path>) -> Result<(PathBuf, BackupManifest)> {
        let destination = destination.as_ref();
        let destination = if destination.is_absolute() {
            destination.to_path_buf()
        } else {
            std::env::current_dir()?.join(destination)
        };

        if is_inside_workspace(&destination, &self.root()) {
            return Err(NeroError::Message(
                "backup destination must be outside the workspace to avoid backing up the backup itself".into(),
            ));
        }

        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let temp = NamedTempFile::new_in(parent)?;
        let output = temp.reopen()?;
        let (_output, manifest) = write_backup_archive(self, output)?;
        temp.persist(&destination).map_err(|error| NeroError::Io(error.error))?;
        Ok((destination, manifest))
    }

    pub fn inspect_backup(archive: impl AsRef<Path>) -> Result<BackupManifest> {
        let file = fs::File::open(archive)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup archive: {error}")))?;
        let mut manifest_file = archive
            .by_name(MANIFEST_NAME)
            .map_err(|error| NeroError::Message(format!("backup manifest not found: {error}")))?;
        let mut manifest_json = Vec::new();
        manifest_file.read_to_end(&mut manifest_json)?;
        let manifest = serde_json::from_slice::<BackupManifest>(&manifest_json)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup manifest: {error}")))?;
        if manifest.format != "nero-backup" || manifest.version != MANIFEST_VERSION {
            return Err(NeroError::Message("unsupported Nero backup format".into()));
        }
        Ok(manifest)
    }

    pub fn restore_backup(archive: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<BackupStats> {
        let destination = destination.as_ref();
        if destination.exists() {
            let mut entries = fs::read_dir(destination)?;
            if entries.next().is_some() {
                return Err(NeroError::Message("restore destination must be empty".into()));
            }
        } else {
            fs::create_dir_all(destination)?;
        }

        let file = fs::File::open(archive)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup archive: {error}")))?;
        let mut manifest_file = archive
            .by_name(MANIFEST_NAME)
            .map_err(|error| NeroError::Message(format!("backup manifest not found: {error}")))?;
        let mut manifest_json = Vec::new();
        manifest_file.read_to_end(&mut manifest_json)?;
        drop(manifest_file);
        let manifest = serde_json::from_slice::<BackupManifest>(&manifest_json)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup manifest: {error}")))?;
        if manifest.format != "nero-backup" || manifest.version != MANIFEST_VERSION {
            return Err(NeroError::Message("unsupported Nero backup format".into()));
        }

        for entry in &manifest.files {
            let relative = safe_archive_path(&entry.path)?;
            let target = destination.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut source = archive
                .by_name(&entry.path)
                .map_err(|error| NeroError::Message(format!("backup entry missing: {} ({error})", entry.path)))?;
            let mut output = fs::File::create(&target)?;
            std::io::copy(&mut source, &mut output)?;
        }

        verify_extracted_manifest(&manifest, destination)?;
        Ok(manifest.stats())
    }

    /// Create a passphrase-encrypted snapshot of the complete Nero application home.
    /// The prior `backups/` directory and rebuildable SQLite indexes are omitted to
    /// avoid recursion and stale cache data; private keys and notes remain encrypted.
    pub fn create_home_backup(
        home: impl AsRef<Path>,
        destination: impl AsRef<Path>,
        passphrase: String,
    ) -> Result<(PathBuf, BackupManifest)> {
        if passphrase.chars().count() < 12 {
            return Err(NeroError::Message("home-backup passphrase must be at least 12 characters".into()));
        }
        let home = fs::canonicalize(home.as_ref()).map_err(|error| {
            NeroError::Message(format!("Nero home does not exist: {error}"))
        })?;
        if !home.is_dir() {
            return Err(NeroError::Message(format!("Nero home is not a directory: {}", home.display())));
        }
        // The CLI passes the active home here. Avoid applying process-level path
        // overrides while unit tests use an isolated temporary home directory.
        if let Ok(active_home) = crate::nero_home_dir() {
            if fs::canonicalize(active_home).ok().as_deref() == Some(home.as_path()) {
                ensure_home_overrides_are_contained(&home)?;
            }
        }
        ensure_home_workspaces_are_contained(&home)?;
        let requested_destination = absolute_path(destination.as_ref())?;
        let backup_dir = home.join("backups");
        if requested_destination.starts_with(&home) && !requested_destination.starts_with(&backup_dir) {
            return Err(NeroError::Message(
                "home backup must be written outside Nero home or inside its backups/ directory".into(),
            ));
        }
        let file_name = requested_destination.file_name().ok_or_else(|| {
            NeroError::Message("home backup destination must include a filename".into())
        })?.to_owned();
        let requested_parent = requested_destination.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(requested_parent)?;
        // Resolve the parent after creation so a symlinked output directory cannot
        // bypass the rule that snapshots must not land inside arbitrary home subdirs.
        let parent = fs::canonicalize(requested_parent)?;
        let destination = parent.join(file_name);
        if destination.starts_with(&home) && !destination.starts_with(&backup_dir) {
            return Err(NeroError::Message(
                "home backup must be written outside Nero home or inside its backups/ directory".into(),
            ));
        }
        if destination.exists() {
            return Err(NeroError::Message(format!("backup already exists: {}", destination.display())));
        }
        let temp = NamedTempFile::new_in(&parent)?;
        let output = temp.reopen()?;
        let encryptor = Encryptor::with_user_passphrase(SecretString::from(passphrase));
        let encrypted = encryptor.wrap_output(output)
            .map_err(|error| NeroError::Message(format!("could not initialize encrypted home backup: {error}")))?;
        let (encrypted, manifest) = write_home_backup_archive(&home, encrypted)?;
        encrypted.finish()
            .map_err(|error| NeroError::Message(format!("could not finalize encrypted home backup: {error}")))?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(&destination).map_err(|error| NeroError::Io(error.error))?;
        Ok((destination, manifest))
    }

    /// Decrypt, parse, and hash-check a full-home backup without restoring it.
    pub fn verify_home_backup(archive: impl AsRef<Path>, passphrase: String) -> Result<BackupStats> {
        let plaintext = decrypt_home_archive(archive.as_ref(), passphrase)?;
        verify_home_archive(plaintext.path())
    }

    /// Restore a complete home backup into a new or empty directory.
    /// Config paths that pointed inside the original home are rebased to the restored location.
    pub fn restore_home_backup(
        archive: impl AsRef<Path>,
        destination: impl AsRef<Path>,
        passphrase: String,
    ) -> Result<BackupStats> {
        let plaintext = decrypt_home_archive(archive.as_ref(), passphrase)?;
        restore_home_archive(plaintext.path(), destination.as_ref())
    }

    /// Perform a complete decrypt/restore/verify cycle in a temporary directory.
    pub fn recovery_test_home_backup(archive: impl AsRef<Path>, passphrase: String) -> Result<BackupStats> {
        let plaintext = decrypt_home_archive(archive.as_ref(), passphrase)?;
        let restore = TempDir::new()?;
        restore_home_archive(plaintext.path(), restore.path())
    }

    pub fn verify_backup(archive: impl AsRef<Path>) -> Result<BackupStats> {
        let file = fs::File::open(archive)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup archive: {error}")))?;
        let mut manifest_file = archive
            .by_name(MANIFEST_NAME)
            .map_err(|error| NeroError::Message(format!("backup manifest not found: {error}")))?;
        let mut manifest_json = Vec::new();
        manifest_file.read_to_end(&mut manifest_json)?;
        drop(manifest_file);
        let manifest = serde_json::from_slice::<BackupManifest>(&manifest_json)
            .map_err(|error| NeroError::Message(format!("invalid Nero backup manifest: {error}")))?;

        if manifest.format != "nero-backup" || manifest.version != MANIFEST_VERSION {
            return Err(NeroError::Message("unsupported Nero backup format".into()));
        }

        for entry in &manifest.files {
            let mut file = archive
                .by_name(&entry.path)
                .map_err(|error| NeroError::Message(format!("backup entry missing: {} ({error})", entry.path)))?;
            let mut hasher = Sha256::new();
            let mut size = 0u64;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 { break; }
                size += read as u64;
                hasher.update(&buffer[..read]);
            }
            let hash = hex_digest(hasher.finalize());
            if size != entry.size || hash != entry.sha256 {
                return Err(NeroError::Message(format!("backup verification failed for {}", entry.path)));
            }
        }

        Ok(manifest.stats())
    }
}

pub(crate) fn write_backup_archive<W: Write>(workspace: &Workspace, output: W) -> Result<(W, BackupManifest)> {
    let entries = collect_backup_entries(workspace.root())?;
    let manifest = BackupManifest {
        format: "nero-backup".into(),
        version: MANIFEST_VERSION,
        created_at_unix: unix_now(),
        files: entries.clone(),
        source_home: None,
    };

    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| NeroError::Message(format!("could not serialize backup manifest: {error}")))?;

    // Encrypted backups wrap an age stream, which implements `Write` but not `Seek`.
    // ZIP's stream mode writes data descriptors instead of seeking back to patch headers.
    let mut zip = ZipWriter::new_stream(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file(MANIFEST_NAME, options)
        .map_err(|error| NeroError::Message(format!("could not create backup manifest: {error}")))?;
    zip.write_all(&manifest_json)?;

    for entry in entries {
        let absolute = workspace.root().join(&entry.path);
        zip.start_file(&entry.path, options)
            .map_err(|error| NeroError::Message(format!("could not add {} to backup: {error}", entry.path)))?;
        let mut file = fs::File::open(&absolute)?;
        std::io::copy(&mut file, &mut zip)?;
    }

    let output = zip
        .finish()
        .map_err(|error| NeroError::Message(format!("could not finalize backup archive: {error}")))?
        .into_inner();

    Ok((output, manifest))
}

fn write_home_backup_archive<W: Write>(home: &Path, output: W) -> Result<(W, BackupManifest)> {
    let entries = collect_home_backup_entries(home)?;
    let manifest = BackupManifest {
        format: "nero-home-backup".into(),
        version: MANIFEST_VERSION,
        created_at_unix: unix_now(),
        files: entries.clone(),
        source_home: Some(home.to_string_lossy().into_owned()),
    };
    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| NeroError::Message(format!("could not serialize home backup manifest: {error}")))?;

    // ZIP stream mode works with the age output stream without requiring Seek.
    let mut zip = ZipWriter::new_stream(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(MANIFEST_NAME, options)
        .map_err(|error| NeroError::Message(format!("could not create home manifest: {error}")))?;
    zip.write_all(&manifest_json)?;
    for entry in entries {
        zip.start_file(&entry.path, options)
            .map_err(|error| NeroError::Message(format!("could not add {} to home backup: {error}", entry.path)))?;
        let mut file = fs::File::open(home.join(&entry.path))?;
        std::io::copy(&mut file, &mut zip)?;
    }
    let output = zip.finish()
        .map_err(|error| NeroError::Message(format!("could not finalize home archive: {error}")))?
        .into_inner();
    Ok((output, manifest))
}

fn collect_home_backup_entries(home: &Path) -> Result<Vec<BackupEntry>> {
    let mut files = Vec::new();
    collect_home_files(home, home, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn collect_home_files(home: &Path, dir: &Path, out: &mut Vec<BackupEntry>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let kind = fs::symlink_metadata(&path)?.file_type();
        if kind.is_symlink() {
            return Err(NeroError::Message(format!("symlinks are not supported in home backups: {}", path.display())));
        }
        let relative = path.strip_prefix(home).map_err(|_| NeroError::Message("home backup path escaped its root".into()))?;
        if relative == Path::new("backups") {
            // Avoid placing old snapshots inside each new full-home snapshot.
            continue;
        }
        if kind.is_dir() {
            collect_home_files(home, &path, out)?;
            continue;
        }
        if !kind.is_file() || is_disposable_index_path(relative) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        out.push(BackupEntry {
            path: relative.to_string_lossy().replace('\\', "/"),
            size: metadata.len(),
            sha256: sha256_file(&path)?,
        });
    }
    Ok(())
}

fn is_disposable_index_path(path: &Path) -> bool {
    let file = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    let in_nero_dir = path.parent().and_then(Path::file_name).is_some_and(|name| name == ".nero");
    in_nero_dir && matches!(file, "index.sqlite" | "index.sqlite-wal" | "index.sqlite-shm")
}

fn verify_home_archive(archive_path: &Path) -> Result<BackupStats> {
    let manifest = read_home_manifest(archive_path)?;
    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| NeroError::Message(format!("invalid Nero home backup archive: {error}")))?;
    for entry in &manifest.files {
        safe_archive_path(&entry.path)?;
        let mut file = archive.by_name(&entry.path)
            .map_err(|error| NeroError::Message(format!("home backup entry missing: {} ({error})", entry.path)))?;
        let (size, digest) = hash_reader(&mut file)?;
        if size != entry.size || digest != entry.sha256 {
            return Err(NeroError::Message(format!("home backup verification failed for {}", entry.path)));
        }
    }
    Ok(manifest.stats())
}

fn restore_home_archive(archive_path: &Path, destination: &Path) -> Result<BackupStats> {
    let stats = verify_home_archive(archive_path)?;
    let manifest = read_home_manifest(archive_path)?;
    let destination = absolute_path(destination)?;
    let existed_before = destination.exists();
    if existed_before {
        let metadata = fs::symlink_metadata(&destination)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() || fs::read_dir(&destination)?.next().is_some() {
            return Err(NeroError::Message(format!("home restore destination must be a new or empty, non-symlink directory: {}", destination.display())));
        }
    } else {
        fs::create_dir_all(&destination)?;
    }
    let destination = fs::canonicalize(&destination)?;
    let result = (|| -> Result<()> {
        let file = fs::File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| NeroError::Message(format!("invalid Nero home backup archive: {error}")))?;
        for entry in &manifest.files {
            let relative = safe_archive_path(&entry.path)?;
            let target = destination.join(relative);
            if let Some(parent) = target.parent() { fs::create_dir_all(parent)?; }
            let mut input = archive.by_name(&entry.path)
                .map_err(|error| NeroError::Message(format!("home backup entry missing: {} ({error})", entry.path)))?;
            let mut output = fs::File::create(&target)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
        }
        verify_extracted_manifest(&manifest, &destination)?;
        if let Some(source_home) = manifest.source_home.as_deref() {
            rebase_home_config(&destination, source_home)?;
        }
        secure_restored_home(&destination)?;
        Ok(())
    })();
    if let Err(error) = result {
        if !existed_before {
            let _ = fs::remove_dir_all(&destination);
        } else {
            // The directory was empty before restore; remove only partial files,
            // preserving the user-created destination directory itself.
            if let Ok(entries) = fs::read_dir(&destination) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() { let _ = fs::remove_dir_all(path); }
                    else { let _ = fs::remove_file(path); }
                }
            }
        }
        return Err(error);
    }
    Ok(stats)
}

fn read_home_manifest(archive_path: &Path) -> Result<BackupManifest> {
    let file = fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| NeroError::Message(format!("invalid Nero home backup archive: {error}")))?;
    let mut manifest_file = archive.by_name(MANIFEST_NAME)
        .map_err(|error| NeroError::Message(format!("home backup manifest not found: {error}")))?;
    let mut bytes = Vec::new();
    manifest_file.read_to_end(&mut bytes)?;
    let manifest: BackupManifest = serde_json::from_slice(&bytes)
        .map_err(|error| NeroError::Message(format!("invalid Nero home backup manifest: {error}")))?;
    if manifest.format != "nero-home-backup" || manifest.version != MANIFEST_VERSION || manifest.source_home.is_none() {
        return Err(NeroError::Message("unsupported Nero home backup format".into()));
    }
    for entry in &manifest.files { safe_archive_path(&entry.path)?; }
    Ok(manifest)
}

fn decrypt_home_archive(archive: &Path, passphrase: String) -> Result<NamedTempFile> {
    let decryptor = Decryptor::new(fs::File::open(archive)?)
        .map_err(|error| NeroError::Message(format!("invalid encrypted Nero home backup: {error}")))?;
    if !decryptor.is_scrypt() {
        return Err(NeroError::Message("home backups use a passphrase; this archive is not passphrase-encrypted".into()));
    }
    let identity = age::scrypt::Identity::new(SecretString::from(passphrase));
    let mut reader = decryptor.decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|error| NeroError::Message(format!("could not decrypt Nero home backup: {error}")))?;
    let mut temp = NamedTempFile::new()?;
    std::io::copy(&mut reader, temp.as_file_mut())?;
    temp.as_file().sync_all()?;
    Ok(temp)
}

fn hash_reader(reader: &mut impl Read) -> Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 { break; }
        size += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok((size, hex_digest(hasher.finalize())))
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() { Ok(path.to_path_buf()) } else { Ok(std::env::current_dir()?.join(path)) }
}

/// A full-home backup must include the effective config and identity. Advanced
/// overrides outside NERO_HOME would otherwise make the archive incomplete.
fn ensure_home_overrides_are_contained(home: &Path) -> Result<()> {
    for (variable, expected) in [
        ("NERO_CONFIG_HOME", home.to_path_buf()),
        ("NERO_CONFIG_DIR", home.join("keys")),
    ] {
        let Some(value) = std::env::var_os(variable) else { continue; };
        let configured = absolute_path(Path::new(&value))?;
        let configured = fs::canonicalize(&configured).unwrap_or(configured);
        if configured != expected {
            return Err(NeroError::Message(format!(
                "cannot create a self-contained home backup while {variable} relocates data from its standard location ({}); unset the override or set it to {}",
                configured.display(), expected.display()
            )));
        }
    }
    Ok(())
}

/// Refuse a “full-home” archive when the config points to workspaces outside
/// the home directory; those note files cannot be included in this snapshot.
fn ensure_home_workspaces_are_contained(home: &Path) -> Result<()> {
    let config_path = home.join("config.json");
    if !config_path.is_file() {
        return Ok(());
    }
    let bytes = fs::read(&config_path)?;
    let config: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| NeroError::Message(format!("Nero config is invalid: {error}")))?;

    for key in ["workspace_home_path", "default_workspace_path"] {
        if let Some(path) = config.get(key).and_then(serde_json::Value::as_str) {
            ensure_workspace_path_is_contained(home, key, path)?;
        }
    }
    if let Some(workspaces) = config.get("workspaces").and_then(serde_json::Value::as_object) {
        for (name, value) in workspaces {
            if let Some(path) = value.as_str() {
                ensure_workspace_path_is_contained(home, name, path)?;
            }
        }
    }
    Ok(())
}

fn ensure_workspace_path_is_contained(home: &Path, name: &str, raw_path: &str) -> Result<()> {
    let requested = absolute_path(Path::new(raw_path))?;
    let resolved = fs::canonicalize(&requested).unwrap_or(requested);
    if !resolved.starts_with(home) {
        return Err(NeroError::Message(format!(
            "cannot create a self-contained home backup because workspace `{name}` is outside NERO_HOME ({}); move it under `$NERO_HOME/workspaces` or back it up separately with `nero backup create`",
            resolved.display()
        )));
    }
    Ok(())
}

fn rebase_home_config(destination: &Path, source_home: &str) -> Result<()> {
    let config_path = destination.join("config.json");
    if !config_path.is_file() { return Ok(()); }
    let bytes = fs::read(&config_path)?;
    let mut config: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| NeroError::Message(format!("restored Nero config is invalid: {error}")))?;
    let destination_text = destination.to_string_lossy().into_owned();
    for key in ["workspace_home_path", "default_workspace_path"] {
        if let Some(value) = config.get_mut(key) { rebase_json_path(value, source_home, &destination_text); }
    }
    if let Some(workspaces) = config.get_mut("workspaces").and_then(serde_json::Value::as_object_mut) {
        for value in workspaces.values_mut() { rebase_json_path(value, source_home, &destination_text); }
    }
    let rewritten = serde_json::to_vec_pretty(&config)
        .map_err(|error| NeroError::Message(format!("could not update restored Nero config: {error}")))?;
    atomic_write_file(&config_path, &rewritten)?;
    Ok(())
}

fn rebase_json_path(value: &mut serde_json::Value, source_home: &str, destination_home: &str) {
    let Some(stored) = value.as_str() else { return; };
    let source = source_home.trim_end_matches(|ch| ch == '/' || ch == '\\');
    if stored == source {
        let rebased = destination_home.to_owned();
        *value = serde_json::Value::String(rebased);
        return;
    }
    let Some(suffix) = stored.strip_prefix(source) else { return; };
    if !(suffix.starts_with('/') || suffix.starts_with('\\')) { return; }
    let suffix = suffix.trim_start_matches(|ch| ch == '/' || ch == '\\')
        .replace('\\', std::path::MAIN_SEPARATOR_STR)
        .replace('/', std::path::MAIN_SEPARATOR_STR);
    let rebased = Path::new(destination_home).join(suffix).to_string_lossy().into_owned();
    *value = serde_json::Value::String(rebased);
}

fn atomic_write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| NeroError::Io(error.error))?;
    Ok(())
}

#[cfg(unix)]
fn secure_restored_home(home: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(home, fs::Permissions::from_mode(0o700))?;
    let keys = home.join("keys");
    if keys.is_dir() {
        fs::set_permissions(&keys, fs::Permissions::from_mode(0o700))?;
        let identity = keys.join("identity.txt");
        if identity.is_file() {
            fs::set_permissions(identity, fs::Permissions::from_mode(0o600))?;
        }
        let config = keys.join("recipient.txt");
        if config.is_file() {
            fs::set_permissions(config, fs::Permissions::from_mode(0o644))?;
        }
    }
    let config = home.join("config.json");
    if config.is_file() {
        fs::set_permissions(config, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn secure_restored_home(_home: &Path) -> Result<()> { Ok(()) }

fn collect_backup_entries(root: &Path) -> Result<Vec<BackupEntry>> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<BackupEntry>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        let file_type = fs::symlink_metadata(&path)?.file_type();
        if file_type.is_symlink() {
            return Err(NeroError::Message(format!("symlinks are not supported in backups: {}", path.display())));
        }
        if file_type.is_dir() {
            if name == ".nero" || name == ".git" { continue; }
            collect_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() { continue; }

        let metadata = fs::symlink_metadata(&path)?;
        let hash = sha256_file(&path)?;
        let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        out.push(BackupEntry { path: relative, size: metadata.len(), sha256: hash });
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 { break; }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize()))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    let mut result = String::with_capacity(bytes.as_ref().len() * 2);
    for byte in bytes.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

fn is_inside_workspace(candidate: &Path, workspace: &Path) -> bool {
    let candidate = candidate.canonicalize().unwrap_or_else(|_| candidate.to_path_buf());
    candidate.starts_with(workspace)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
fn safe_archive_path(path: &str) -> Result<PathBuf> {
    let raw = Path::new(path);
    if raw.is_absolute() {
        return Err(NeroError::Message(format!("unsafe absolute path in backup: {path}")));
    }
    let mut clean = PathBuf::new();
    for component in raw.components() {
        use std::path::Component;
        match component {
            Component::Normal(value) => clean.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(NeroError::Message(format!("unsafe path in backup: {path}")));
            }
        }
    }
    Ok(clean)
}

fn verify_extracted_manifest(manifest: &BackupManifest, destination: &Path) -> Result<()> {
    for entry in &manifest.files {
        let relative = safe_archive_path(&entry.path)?;
        let path = destination.join(relative);
        let metadata = fs::metadata(&path)?;
        let digest = sha256_file(&path)?;
        if metadata.len() != entry.size || digest != entry.sha256 {
            return Err(NeroError::Message(format!("restored file failed verification: {}", entry.path)));
        }
    }
    Ok(())
}


#[cfg(test)]
mod home_backup_tests {
    use super::*;
    use std::fs;

    fn temporary(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("nero-home-backup-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn encrypted_home_backup_restores_config_keys_and_workspaces_portably() {
        let source = temporary("source");
        let workspace = source.join("workspaces/personal");
        fs::create_dir_all(workspace.join(".nero")).unwrap();
        fs::create_dir_all(source.join("keys")).unwrap();
        fs::create_dir_all(source.join("backups")).unwrap();
        fs::write(workspace.join("note.md"), "# Portable note\n").unwrap();
        fs::write(workspace.join(".nero/storage.json"), r#"{"format":"nero-storage","version":1,"remotes":[]}"#).unwrap();
        fs::write(workspace.join(".nero/index.sqlite"), b"disposable sqlite index").unwrap();
        fs::write(source.join("keys/identity.txt"), "AGE-SECRET-KEY-TEST\n").unwrap();
        fs::write(source.join("keys/recipient.txt"), "age1example\n").unwrap();
        fs::write(source.join("backups/older.age"), b"must be excluded to avoid recursive snapshots").unwrap();
        let config = serde_json::json!({
            "format": "nero-config", "version": 1,
            "workspace_home_path": source.join("workspaces").to_string_lossy(),
            "default_workspace_path": workspace.to_string_lossy(),
            "default_workspace_name": "personal",
            "workspaces": {"personal": workspace.to_string_lossy()}
        });
        fs::write(source.join("config.json"), serde_json::to_vec_pretty(&config).unwrap()).unwrap();

        let archive = temporary("external").with_extension("zip.age");
        let passphrase = "a strong home backup phrase".to_owned();
        let (_created_path, manifest) = Workspace::create_home_backup(&source, &archive, passphrase.clone()).unwrap();
        assert_eq!(manifest.format, "nero-home-backup");
        assert!(manifest.files.iter().any(|entry| entry.path == "workspaces/personal/note.md"));
        assert!(manifest.files.iter().any(|entry| entry.path == "workspaces/personal/.nero/storage.json"));
        assert!(manifest.files.iter().any(|entry| entry.path == "keys/identity.txt"));
        assert!(!manifest.files.iter().any(|entry| entry.path.starts_with("backups/")));
        assert!(!manifest.files.iter().any(|entry| entry.path.ends_with("index.sqlite")));
        assert_eq!(Workspace::verify_home_backup(&archive, passphrase.clone()).unwrap().files, manifest.files.len());

        let restored = temporary("restored");
        let stats = Workspace::restore_home_backup(&archive, &restored, passphrase).unwrap();
        assert_eq!(stats.files, manifest.files.len());
        assert_eq!(fs::read_to_string(restored.join("workspaces/personal/note.md")).unwrap(), "# Portable note\n");
        assert!(restored.join("keys/identity.txt").is_file());
        assert!(restored.join("workspaces/personal/.nero/storage.json").is_file());
        assert!(!restored.join("workspaces/personal/.nero/index.sqlite").exists());
        assert!(!restored.join("backups").exists());
        let restored_config: serde_json::Value = serde_json::from_slice(&fs::read(restored.join("config.json")).unwrap()).unwrap();
        let restored_workspace_path = restored.join("workspaces/personal").to_string_lossy().into_owned();
        assert_eq!(restored_config["workspaces"]["personal"].as_str(), Some(restored_workspace_path.as_str()));

        let _ = fs::remove_file(archive);
        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(restored);
    }

    #[test]
    fn full_home_backup_refuses_registered_workspaces_outside_home() {
        let source = temporary("external-workspace-source");
        let external = temporary("external-workspace");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&external).unwrap();
        fs::write(external.join("note.md"), "outside the profile\n").unwrap();
        let config = serde_json::json!({
            "format": "nero-config", "version": 1,
            "workspace_home_path": source.join("workspaces").to_string_lossy().into_owned(),
            "default_workspace_path": external.to_string_lossy().into_owned(),
            "default_workspace_name": "external",
            "workspaces": {"external": external.to_string_lossy().into_owned()}
        });
        fs::write(source.join("config.json"), serde_json::to_vec_pretty(&config).unwrap()).unwrap();
        let archive = temporary("external-workspace-out").with_extension("zip.age");
        let error = Workspace::create_home_backup(&source, &archive, "long enough backup phrase".into()).unwrap_err();
        assert!(error.to_string().contains("outside NERO_HOME"));
        assert!(!archive.exists());
        let _ = fs::remove_dir_all(source);
        let _ = fs::remove_dir_all(external);
    }

}
