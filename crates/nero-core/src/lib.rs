use std::{
    env,
    error::Error,
    fmt,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    time::{SystemTime, UNIX_EPOCH},
};

pub type Result<T> = std::result::Result<T, NeroError>;

#[derive(Debug)]
pub enum NeroError {
    Io(std::io::Error),
    Message(String),
}

impl fmt::Display for NeroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl Error for NeroError {}

impl From<std::io::Error> for NeroError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSummary {
    pub path: PathBuf,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub note: NoteSummary,
    pub score: usize,
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
        fs::create_dir_all(root.join(".note"))?;
        Self::open(root)
    }

    pub fn discover(start: impl AsRef<Path>) -> Result<Self> {
        let start = canonicalize_root(start.as_ref().to_path_buf())?;
        for candidate in start.ancestors() {
            if candidate.join(".note").is_dir() {
                return Self::open(candidate);
            }
        }
        Err(NeroError::Message(format!(
            "no Nero workspace found from {}",
            start.display()
        )))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

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

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let body = format!("# {clean_title}\n\n");
        fs::write(&path, body)?;
        Ok(NoteSummary {
            path: self.relative_path(&path),
            title: clean_title.to_owned(),
        })
    }

    pub fn read_note(&self, query: &str) -> Result<(NoteSummary, String)> {
        let path = self.resolve_note(query)?.ok_or_else(|| {
            NeroError::Message(format!("note not found: {query}"))
        })?;
        let absolute = self.root.join(&path);
        let body = fs::read_to_string(&absolute)?;
        let title = title_from_markdown(&body)
            .or_else(|| path.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
            .unwrap_or_else(|| path.display().to_string());
        Ok((NoteSummary { path, title }, body))
    }

    pub fn list_notes(&self) -> Result<Vec<NoteSummary>> {
        let mut notes = Vec::new();
        collect_markdown_files(&self.root, &mut notes)?;
        notes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(notes)
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let normalized = query.trim().to_lowercase();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for note in self.list_notes()? {
            let body = fs::read_to_string(self.root.join(&note.path))?;
            let haystack = format!("{}\n{}", note.title, body).to_lowercase();
            if !haystack.contains(&normalized) {
                continue;
            }
            let score = count_non_overlapping(&haystack, &normalized);
            let preview = preview_for_query(&body, &normalized);
            results.push(SearchResult { note, score, preview });
        }
        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.note.path.cmp(&b.note.path))
        });
        Ok(results)
    }

    pub fn backlinks(&self, query: &str) -> Result<Vec<NoteSummary>> {
        let target = self.resolve_note(query)?.ok_or_else(|| {
            NeroError::Message(format!("note not found: {query}"))
        })?;
        let target_stem = target
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_lowercase();
        let target_path = target.to_string_lossy().to_lowercase();

        let mut found = Vec::new();
        for note in self.list_notes()? {
            if note.path == target {
                continue;
            }
            let body = fs::read_to_string(self.root.join(&note.path))?;
            for link in extract_wiki_links(&body) {
                let normalized = link.trim().to_lowercase();
                if normalized == target_stem
                    || normalized == target_path
                    || format!("{normalized}.md") == target_path
                {
                    found.push(note.clone());
                    break;
                }
            }
        }
        Ok(found)
    }

    pub fn today(&self) -> Result<NoteSummary> {
        let now = chrono_like_date();
        let path = PathBuf::from("daily").join(format!("{now}.md"));
        let absolute = self.root.join(&path);
        if !absolute.exists() {
            fs::create_dir_all(absolute.parent().expect("daily has a parent"))?;
            fs::write(&absolute, format!("# {now}\n\n"))?;
        }
        Ok(NoteSummary {
            path,
            title: now,
        })
    }

    pub fn edit(&self, query: &str) -> Result<ExitStatus> {
        let (note, _) = self.read_note(query)?;
        let editor = env::var("EDITOR")
            .or_else(|_| env::var("VISUAL"))
            .map_err(|_| NeroError::Message("set $EDITOR or $VISUAL before using `nero edit`".into()))?;

        let mut parts = editor.split_whitespace();
        let command = parts
            .next()
            .ok_or_else(|| NeroError::Message("$EDITOR is empty".into()))?;
        let mut child = Command::new(command);
        for arg in parts {
            child.arg(arg);
        }
        child.arg(self.root.join(&note.path));
        Ok(child.status()?)
    }

    pub fn doctor(&self) -> Result<Vec<String>> {
        let mut report = Vec::new();
        if self.root.join(".note").is_dir() {
            report.push("ok: .note directory exists".into());
        } else {
            report.push("fixing: .note directory was missing".into());
            fs::create_dir_all(self.root.join(".note"))?;
        }

        let notes = self.list_notes()?;
        report.push(format!("ok: {} Markdown note(s) found", notes.len()));

        let mut broken_links = 0usize;
        for note in notes {
            let body = fs::read_to_string(self.root.join(&note.path))?;
            for link in extract_wiki_links(&body) {
                if self.resolve_note(&link)?.is_none() {
                    broken_links += 1;
                }
            }
        }
        if broken_links == 0 {
            report.push("ok: no broken wiki links found".into());
        } else {
            report.push(format!("warning: {broken_links} broken wiki link(s) found"));
        }
        Ok(report)
    }

    fn note_path_for_title(&self, title: &str) -> Result<PathBuf> {
        let slug = slugify(title);
        if slug.is_empty() {
            return Err(NeroError::Message(
                "title must contain at least one alphanumeric character".into(),
            ));
        }
        Ok(self.root.join(format!("{slug}.md")))
    }

    fn resolve_note(&self, query: &str) -> Result<Option<PathBuf>> {
        let raw = query.trim();
        if raw.is_empty() {
            return Ok(None);
        }

        let path_query = PathBuf::from(raw);
        let candidates = [
            path_query.clone(),
            PathBuf::from(format!("{raw}.md")),
            PathBuf::from(slugify(raw) + ".md"),
        ];
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
            let path_lower = note.to_string_lossy().to_lowercase();
            let stem_lower = note
                .path
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or_default()
                .to_lowercase();
            if path_lower == raw_lower || stem_lower == raw_stem || note.title.to_lowercase() == raw_lower {
                matches.push(note.path);
            }
        }
        Ok(matches.into_iter().next())
    }

    fn relative_path(&self, absolute: &Path) -> PathBuf {
        absolute
            .strip_prefix(&self.root)
            .unwrap_or(absolute)
            .to_path_buf()
    }
}

fn canonicalize_root(root: PathBuf) -> Result<PathBuf> {
    if root.exists() {
        Ok(fs::canonicalize(root)?)
    } else {
        Ok(root)
    }
}

fn collect_markdown_files(root: &Path, out: &mut Vec<NoteSummary>) -> Result<()> {
    fn visit(root: &Path, dir: &Path, out: &mut Vec<NoteSummary>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                visit(root, &path, out)?;
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let body = fs::read_to_string(&path)?;
            let title = title_from_markdown(&body)
                .or_else(|| path.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
                .unwrap_or_else(|| path.display().to_string());
            out.push(NoteSummary {
                path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                title,
            });
        }
        Ok(())
    }
    visit(root, root, out)
}

fn title_from_markdown(body: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let trimmed = line.trim_start();
        trimmed.strip_prefix("# ").map(|value| value.trim().to_owned())
    })
}

fn extract_wiki_links(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut links = Vec::new();
    let mut cursor = 0usize;
    while cursor + 3 < bytes.len() {
        if bytes[cursor] == b'[' && bytes[cursor + 1] == b'[' {
            if let Some(end) = body[cursor + 2..].find("]]") {
                let value = body[cursor + 2..cursor + 2 + end].trim();
                if !value.is_empty() {
                    links.push(value.to_owned());
                }
                cursor += end + 4;
                continue;
            }
        }
        cursor += 1;
    }
    links
}

fn slugify(title: &str) -> String {
    let mut result = String::new();
    let mut dash_pending = false;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            if dash_pending && !result.is_empty() {
                result.push('-');
            }
            result.extend(ch.to_lowercase());
            dash_pending = false;
        } else if !result.is_empty() {
            dash_pending = true;
        }
    }
    result.trim_end_matches('-').to_owned()
}

fn count_non_overlapping(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut offset = 0;
    while let Some(index) = haystack[offset..].find(needle) {
        count += 1;
        offset += index + needle.len();
        if offset >= haystack.len() {
            break;
        }
    }
    count
}

fn preview_for_query(body: &str, query: &str) -> String {
    let lower = body.to_lowercase();
    let Some(index) = lower.find(query) else {
        return body.lines().next().unwrap_or_default().trim().to_owned();
    };
    let original_start = index.saturating_sub(72);
    let original_end = (index + query.len() + 120).min(body.len());
    let mut preview = body.get(original_start..original_end).unwrap_or(body).replace('\n', " ");
    if original_start > 0 {
        preview.insert_str(0, "…");
    }
    if original_end < body.len() {
        preview.push('…');
    }
    preview.trim().to_owned()
}

fn chrono_like_date() -> String {
    // A dependency-free UTC date helper. It uses the Unix day number and a small
    // Gregorian conversion, keeping the core independent of a time crate for now.
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 86_400)
        .unwrap_or(0);
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
    fn slugifies_titles() {
        assert_eq!(slugify("Fourier Transform!"), "fourier-transform");
        assert_eq!(slugify("Hello,   World"), "hello-world");
    }

    #[test]
    fn creates_and_lists_notes() {
        let root = temp_workspace("create-list");
        let workspace = Workspace::init(&root).unwrap();
        workspace.create_note("Fourier Transform").unwrap();
        let notes = workspace.list_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].path, PathBuf::from("fourier-transform.md"));
    }

    #[test]
    fn finds_backlinks() {
        let root = temp_workspace("backlinks");
        let workspace = Workspace::init(&root).unwrap();
        workspace.create_note("Alpha").unwrap();
        workspace.create_note("Beta").unwrap();
        fs::write(root.join("alpha.md"), "# Alpha\n\nSee [[Beta]].\n").unwrap();
        let links = workspace.backlinks("Beta").unwrap();
        assert_eq!(links[0].path, PathBuf::from("alpha.md"));
    }
}
