//! Persistent user-level preferences, primarily named and default workspaces.
//!
//! Workspace preferences live outside note directories so the CLI can select a
//! workspace from any current directory without changing the portable Markdown tree.

use std::{
    collections::BTreeMap,
    env,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use nero_core::{NeroError, Result, Workspace};

const CONFIG_FORMAT: &str = "nero-config";
const CONFIG_VERSION: u32 = 1;
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct UserConfig {
    format: String,
    version: u32,
    /// Used when no `--workspace` or `NERO_WORKSPACE` override is provided.
    default_workspace_path: Option<PathBuf>,
    /// Name of a registered workspace selected as the default.
    default_workspace_name: Option<String>,
    /// Named workspace paths are canonical absolute paths for stable resolution.
    workspaces: BTreeMap<String, PathBuf>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            format: CONFIG_FORMAT.to_owned(),
            version: CONFIG_VERSION,
            default_workspace_path: None,
            default_workspace_name: None,
            workspaces: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceConfigView {
    pub path: PathBuf,
    pub default_workspace_path: Option<PathBuf>,
    pub default_workspace_name: Option<String>,
    pub workspaces: Vec<(String, PathBuf)>,
}

pub fn config_path() -> Result<PathBuf> {
    let directory = if let Some(path) = env::var_os("NERO_CONFIG_HOME") {
        PathBuf::from(path)
    } else if cfg!(target_os = "windows") {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| NeroError::Message("APPDATA is not set; cannot locate Nero configuration".into()))?
            .join("Nero")
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| NeroError::Message("HOME is not set; cannot locate Nero configuration".into()))?
            .join("Library")
            .join("Application Support")
            .join("Nero")
    } else {
        let base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .ok_or_else(|| NeroError::Message("HOME/XDG_CONFIG_HOME is not set; cannot locate Nero configuration".into()))?;
        base.join("nero")
    };
    Ok(directory.join(CONFIG_FILE))
}

/// Read the user's default workspace, if one has been configured.
pub fn default_workspace() -> Result<Option<PathBuf>> {
    let config = load()?;
    if let Some(name) = config.default_workspace_name {
        return config.workspaces.get(&name).cloned().map(Some).ok_or_else(|| {
            NeroError::Message(format!("configured default workspace `{name}` no longer exists; run `nero workspace list` or `nero workspace clear`"))
        });
    }
    Ok(config.default_workspace_path)
}

/// Resolve a `--workspace` target as a registered name first, otherwise as a path.
pub fn resolve_target(target: &str) -> Result<PathBuf> {
    let path = Path::new(target);
    // Explicit paths should still work if a user's aliases file is damaged; only
    // simple names need to consult the registry.
    if path.is_absolute() || matches!(target, "." | "..") || target.contains('/') || target.contains('\\') {
        return Ok(path.to_path_buf());
    }
    let config = load()?;
    if let Some(path) = config.workspaces.get(target) {
        return Ok(path.clone());
    }
    Ok(PathBuf::from(target))
}

pub fn set_default_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = validate_workspace_path(path.as_ref())?;
    let mut config = load()?;
    config.default_workspace_path = Some(path.clone());
    config.default_workspace_name = None;
    save(&config)?;
    Ok(path)
}

pub fn add_workspace(name: &str, path: impl AsRef<Path>) -> Result<PathBuf> {
    if !valid_workspace_name(name) {
        return Err(NeroError::Message("workspace name must contain only letters, numbers, '.', '_' or '-'".into()));
    }
    let path = validate_workspace_path(path.as_ref())?;
    let mut config = load()?;
    if config.workspaces.contains_key(name) {
        return Err(NeroError::Message(format!("workspace name already exists: {name}")));
    }
    config.workspaces.insert(name.to_owned(), path.clone());
    save(&config)?;
    Ok(path)
}

pub fn use_workspace(name: &str) -> Result<PathBuf> {
    let mut config = load()?;
    let path = config.workspaces.get(name).cloned().ok_or_else(|| {
        NeroError::Message(format!("unknown workspace `{name}`; use `nero workspace list`"))
    })?;
    config.default_workspace_name = Some(name.to_owned());
    config.default_workspace_path = None;
    save(&config)?;
    Ok(path)
}

pub fn remove_workspace(name: &str) -> Result<PathBuf> {
    let mut config = load()?;
    let path = config.workspaces.remove(name).ok_or_else(|| {
        NeroError::Message(format!("unknown workspace `{name}`; use `nero workspace list`"))
    })?;
    if config.default_workspace_name.as_deref() == Some(name) {
        config.default_workspace_name = None;
    }
    save(&config)?;
    Ok(path)
}

pub fn clear_default() -> Result<()> {
    let mut config = load()?;
    config.default_workspace_path = None;
    config.default_workspace_name = None;
    save(&config)
}

pub fn view() -> Result<WorkspaceConfigView> {
    let config = load()?;
    Ok(WorkspaceConfigView {
        path: config_path()?,
        default_workspace_path: config.default_workspace_path,
        default_workspace_name: config.default_workspace_name,
        workspaces: config.workspaces.into_iter().collect(),
    })
}

fn validate_workspace_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    let canonical = fs::canonicalize(&absolute).map_err(|error| {
        NeroError::Message(format!("cannot access workspace {}: {error}", absolute.display()))
    })?;
    if !canonical.is_dir() || !canonical.join(".nero").is_dir() {
        return Err(NeroError::Message(format!(
            "{} is not an initialized Nero workspace; run `nero init {}` first",
            canonical.display(), canonical.display()
        )));
    }
    Ok(canonical)
}

fn valid_workspace_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

fn load() -> Result<UserConfig> {
    load_from(&config_path()?)
}

fn load_from(path: &Path) -> Result<UserConfig> {
    if !path.exists() {
        return Ok(UserConfig::default());
    }
    let source = fs::read_to_string(&path)?;
    let config: UserConfig = serde_json::from_str(&source).map_err(|error| {
        NeroError::Message(format!("invalid Nero configuration at {}: {error}", path.display()))
    })?;
    if config.format != CONFIG_FORMAT || config.version != CONFIG_VERSION {
        return Err(NeroError::Message(format!(
            "unsupported Nero configuration format/version at {}",
            path.display()
        )));
    }
    Ok(config)
}

fn save(config: &UserConfig) -> Result<()> {
    save_to(config, &config_path()?)
}

fn save_to(config: &UserConfig, path: &Path) -> Result<()> {
    let parent = path.parent().ok_or_else(|| NeroError::Message("Nero config path has no parent directory".into()))?;
    fs::create_dir_all(parent)?;
    let contents = serde_json::to_vec_pretty(config)
        .map_err(|error| NeroError::Message(format!("could not serialize Nero config: {error}")))?;
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(&contents)?;
    temp.as_file().sync_all()?;
    temp.persist(&path).map_err(|error| NeroError::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_workspace_aliases() {
        assert!(valid_workspace_name("research-2026"));
        assert!(valid_workspace_name("personal_notes"));
        assert!(!valid_workspace_name("../outside"));
        assert!(!valid_workspace_name("two words"));
    }

    #[test]
    fn default_config_has_no_implicit_workspace() {
        let config = UserConfig::default();
        assert!(config.default_workspace_path.is_none());
        assert!(config.default_workspace_name.is_none());
        assert!(config.workspaces.is_empty());
    }

    #[test]
    fn user_config_round_trips_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nero").join("config.json");
        let mut config = UserConfig::default();
        config.default_workspace_path = Some(PathBuf::from("/tmp/example-notes"));
        config.workspaces.insert("research".into(), PathBuf::from("/tmp/research-notes"));
        save_to(&config, &path).unwrap();

        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.default_workspace_path, config.default_workspace_path);
        assert_eq!(loaded.workspaces, config.workspaces);
        assert_eq!(loaded.format, CONFIG_FORMAT);
        assert_eq!(loaded.version, CONFIG_VERSION);
    }
}
