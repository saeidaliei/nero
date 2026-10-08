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
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    time::{SystemTime, UNIX_EPOCH},
};

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
        fs::write(&path, format!("# {clean_title}\n\n"))?;
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
        let normalized_parent = fs::canonicalize(absolute.parent().unwrap_or(&self.root))?;
        let normalized_file = fs::canonicalize(&absolute)?;
        if !normalized_parent.starts_with(&self.root) || !normalized_file.starts_with(&self.root) {
            return Err(NeroError::Message("note path escapes workspace".into()));
        }
        fs::write(&absolute, source)?;
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
        if !absolute.exists() {
            fs::create_dir_all(absolute.parent().expect("daily has a parent"))?;
            fs::write(&absolute, format!("# {now}\n\n"))?;
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
            let clean = candidate.strip_prefix("./").unwrap_or(&candidate);
            let full = self.root.join(clean);
            if full.is_file() && full.extension().and_then(|s| s.to_str()) == Some("md") {
                return Ok(Some(self.relative_path(&full)));
            }
        }
        let raw_lower = raw.to_lowercase();
        let raw_stem = raw.trim_end_matches(".md").to_lowercase();
        let mut matches = Vec::new();
        for note in self.list_notes()? {
            let path_lower = normalize_path(&note.path).to_lowercase();
            let stem_lower = note.path.file_stem().and_then(|v| v.to_str()).unwrap_or_default().to_lowercase();
            if path_lower == raw_lower || stem_lower == raw_stem || note.title.to_lowercase() == raw_lower {
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
        if let Some(note) = notes.iter().find(|note| normalize_relative_path(&note.path).as_deref() == Some(normalized.as_str())) {
            return Some(note.clone());
        }
    }

    let lower = target_without_fragment.to_lowercase();
    let stem = target_without_fragment
        .trim_end_matches(".md")
        .to_lowercase();

    if let Some(note) = notes.iter().find(|note| note.title.to_lowercase() == lower) {
        return Some(note.clone());
    }

    notes.iter().find(|note| {
        note.path
            .file_stem()
            .and_then(|value| value.to_str())
            .map(|value| value.to_lowercase() == stem)
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
    Some(components.join("/").to_lowercase())
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
            if path.is_dir() { visit(root, &path, out)?; continue; }
            if path.extension().and_then(|s| s.to_str()) != Some("md") { continue; }
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
    let Some(byte_index) = lower.find(&query_lower) else {
        return body.lines().next().unwrap_or_default().trim().to_owned();
    };

    let char_index = lower[..byte_index].chars().count();
    let query_chars = query_lower.chars().count();
    let total_chars = body.chars().count();
    let start_char = char_index.saturating_sub(72);
    let end_char = (char_index + query_chars + 120).min(total_chars);
    let preview = body
        .chars()
        .skip(start_char)
        .take(end_char.saturating_sub(start_char))
        .collect::<String>()
        .replace('\n', " ");
    let mut preview = preview;
    if start_char > 0 { preview.insert_str(0, "…"); }
    if end_char < total_chars { preview.push('…'); }
    preview.trim().to_owned()
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

fn chrono_like_date() -> String {
    let days = SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_secs() / 86_400).unwrap_or(0);
    civil_from_days(days as i64)
}

fn civil_from_days(days_since_epoch: i64) -> String {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
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
