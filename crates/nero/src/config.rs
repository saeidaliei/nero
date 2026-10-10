//! Persistent user-level preferences, primarily named and default workspaces.
//!
//! Workspace preferences live outside note directories so the CLI can select a
//! workspace from any current directory without changing the portable Markdown tree.

use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use nero_core::{
    NeroError, Result, Workspace, is_nero_home_path, is_workspace_root, nero_home_dir,
};

const CONFIG_FORMAT: &str = "nero-config";
const CONFIG_VERSION: u32 = 1;
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct UserConfig {
    format: String,
    version: u32,
    /// Parent directory containing named workspaces, defaulting to `$NERO_HOME/workspaces`.
    workspace_home_path: Option<PathBuf>,
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
            workspace_home_path: None,
            default_workspace_path: None,
            default_workspace_name: None,
            workspaces: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceConfigView {
    pub workspace_home_path: PathBuf,
    pub default_workspace_path: Option<PathBuf>,
    pub default_workspace_name: Option<String>,
    pub workspaces: Vec<(String, PathBuf)>,
}

pub fn config_path() -> Result<PathBuf> {
    // `NERO_CONFIG_HOME` remains available for isolated tests and advanced setups.
    // Normal installations keep the config alongside all other application state.
    if let Some(path) = env::var_os("NERO_CONFIG_HOME") {
        return Ok(PathBuf::from(path).join(CONFIG_FILE));
    }

    Ok(nero_home_dir()?.join(CONFIG_FILE))
}

/// Return the workspace container. It is a parent directory, never a workspace itself.
pub fn workspace_home() -> Result<PathBuf> {
    let config = load()?;
    if let Some(path) = config.workspace_home_path {
        return Ok(path);
    }
    default_workspace_home()
}

fn default_workspace_home() -> Result<PathBuf> {
    Ok(nero_home_dir()?.join("workspaces"))
}

/// Persist a custom workspace container after ensuring it is not itself a workspace.
pub fn set_workspace_home(path: impl AsRef<Path>) -> Result<PathBuf> {
    let canonical = canonical_workspace_home(path.as_ref())?;
    let mut config = load()?;
    config.workspace_home_path = Some(canonical.clone());
    save(&config)?;
    Ok(canonical)
}

/// Canonicalize the workspace container and reject any path inside an initialized workspace.
/// Otherwise a parent workspace would index the child workspaces as if they were its own notes.
fn canonical_workspace_home(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    // Keep the lexical form as well as a canonical form: on first run the home
    // may not exist yet, while after creation canonical paths also matter (symlinks).
    if is_nero_home_path(&absolute) {
        return Err(NeroError::Message(format!(
            "workspace home cannot be the Nero application home {}; use {}/workspaces instead",
            absolute.display(),
            absolute.display()
        )));
    }

    // Check the nearest existing directory before creating anything, so an invalid
    // nested home doesn't leave an empty directory inside a note workspace.
    let anchor = absolute
        .ancestors()
        .find(|candidate| candidate.is_dir())
        .ok_or_else(|| {
            NeroError::Message(format!(
                "cannot find an existing parent for workspace home {}",
                absolute.display()
            ))
        })?;
    let canonical_anchor = fs::canonicalize(anchor)?;
    if canonical_anchor.ancestors().any(is_workspace_root) {
        return Err(NeroError::Message(format!(
            "{} is inside an initialized Nero workspace; workspace home must be a separate parent directory",
            absolute.display()
        )));
    }

    fs::create_dir_all(&absolute)?;
    let canonical = fs::canonicalize(&absolute)?;
    // Recheck after canonicalization in case the directory tree changed concurrently.
    if canonical.ancestors().any(is_workspace_root) {
        return Err(NeroError::Message(format!(
            "{} is inside an initialized Nero workspace; workspace home must be a separate parent directory",
            canonical.display()
        )));
    }
    Ok(canonical)
}

/// Restore the conventional `$NERO_HOME/workspaces` container. Existing workspaces are not moved.
pub fn reset_workspace_home() -> Result<PathBuf> {
    let mut config = load()?;
    config.workspace_home_path = None;
    save(&config)?;
    default_workspace_home()
}

/// Create and register a workspace beneath the workspace home. The first workspace
/// created becomes the default; later creations do not silently switch the default.
pub fn create_workspace(name: &str) -> Result<PathBuf> {
    if !valid_workspace_name(name) {
        return Err(NeroError::Message(
            "workspace name must contain only letters, numbers, '.', '_' or '-'".into(),
        ));
    }
    let mut config = load()?;
    if config.workspaces.contains_key(name) {
        return Err(NeroError::Message(format!(
            "workspace name already exists: {name}"
        )));
    }
    let home = match config.workspace_home_path.clone() {
        Some(home) => home,
        None => default_workspace_home()?,
    };
    let home = canonical_workspace_home(&home)?;
    let path = home.join(name);
    if path.exists() {
        return Err(NeroError::Message(format!(
            "{} already exists; use `nero workspace add {name} {}` to register an existing workspace",
            path.display(),
            path.display()
        )));
    }
    let workspace = Workspace::init(&path)?;
    let path = workspace.root().to_path_buf();
    config.workspaces.insert(name.to_owned(), path.clone());
    if config.default_workspace_path.is_none() && config.default_workspace_name.is_none() {
        config.default_workspace_name = Some(name.to_owned());
    }
    save(&config)?;
    Ok(path)
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
    if path.is_absolute()
        || matches!(target, "." | "..")
        || target.contains('/')
        || target.contains('\\')
    {
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
        return Err(NeroError::Message(
            "workspace name must contain only letters, numbers, '.', '_' or '-'".into(),
        ));
    }
    let path = validate_workspace_path(path.as_ref())?;
    let mut config = load()?;
    if config.workspaces.contains_key(name) {
        return Err(NeroError::Message(format!(
            "workspace name already exists: {name}"
        )));
    }
    config.workspaces.insert(name.to_owned(), path.clone());
    save(&config)?;
    Ok(path)
}

pub fn use_workspace(name: &str) -> Result<PathBuf> {
    let mut config = load()?;
    let path = config.workspaces.get(name).cloned().ok_or_else(|| {
        NeroError::Message(format!(
            "unknown workspace `{name}`; use `nero workspace list`"
        ))
    })?;
    config.default_workspace_name = Some(name.to_owned());
    config.default_workspace_path = None;
    save(&config)?;
    Ok(path)
}

pub fn remove_workspace(name: &str) -> Result<PathBuf> {
    let mut config = load()?;
    let path = config.workspaces.remove(name).ok_or_else(|| {
        NeroError::Message(format!(
            "unknown workspace `{name}`; use `nero workspace list`"
        ))
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
    let workspace_home_path = match config.workspace_home_path.clone() {
        Some(home) => home,
        None => default_workspace_home()?,
    };
    Ok(WorkspaceConfigView {
        workspace_home_path,
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
        NeroError::Message(format!(
            "cannot access workspace {}: {error}",
            absolute.display()
        ))
    })?;
    if !canonical.is_dir() || !canonical.join(".nero").is_dir() {
        return Err(NeroError::Message(format!(
            "{} is not an initialized Nero workspace; run `nero init {}` first",
            canonical.display(),
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn valid_workspace_name(name: &str) -> bool {
    !name.is_empty()
        && !matches!(name, "." | "..")
        && !name.starts_with('.')
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

fn load() -> Result<UserConfig> {
    load_from(&config_path()?)
}

fn load_from(path: &Path) -> Result<UserConfig> {
    if !path.exists() {
        return Ok(UserConfig::default());
    }
    let source = fs::read_to_string(path)?;
    let config: UserConfig = serde_json::from_str(&source).map_err(|error| {
        NeroError::Message(format!(
            "invalid Nero configuration at {}: {error}",
            path.display()
        ))
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

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

fn save_to(config: &UserConfig, path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| NeroError::Message("Nero config path has no parent directory".into()))?;
    fs::create_dir_all(parent)?;
    // Only chmod Nero's own app-home directory; an explicit NERO_CONFIG_HOME may
    // point at a shared general-purpose config directory and should not be changed.
    if nero_home_dir()
        .ok()
        .is_some_and(|home| path == home.join(CONFIG_FILE))
    {
        set_private_directory_permissions(parent)?;
    }
    let contents = serde_json::to_vec_pretty(config)
        .map_err(|error| NeroError::Message(format!("could not serialize Nero config: {error}")))?;
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(&contents)?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|error| NeroError::Io(error.error))?;
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
        assert!(!valid_workspace_name("."));
        assert!(!valid_workspace_name(".."));
        assert!(!valid_workspace_name(".nero"));
        assert!(!valid_workspace_name("two words"));
    }

    #[test]
    fn default_workspace_container_is_under_app_home() {
        let home = nero_home_dir().unwrap();
        assert_eq!(default_workspace_home().unwrap(), home.join("workspaces"));
    }

    #[test]
    fn default_config_has_no_implicit_workspace() {
        let config = UserConfig::default();
        assert!(config.workspace_home_path.is_none());
        assert!(config.default_workspace_path.is_none());
        assert!(config.default_workspace_name.is_none());
        assert!(config.workspaces.is_empty());
    }

    #[test]
    fn user_config_round_trips_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nero").join("config.json");
        let config = UserConfig {
            workspace_home_path: Some(PathBuf::from("/tmp/Nero")),
            default_workspace_path: Some(PathBuf::from("/tmp/example-notes")),
            workspaces: BTreeMap::from([("research".into(), PathBuf::from("/tmp/research-notes"))]),
            ..UserConfig::default()
        };
        save_to(&config, &path).unwrap();

        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.workspace_home_path, config.workspace_home_path);
        assert_eq!(loaded.default_workspace_path, config.default_workspace_path);
        assert_eq!(loaded.workspaces, config.workspaces);
        assert_eq!(loaded.format, CONFIG_FORMAT);
        assert_eq!(loaded.version, CONFIG_VERSION);
    }

    #[test]
    fn app_home_is_not_treated_as_workspace_metadata() {
        let app_home = nero_home_dir().unwrap();
        assert!(!is_workspace_root(&app_home));
        if let Some(user_home) = app_home.parent() {
            assert!(!is_workspace_root(user_home));
        }
    }

    #[test]
    fn rejects_workspace_home_equal_to_app_home() {
        let app_home = nero_home_dir().unwrap();
        assert!(canonical_workspace_home(&app_home).is_err());
    }

    #[test]
    fn rejects_workspace_home_nested_inside_an_existing_workspace() {
        let temporary = tempfile::tempdir().unwrap();
        let outer = Workspace::init(temporary.path().join("outer")).unwrap();
        let nested_home = outer.root().join("nested-home");
        assert!(canonical_workspace_home(&nested_home).is_err());
        assert!(
            !nested_home.exists(),
            "invalid workspace home should not be created inside another workspace"
        );
    }
}
