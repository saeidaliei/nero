mod backup;
mod encryption;
mod git;
mod document;
mod index;
mod watcher;
mod storage;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};

use std::{
    env,
    error::Error,
    fmt,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use chrono::Local;
use tempfile::NamedTempFile;

pub use backup::{BackupEntry, BackupManifest, BackupStats};
pub use encryption::BackupKeyInfo;
pub use git::GitStatus;
pub use document::{render_html, Document, TaskCounts, WikiLink};
pub use index::IndexStats;
pub use watcher::WorkspaceWatcher;
pub use storage::{RemoteBackup, StorageRemote};

use index::Index;

pub type Result<T> = std::result::Result<T, NeroError>;

#[derive(Debug)]
pub enum NeroError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Notify(notify::Error),
    Message(String),
}

impl fmt::Display for NeroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Sqlite(err) => write!(f, "{err}"),
            Self::Notify(err) => write!(f, "{err}"),
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl Error for NeroError {}
impl From<std::io::Error> for NeroError { fn from(value: std::io::Error) -> Self { Self::Io(value) } }
impl From<rusqlite::Error> for NeroError { fn from(value: rusqlite::Error) -> Self { Self::Sqlite(value) } }
impl From<notify::Error> for NeroError { fn from(value: notify::Error) -> Self { Self::Notify(value) } }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSummary {
    pub path: PathBuf,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct Note {
    pub summary: NoteSummary,
    pub document: Document,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub note: NoteSummary,
    pub score: f64,
    pub preview: String,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = canonicalize_root(root.into())?;
        if !root.is_dir() {
            return Err(NeroError::Message(format!(
                "workspace does not exist or is not a directory: {}",
                root.display()
            )));
        }
        Ok(Self { root })
    }

    pub fn init(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        fs::create_dir_all(root.join(".nero"))?;
        Self::open(root)
    }

    pub fn discover(start: impl AsRef<Path>) -> Result<Self> {
        let start = canonicalize_root(start.as_ref().to_path_buf())?;
        for candidate in start.ancestors() {
            if candidate.join(".nero").is_dir() || candidate.join(".note").is_dir() {
                return Self::open(candidate);
            }
        }
        Err(NeroError::Message(format!(
            "no Nero workspace found from {}",
            start.display()
        )))
    }

    pub fn root(&self) -> &Path { &self.root }
    pub fn metadata_dir(&self) -> PathBuf { self.root.join(".nero") }
    pub fn index_path(&self) -> PathBuf { self.metadata_dir().join("index.sqlite") }

    pub fn create_note(&self, title: &str) -> Result<NoteSummary> {
        let clean_title = title.trim();
        if clean_title.is_empty() {
            return Err(NeroError::Message("note title cannot be empty".into()));
        }
        let path = self.note_path_for_title(clean_title)?;
        if path.exists() {
            return Err(NeroError::Message(format!(
                "note already exists: {}",
                self.relative_path(&path).display()
            )));
        }
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        atomic_write(&path, format!("# {clean_title}\n\n").as_bytes())?;
        Ok(NoteSummary { path: self.relative_path(&path), title: clean_title.to_owned() })
    }

    pub fn read_note(&self, query: &str) -> Result<Note> {
        let path = self.resolve_note(query)?.ok_or_else(|| NeroError::Message(format!("note not found: {query}")))?;
        let body = fs::read_to_string(self.root.join(&path))?;
        let document = document::Document::parse(&body);
        let title = if document.title == "Untitled" {
            path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_owned()
        } else {
            document.title.clone()
        };
        Ok(Note { summary: NoteSummary { path, title }, document })
    }

    pub fn list_notes(&self) -> Result<Vec<NoteSummary>> {
        let mut notes = Vec::new();
        collect_markdown_files(&self.root, &mut notes)?;
        notes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(notes)
    }

    pub fn render_note_html(&self, query: &str) -> Result<String> {
        Ok(self.read_note(query)?.document.html)
    }

    pub fn save_note(&self, query: &str, source: &str) -> Result<Note> {
        let path = self.resolve_note(query)?
            .ok_or_else(|| NeroError::Message(format!("note not found: {query}")))?;
        let absolute = self.root.join(&path);
        if absolute.extension().and_then(|s| s.to_str()) != Some("md") {
            return Err(NeroError::Message("only Markdown notes can be saved".into()));
        }
        let normalized_file = canonical_workspace_file(&self.root, &absolute)?
            .ok_or_else(|| NeroError::Message("note path escapes workspace".into()))?;
        atomic_write(&normalized_file, source.as_bytes())?;
        let document = document::Document::parse(source);
        let title = if document.title == "Untitled" {
            path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_owned()
        } else {
            document.title.clone()
        };
        Ok(Note {
            summary: NoteSummary { path, title },
            document,
        })
    }

    /// Copy a user-selected image into the workspace's `assets/` directory.
    ///
    /// The returned path is workspace-relative so it can be inserted directly into Markdown.
    pub fn import_asset(&self, source: impl AsRef<Path>) -> Result<String> {
        let source = fs::canonicalize(source.as_ref())?;
        if !source.is_file() {
            return Err(NeroError::Message("selected asset is not a file".into()));
        }

        let assets_dir = self.root.join("assets");
        fs::create_dir_all(&assets_dir)?;

        let original_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| NeroError::Message("asset filename is not valid UTF-8".into()))?;
        if asset_mime_type(&source) == "application/octet-stream" {
            return Err(NeroError::Message("only image assets can be imported".into()));
        }
        let clean_name = sanitize_asset_filename(original_name);
        let destination = unique_asset_path(&assets_dir, &clean_name);
        fs::copy(&source, &destination)?;

        Ok(self.relative_path(&destination).to_string_lossy().replace('\\', "/"))
    }

    /// Read a workspace-relative or note-relative asset as a data URL.
    ///
    /// Returning a data URL keeps the desktop UI independent of filesystem URL permissions
    /// while still allowing Markdown image syntax to remain ordinary and portable.
    pub fn read_asset_data_url(&self, source_note: &Path, reference: &str) -> Result<String> {
        let without_fragment = reference.split_once('#').map_or(reference, |(path, _)| path);
        let clean_reference = without_fragment.split_once('?').map_or(without_fragment, |(path, _)| path).trim();
        if clean_reference.is_empty()
            || clean_reference.starts_with("http://")
            || clean_reference.starts_with("https://")
            || clean_reference.starts_with("data:")
            || Path::new(clean_reference).is_absolute()
        {
            return Err(NeroError::Message("asset reference is not a local workspace path".into()));
        }

        let source_absolute = self.root.join(source_note);
        let source_canonical = fs::canonicalize(&source_absolute)
            .map_err(|_| NeroError::Message("source note is not inside the workspace".into()))?;
        if !source_canonical.starts_with(&self.root) || !source_canonical.is_file() {
            return Err(NeroError::Message("source note is outside the workspace".into()));
        }
        let note_parent = source_canonical.parent().unwrap_or(&self.root);
        let candidate = note_parent.join(clean_reference);
        let canonical = fs::canonicalize(&candidate)?;
        if !canonical.starts_with(&self.root) || canonical.starts_with(&self.metadata_dir()) || !canonical.is_file() {
            return Err(NeroError::Message("asset is outside the workspace or not a file".into()));
        }

        let mime = asset_mime_type(&canonical);
        if mime == "application/octet-stream" {
            return Err(NeroError::Message("only image assets can be displayed".into()));
        }
        let bytes = fs::read(&canonical)?;
        let encoded = BASE64_STANDARD.encode(bytes);
        Ok(format!("data:{mime};base64,{encoded}"))
    }

    pub fn reindex(&self) -> Result<IndexStats> {
        let mut index = Index::open(&self.index_path())?;
        index.refresh(self)
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let mut index = Index::open(&self.index_path())?;
        index.refresh(self)?;
        index.search(query)
    }

    pub fn backlinks(&self, query: &str) -> Result<Vec<NoteSummary>> {
        let target = self
            .resolve_note(query)?
            .ok_or_else(|| NeroError::Message(format!("note not found: {query}")))?;
        let mut index = Index::open(&self.index_path())?;
        index.refresh(self)?;
        let summaries = self.list_notes()?;
        let mut found = Vec::new();

        for source_path in index.link_sources_for_target(&target, &summaries)? {
            if let Some(summary) = summaries.iter().find(|note| note.path == source_path) {
                found.push(summary.clone());
            }
        }
        found.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(found)
    }

    /// Resolve a wiki link relative to the note containing it.
    ///
    /// Resolution order is deliberate: relative path, workspace path, title, then filename stem.
    pub fn resolve_link(&self, source_path: &Path, target: &str) -> Result<Option<NoteSummary>> {
        let notes = self.list_notes()?;
        Ok(resolve_link_from_catalog(source_path, target, &notes))
    }


    pub fn today(&self) -> Result<NoteSummary> {
        let now = chrono_like_date();
        let path = PathBuf::from("daily").join(format!("{now}.md"));
        let absolute = self.root.join(&path);
        match fs::symlink_metadata(&absolute) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(NeroError::Message("today's note path is a symlink; refusing to follow it".into()));
            }
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Err(NeroError::Message("today's note path exists but is not a regular file".into())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(absolute.parent().expect("daily has a parent"))?;
                atomic_write(&absolute, format!("# {now}\n\n").as_bytes())?;
            }
            Err(error) => return Err(error.into()),
        }
        Ok(NoteSummary { path, title: now })
    }

    pub fn edit(&self, query: &str) -> Result<ExitStatus> {
        let note = self.read_note(query)?;
        let editor = env::var("EDITOR")
            .or_else(|_| env::var("VISUAL"))
            .map_err(|_| NeroError::Message("set $EDITOR or $VISUAL before using `nero edit`".into()))?;
        let mut parts = editor.split_whitespace();
        let command = parts.next().ok_or_else(|| NeroError::Message("$EDITOR is empty".into()))?;
        let mut child = Command::new(command);
        for arg in parts { child.arg(arg); }
        child.arg(self.root.join(&note.summary.path));
        Ok(child.status()?)
    }

    pub fn doctor(&self) -> Result<Vec<String>> {
        let mut report = Vec::new();
        if self.metadata_dir().is_dir() {
            report.push(format!("ok: {} directory exists", self.metadata_dir().file_name().unwrap().to_string_lossy()));
        } else {
            fs::create_dir_all(self.metadata_dir())?;
            report.push("fixing: .nero directory was missing".into());
        }
        let notes = self.list_notes()?;
        report.push(format!("ok: {} Markdown note(s) found", notes.len()));
        let stats = self.reindex()?;
        report.push(format!("ok: index refreshed ({} updated, {} removed)", stats.indexed, stats.removed));

        let mut broken_links = 0usize;
        for note in notes {
            let note_data = self.read_note(&note.path)?;
            for link in &note_data.document.wiki_links {
                if self.resolve_link(&note.path, &link.target)?.is_none() { broken_links += 1; }
            }
        }
        if broken_links == 0 { report.push("ok: no broken wiki links found".into()); }
        else { report.push(format!("warning: {broken_links} broken wiki link(s) found")); }

        let storage = self.storage_remotes()?;
        if storage.is_empty() {
            report.push("info: no remote storage profiles configured".into());
        } else {
            report.push(format!("ok: {} remote storage profile(s) configured", storage.len()));
        }
        Ok(report)
    }

    fn note_path_for_title(&self, title: &str) -> Result<PathBuf> {
        let slug = slugify(title);
        if slug.is_empty() {
            return Err(NeroError::Message("title must contain at least one alphanumeric character".into()));
        }
        Ok(self.root.join(format!("{slug}.md")))
    }

    fn resolve_note(&self, query: &str) -> Result<Option<PathBuf>> {
        let raw = query.trim();
        if raw.is_empty() { return Ok(None); }
        let path_query = PathBuf::from(raw);
        let candidates = [path_query.clone(), PathBuf::from(format!("{raw}.md")), PathBuf::from(format!("{}.md", slugify(raw)))];
        for candidate in candidates {
            if let Some(path) = resolve_existing_note_candidate(&self.root, &candidate)? {
                return Ok(Some(self.relative_path(&path)));
            }
        }
        let raw_lower = raw.to_lowercase();
        let raw_stem = raw.trim_end_matches(".md");
        let mut matches = Vec::new();
        for note in self.list_notes()? {
            let path_key = path_match_key(&note.path);
            let stem = note.path.file_stem().and_then(|v| v.to_str()).unwrap_or_default();
            let stem_matches = path_match_key(Path::new(stem)) == path_match_key(Path::new(raw_stem));
            if path_key == path_match_key(Path::new(raw)) || stem_matches || note.title.to_lowercase() == raw_lower {
                matches.push(note.path);
            }
        }
        Ok(matches.into_iter().next())
    }

    fn relative_path(&self, absolute: &Path) -> PathBuf {
        absolute.strip_prefix(&self.root).unwrap_or(absolute).to_path_buf()
    }
}

fn resolve_link_from_catalog(source_path: &Path, target: &str, notes: &[NoteSummary]) -> Option<NoteSummary> {
    let raw = target.trim();
    if raw.is_empty() {
        return None;
    }
    let target_without_fragment = raw.split_once('#').map_or(raw, |(path, _)| path).trim();
    if target_without_fragment.is_empty() {
        return None;
    }

    let source_parent = source_path.parent().unwrap_or_else(|| Path::new(""));
    let mut path_candidates = Vec::new();
    let raw_path = PathBuf::from(target_without_fragment);
    let with_extension = if raw_path.extension().is_some() {
        raw_path.clone()
    } else {
        PathBuf::from(format!("{}.md", target_without_fragment))
    };

    path_candidates.push(source_parent.join(&with_extension));
    path_candidates.push(with_extension.clone());

    for candidate in path_candidates {
        let normalized = normalize_relative_path(&candidate)?;
        let normalized_key = path_match_key(Path::new(&normalized));
        if let Some(note) = notes.iter().find(|note| path_match_key(&note.path) == normalized_key) {
            return Some(note.clone());
        }
    }

    let lower = target_without_fragment.to_lowercase();
    let stem = target_without_fragment.trim_end_matches(".md");

    if let Some(note) = notes.iter().find(|note| note.title.to_lowercase() == lower) {
        return Some(note.clone());
    }

    notes.iter().find(|note| {
        note.path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(|value| path_match_key(Path::new(value)) == path_match_key(Path::new(stem)))
            .unwrap_or(false)
    }).cloned()
}

fn normalize_relative_path(path: &Path) -> Option<String> {
    let mut components = Vec::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::Normal(value) => components.push(value.to_string_lossy().to_string()),
            Component::ParentDir => {
                components.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(components.join("/"))
}

fn path_match_key(path: &Path) -> String {
    let normalized = normalize_relative_path(path).unwrap_or_else(|| normalize_path(path));
    #[cfg(windows)]
    { normalized.to_lowercase() }
    #[cfg(not(windows))]
    { normalized }
}

fn resolve_existing_note_candidate(root: &Path, candidate: &Path) -> Result<Option<PathBuf>> {
    use std::path::Component;
    if candidate.is_absolute() { return Ok(None); }
    let clean = candidate.strip_prefix("./").unwrap_or(candidate);
    let mut current = root.to_path_buf();
    for component in clean.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => {
                current.push(value);
                match fs::symlink_metadata(&current) {
                    Ok(metadata) if metadata.file_type().is_symlink() => return Ok(None),
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => return Err(error.into()),
                }
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return Ok(None),
        }
    }

    if current.extension().and_then(|s| s.to_str()) != Some("md") { return Ok(None); }
    if !current.is_file() { return Ok(None); }
    let canonical = fs::canonicalize(&current)?;
    if !canonical.starts_with(root) { return Ok(None); }
    Ok(Some(canonical))
}

fn canonical_workspace_file(root: &Path, path: &Path) -> Result<Option<PathBuf>> {
    if path.is_absolute() && !path.starts_with(root) { return Ok(None); }
    let relative = path.strip_prefix(root).unwrap_or(path);
    let mut current = root.to_path_buf();
    for component in relative.components() {
        use std::path::Component;
        match component {
            Component::CurDir => {}
            Component::Normal(value) => {
                current.push(value);
                if let Ok(metadata) = fs::symlink_metadata(&current) {
                    if metadata.file_type().is_symlink() { return Ok(None); }
                }
            }
            _ => return Ok(None),
        }
    }
    let canonical = fs::canonicalize(&current)?;
    if canonical.starts_with(root) { Ok(Some(canonical)) } else { Ok(None) }
}

pub(crate) fn resolve_link_for_index(source_path: &Path, target: &str, notes: &[NoteSummary]) -> Option<NoteSummary> {
    resolve_link_from_catalog(source_path, target, notes)
}

fn canonicalize_root(root: PathBuf) -> Result<PathBuf> {
    if root.exists() { Ok(fs::canonicalize(root)?) } else { Ok(root) }
}

fn collect_markdown_files(root: &Path, out: &mut Vec<NoteSummary>) -> Result<()> {
    fn visit(root: &Path, dir: &Path, out: &mut Vec<NoteSummary>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') { continue; }
            let file_type = fs::symlink_metadata(&path)?.file_type();
            if file_type.is_symlink() { continue; }
            if file_type.is_dir() { visit(root, &path, out)?; continue; }
            if !file_type.is_file() || path.extension().and_then(|s| s.to_str()) != Some("md") { continue; }
            let body = fs::read_to_string(&path)?;
            let document = document::Document::parse(&body);
            let title = if document.title == "Untitled" {
                path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_owned()
            } else { document.title };
            out.push(NoteSummary { path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(), title });
        }
        Ok(())
    }
    visit(root, root, out)
}

pub(crate) fn preview_for_query(body: &str, query: &str) -> String {
    let lower = body.to_lowercase();
    let query_lower = query.to_lowercase();
    let Some(lower_start) = lower.find(&query_lower) else {
        return body.lines().next().unwrap_or_default().trim().to_owned();
    };

    let lower_end = lower_start + query_lower.len();
    let original_start = lower_byte_to_original(body, lower_start, false);
    let original_end = lower_byte_to_original(body, lower_end, true);
    let start = char_boundary_before(body, original_start.saturating_sub(240));
    let end = char_boundary_after(body, (original_end + 420).min(body.len()));
    let mut preview = body[start..end].replace('\n', " ");
    if start > 0 { preview.insert_str(0, "…"); }
    if end < body.len() { preview.push('…'); }
    preview.trim().to_owned()
}

fn lower_byte_to_original(body: &str, lower_target: usize, round_up: bool) -> usize {
    let mut lower_cursor = 0usize;
    for (original_start, ch) in body.char_indices() {
        let lower_len = ch.to_lowercase().map(|c| c.len_utf8()).sum::<usize>();
        let next = lower_cursor + lower_len;
        if lower_target < next || (round_up && lower_target <= next) {
            return if round_up { original_start + ch.len_utf8() } else { original_start };
        }
        lower_cursor = next;
    }
    body.len()
}

fn char_boundary_before(body: &str, index: usize) -> usize {
    if index >= body.len() { return body.len(); }
    let mut result = 0;
    for (start, _) in body.char_indices() {
        if start > index { break; }
        result = start;
    }
    result
}

fn char_boundary_after(body: &str, index: usize) -> usize {
    if index >= body.len() { return body.len(); }
    body.char_indices().find(|(start, _)| *start >= index).map(|(start, _)| start).unwrap_or(body.len())
}

fn slugify(title: &str) -> String {
    let mut result = String::new();
    let mut dash_pending = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            if dash_pending && !result.is_empty() { result.push('-'); }
            result.extend(ch.to_lowercase());
            dash_pending = false;
        } else if !result.is_empty() { dash_pending = true; }
    }
    result.trim_end_matches('-').to_owned()
}

fn sanitize_asset_filename(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            output.push(ch);
        } else if ch.is_whitespace() {
            output.push('-');
        } else {
            output.push('_');
        }
    }
    let trimmed = output.trim_matches('.').trim_matches('-').trim_matches('_');
    if trimmed.is_empty() { "asset".to_owned() } else { trimmed.to_owned() }
}

fn unique_asset_path(dir: &Path, name: &str) -> PathBuf {
    let base = Path::new(name);
    let stem = base.file_stem().and_then(|value| value.to_str()).unwrap_or("asset");
    let extension = base.extension().and_then(|value| value.to_str());
    let mut candidate = dir.join(name);
    let mut suffix = 2usize;
    while candidate.exists() {
        let filename = match extension {
            Some(ext) => format!("{stem}-{suffix}.{ext}"),
            None => format!("{stem}-{suffix}"),
        };
        candidate = dir.join(filename);
        suffix += 1;
    }
    candidate
}

fn asset_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()).map(|value| value.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("svg") => "image/svg+xml",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}


#[cfg(test)]
mod asset_tests {
    use super::*;

    #[test]
    fn sanitizes_asset_names_without_path_components() {
        assert_eq!(sanitize_asset_filename("my image (final).png"), "my-image-_final_.png");
        assert_eq!(sanitize_asset_filename("../secret.txt"), "secret.txt");
    }

    #[test]
    fn allocates_a_unique_asset_path() {
        let root = std::env::temp_dir().join(format!("nero-asset-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(root.join("photo.png"), b"one").expect("write first file");
        assert_eq!(unique_asset_path(&root, "photo.png").file_name().unwrap(), "photo-2.png");
        let _ = fs::remove_dir_all(&root);
    }
}

fn normalize_path(path: &Path) -> String { path.to_string_lossy().replace('\\', "/") }

pub(crate) fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temp = NamedTempFile::new_in(parent)?;
    if let Ok(metadata) = fs::metadata(path) {
        temp.as_file().set_permissions(metadata.permissions())?;
    }
    temp.write_all(contents)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| NeroError::Io(error.error))?;
    sync_parent_directory(parent)?;
    Ok(())
}

fn sync_parent_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let directory = fs::File::open(path)?;
        directory.sync_all()?;
    }
    Ok(())
}

fn chrono_like_date() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod hardening_tests {
    use super::*;
    use std::fs;

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("nero-hardening-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn rejects_note_paths_that_escape_workspace() {
        let root = temp_workspace("escape");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.parent().unwrap().join("outside.md"), "# outside\n").unwrap();
        let workspace = Workspace::init(&root).unwrap();
        assert!(workspace.read_note("../outside").is_err());
        let _ = fs::remove_file(root.parent().unwrap().join("outside.md"));
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn ignores_symlinked_note_files() {
        use std::os::unix::fs::symlink;
        let root = temp_workspace("symlink");
        let outside = temp_workspace("outside-file");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.md"), "# Secret\n").unwrap();
        symlink(outside.join("secret.md"), root.join("secret.md")).unwrap();
        let workspace = Workspace::init(&root).unwrap();
        assert!(workspace.list_notes().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn preview_handles_unicode_case_folding_without_panicking() {
        let body = "Straße und München sind schön.";
        let preview = preview_for_query(body, "STRASSE");
        assert!(preview.contains("Straße"));
    }

    #[test]
    fn atomic_write_replaces_file_without_partial_content() {
        let root = temp_workspace("atomic-write");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("note.md");
        fs::write(&path, b"old").unwrap();
        atomic_write(&path, b"new content").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new content");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn ignores_symlinked_note_directories() {
        use std::os::unix::fs::symlink;
        let root = temp_workspace("symlink-dir");
        let outside = temp_workspace("outside-dir");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.md"), b"# Secret\n").unwrap();
        symlink(&outside, root.join("linked")).unwrap();
        let workspace = Workspace::init(&root).unwrap();
        assert!(workspace.list_notes().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_workspace(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("nero-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn creates_and_searches_with_index() {
        let root = temp_workspace("search-index");
        let workspace = Workspace::init(&root).unwrap();
        workspace.create_note("Fourier Transform").unwrap();
        fs::write(root.join("fourier-transform.md"), "# Fourier Transform\n\nFrequency domain analysis.\n").unwrap();
        let results = workspace.search("frequency").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note.title, "Fourier Transform");
    }

    #[test]
    fn saves_note_and_reparses_document() {
        let root = temp_workspace("save-note");
        let workspace = Workspace::init(&root).unwrap();
        workspace.create_note("Editor Test").unwrap();
        let saved = workspace.save_note("editor-test", "# Edited\n\n$E=mc^2$\n").unwrap();
        assert_eq!(saved.summary.title, "Edited");
        assert_eq!(saved.document.math_count, 1);
        assert_eq!(workspace.read_note("editor-test").unwrap().document.title, "Edited");
    }

    #[test]
    fn rejects_save_outside_workspace() {
        let root = temp_workspace("save-outside");
        let workspace = Workspace::init(&root).unwrap();
        let error = workspace.save_note("../escape", "# No").unwrap_err();
        assert!(error.to_string().contains("note not found"));
    }

    #[test]
    fn resolves_links_by_relative_path_title_stem_and_fragment() {
        let root = temp_workspace("link-resolution");
        let workspace = Workspace::init(&root).unwrap();
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join("notes/Index.md"), "# Index\n\n[[../math/Fourier|the transform]]\n[[../math/Fourier#definition]]\n").unwrap();
        fs::create_dir_all(root.join("math")).unwrap();
        fs::write(root.join("math/Fourier.md"), "# Fourier Transform\n\nDefinition.\n").unwrap();
        fs::write(root.join("math/Signal.md"), "---\ntitle: Signal Processing\n---\n").unwrap();

        let resolved = workspace.resolve_link(Path::new("notes/Index.md"), "../math/Fourier").unwrap().unwrap();
        assert_eq!(resolved.path, PathBuf::from("math/Fourier.md"));

        let by_title = workspace.resolve_link(Path::new("notes/Index.md"), "Signal Processing").unwrap().unwrap();
        assert_eq!(by_title.path, PathBuf::from("math/Signal.md"));

        let by_fragment = workspace.resolve_link(Path::new("notes/Index.md"), "../math/Fourier#definition").unwrap().unwrap();
        assert_eq!(by_fragment.path, PathBuf::from("math/Fourier.md"));
    }

    #[test]
    fn finds_backlinks_through_titles() {
        let root = temp_workspace("backlinks-title");
        let workspace = Workspace::init(&root).unwrap();
        workspace.create_note("Alpha").unwrap();
        workspace.create_note("Signal Processing").unwrap();
        fs::write(root.join("alpha.md"), "# Alpha\n\nSee [[Signal Processing]].\n").unwrap();
        let links = workspace.backlinks("Signal Processing").unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].path, PathBuf::from("alpha.md"));
    }

    #[test]
    fn doctor_rebuilds_index() {
        let root = temp_workspace("doctor");
        let workspace = Workspace::init(&root).unwrap();
        workspace.create_note("Alpha").unwrap();
        let report = workspace.doctor().unwrap();
        assert!(report.iter().any(|line| line.contains("index refreshed")));
    }
}
