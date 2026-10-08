use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};

use crate::{atomic_write, NeroError, Result, Workspace};

const STORAGE_CONFIG_VERSION: u32 = 1;
const STORAGE_CONFIG_NAME: &str = "storage.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageRemote {
    pub name: String,
    pub target: String,
    #[serde(default)]
    pub encrypt_backups: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StorageConfig {
    format: String,
    version: u32,
    remotes: Vec<StorageRemote>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            format: "nero-storage".into(),
            version: STORAGE_CONFIG_VERSION,
            remotes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteBackup {
    pub name: String,
    pub encrypted: bool,
}

impl Workspace {
    pub fn storage_config_path(&self) -> PathBuf {
        self.metadata_dir().join(STORAGE_CONFIG_NAME)
    }

    pub fn storage_remotes(&self) -> Result<Vec<StorageRemote>> {
        let config = self.load_storage_config()?;
        Ok(config.remotes)
    }

    pub fn storage_get(&self, name: &str) -> Result<StorageRemote> {
        let config = self.load_storage_config()?;
        config
            .remotes
            .into_iter()
            .find(|remote| remote.name == name)
            .ok_or_else(|| NeroError::Message(format!("storage remote not found: {name}")))
    }

    pub fn storage_add(&self, name: &str, target: &str, encrypt_backups: bool) -> Result<StorageRemote> {
        let name = name.trim();
        let target = target.trim();
        if !valid_storage_name(name) {
            return Err(NeroError::Message(
                "storage name must contain only letters, numbers, '.', '_' or '-'".into(),
            ));
        }
        if target.is_empty() || target.contains('\0') {
            return Err(NeroError::Message("storage target cannot be empty".into()));
        }

        fs::create_dir_all(self.metadata_dir())?;
        let mut config = self.load_storage_config()?;
        if config.remotes.iter().any(|remote| remote.name == name) {
            return Err(NeroError::Message(format!("storage remote already exists: {name}")));
        }

        let remote = StorageRemote {
            name: name.to_owned(),
            target: target.to_owned(),
            encrypt_backups,
        };
        config.remotes.push(remote.clone());
        config.remotes.sort_by(|a, b| a.name.cmp(&b.name));
        self.save_storage_config(&config)?;
        Ok(remote)
    }

    pub fn storage_remove(&self, name: &str) -> Result<StorageRemote> {
        let mut config = self.load_storage_config()?;
        let position = config
            .remotes
            .iter()
            .position(|remote| remote.name == name)
            .ok_or_else(|| NeroError::Message(format!("storage remote not found: {name}")))?;
        let removed = config.remotes.remove(position);
        self.save_storage_config(&config)?;
        Ok(removed)
    }

    /// Verify that rclone is installed and that the configured target can be listed.
    pub fn storage_test(&self, name: &str) -> Result<String> {
        let remote = self.storage_get(name)?;
        ensure_rclone()?;
        let output = run_rclone(&["lsf", &remote.target, "--files-only", "--max-depth", "1"])?;
        Ok(output)
    }

    pub fn storage_list_backups(&self, name: &str) -> Result<Vec<RemoteBackup>> {
        let remote = self.storage_get(name)?;
        ensure_rclone()?;
        let output = run_rclone(&["lsf", &remote.target, "--files-only", "--max-depth", "1"])?;
        let mut backups = output
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .filter(|name| {
                let lower = name.to_ascii_lowercase();
                lower.ends_with(".zip") || lower.ends_with(".age")
            })
            .map(|name| RemoteBackup {
                name: name.to_owned(),
                encrypted: name.to_ascii_lowercase().ends_with(".age"),
            })
            .collect::<Vec<_>>();
        backups.sort_by(|a, b| b.name.cmp(&a.name));
        Ok(backups)
    }

    pub fn storage_upload_file(&self, storage_name: &str, local_file: impl AsRef<Path>) -> Result<String> {
        let remote = self.storage_get(storage_name)?;
        let local_file = local_file.as_ref();
        if !local_file.is_file() {
            return Err(NeroError::Message(format!("backup file does not exist: {}", local_file.display())));
        }
        let file_name = local_file
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| NeroError::Message("backup filename is not valid UTF-8".into()))?;
        let source = local_file.to_string_lossy().into_owned();
        let destination = join_remote_path(&remote.target, file_name);
        ensure_rclone()?;
        run_rclone(&["copyto", &source, &destination, "--checksum", "--immutable"])?;
        Ok(destination)
    }

    pub fn storage_download_file(
        &self,
        storage_name: &str,
        remote_name: &str,
        destination: impl AsRef<Path>,
    ) -> Result<PathBuf> {
        let remote = self.storage_get(storage_name)?;
        validate_remote_name(remote_name)?;
        let destination = normalize_path(destination.as_ref());
        if destination.exists() {
            return Err(NeroError::Message(format!("destination already exists: {}", destination.display())));
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        let source = join_remote_path(&remote.target, remote_name);
        let destination_string = destination.to_string_lossy().into_owned();
        ensure_rclone()?;
        run_rclone(&["copyto", &source, &destination_string, "--checksum"])?;
        Ok(destination)
    }

    fn load_storage_config(&self) -> Result<StorageConfig> {
        let path = self.storage_config_path();
        if !path.exists() {
            return Ok(StorageConfig::default());
        }
        let contents = fs::read_to_string(&path)?;
        let config = serde_json::from_str::<StorageConfig>(&contents)
            .map_err(|error| NeroError::Message(format!("invalid Nero storage configuration: {error}")))?;
        if config.format != "nero-storage" || config.version != STORAGE_CONFIG_VERSION {
            return Err(NeroError::Message("unsupported Nero storage configuration".into()));
        }
        Ok(config)
    }

    fn save_storage_config(&self, config: &StorageConfig) -> Result<()> {
        let path = self.storage_config_path();
        let encoded = serde_json::to_vec_pretty(config)
            .map_err(|error| NeroError::Message(format!("could not serialize storage configuration: {error}")))?;
        atomic_write(&path, &encoded)
    }
}

fn ensure_rclone() -> Result<()> {
    let status = Command::new("rclone")
        .arg("version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                NeroError::Message("rclone is not installed or not on PATH; install rclone and run `rclone config` first".into())
            } else {
                NeroError::Message(format!("could not launch rclone: {error}"))
            }
        })?;
    if !status.success() {
        return Err(NeroError::Message("rclone version check failed".into()));
    }
    Ok(())
}

fn run_rclone(args: &[&str]) -> Result<String> {
    let output = Command::new("rclone")
        .args(args)
        .output()
        .map_err(|error| NeroError::Message(format!("could not launch rclone: {error}")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let detail = if stderr.is_empty() {
        format!("rclone exited with {}", output.status)
    } else {
        stderr
    };
    Err(NeroError::Message(format!("rclone: {detail}")))
}

fn valid_storage_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

fn validate_remote_name(name: &str) -> Result<()> {
    if name.trim().is_empty()
        || name.contains('\0')
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
    {
        return Err(NeroError::Message("remote backup name must be a single filename".into()));
    }
    Ok(())
}

fn join_remote_path(target: &str, name: &str) -> String {
    format!("{}/{}", target.trim_end_matches('/'), name.trim_start_matches('/'))
}

fn normalize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_storage_names() {
        assert!(valid_storage_name("mega"));
        assert!(valid_storage_name("my-backups_01"));
        assert!(!valid_storage_name("my backups"));
        assert!(!valid_storage_name("../remote"));
    }

    #[test]
    fn joins_rclone_paths_without_duplicate_slashes() {
        assert_eq!(join_remote_path("mega:nero/", "backup.age"), "mega:nero/backup.age");
        assert_eq!(join_remote_path("mega:nero", "backup.age"), "mega:nero/backup.age");
    }

    #[test]
    fn rejects_remote_path_traversal() {
        assert!(validate_remote_name("../backup.age").is_err());
        assert!(validate_remote_name("nested/backup.age").is_err());
        assert!(validate_remote_name("backup.age").is_ok());
    }

    #[test]
    fn storage_profiles_round_trip_without_credentials() {
        let root = std::env::temp_dir().join(format!("nero-storage-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let workspace = Workspace::init(&root).unwrap();

        workspace.storage_add("mega", "mega:nero-backups", true).unwrap();
        let remotes = workspace.storage_remotes().unwrap();
        assert_eq!(remotes, vec![StorageRemote {
            name: "mega".into(),
            target: "mega:nero-backups".into(),
            encrypt_backups: true,
        }]);

        let removed = workspace.storage_remove("mega").unwrap();
        assert_eq!(removed.name, "mega");
        assert!(workspace.storage_remotes().unwrap().is_empty());

        let _ = fs::remove_dir_all(root);
    }
}
