use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use crate::{NeroError, Result, document::Document};

/// Render an ordinary Markdown source file as a standalone HTML document.
///
/// `base_dir` is used for a `<base>` URL so relative links and local images continue
/// to resolve against the Markdown file's original directory, rather than the temp
/// directory where the preview HTML is stored.
pub fn render_standalone_html(title: &str, source: &str, base_dir: Option<&Path>) -> String {
    let document = Document::parse(source);
    let escaped_title = escape_html(title);
    let base = base_dir
        .map(|path| format!("<base href=\"{}\">", escape_html(&file_url(path))))
        .unwrap_or_default();
    let metadata_fields = document
        .frontmatter
        .iter()
        .filter(|(key, _)| !key.eq_ignore_ascii_case("title"))
        .collect::<Vec<_>>();
    let metadata = if metadata_fields.is_empty() {
        String::new()
    } else {
        let rows = metadata_fields
            .iter()
            .map(|(key, value)| {
                format!(
                    "<div class=\"metadata-row\"><dt>{}</dt><dd>{}</dd></div>",
                    escape_html(key),
                    escape_html(value)
                )
            })
            .collect::<String>();
        format!(
            "<aside class=\"metadata\" aria-label=\"Document metadata\"><details open><summary>Metadata <span>{} fields</span></summary><dl>{rows}</dl></details></aside>",
            metadata_fields.len()
        )
    };

    // A title is always shown in the viewer chrome. If the Markdown body starts with
    // the same H1, remove only that exact leading heading to avoid displaying it twice.
    let heading = format!("<h1>{escaped_title}</h1>");
    let body = document
        .html
        .trim_start()
        .strip_prefix(&heading)
        .map(str::trim_start)
        .unwrap_or(document.html.trim());

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
{base}
<title>{escaped_title} · Nero</title>
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.11.1/styles/github.min.css">
<link rel="stylesheet" media="(prefers-color-scheme: dark)" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.11.1/styles/github-dark.min.css">
<style>
:root {{ color-scheme: light dark; --page: #ffffff; --ink: #24292f; --muted: #667085; --line: #e5e7eb; --panel: #f6f8fa; --link: #0969da; --code: #f6f8fa; }}
@media (prefers-color-scheme: dark) {{ :root {{ --page: #0d1117; --ink: #e6edf3; --muted: #9da7b3; --line: #30363d; --panel: #161b22; --link: #58a6ff; --code: #161b22; }} }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; background: var(--page); color: var(--ink); font: 16px/1.7 Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }}
.topbar {{ position: sticky; top: 0; z-index: 2; display: flex; gap: 12px; align-items: center; padding: 12px max(20px, calc((100vw - 1100px) / 2)); border-bottom: 1px solid var(--line); background: var(--panel); color: var(--muted); font-size: 13px; }}
.brand {{ color: var(--ink); font-weight: 750; letter-spacing: .08em; }}
.layout {{ width: min(850px, calc(100% - 40px)); margin: 40px auto 80px; }}
.content {{ min-width: 0; }}
.document-title {{ margin: 0 0 28px; padding-bottom: 18px; border-bottom: 1px solid var(--line); font-size: clamp(2rem, 4vw, 3rem); line-height: 1.15; letter-spacing: -.035em; }}
.metadata {{ margin: 0 0 28px; padding: 14px 16px; border: 1px solid var(--line); border-radius: 10px; background: var(--panel); }}
.metadata summary {{ cursor: pointer; font-weight: 650; }} .metadata summary span {{ margin-left: 8px; color: var(--muted); font-size: 12px; font-weight: 400; }}
.metadata dl {{ margin: 12px 0 0; }} .metadata-row {{ display: grid; grid-template-columns: minmax(100px, 180px) minmax(0, 1fr); gap: 14px; padding: 5px 0; }}
.metadata dt {{ color: var(--muted); overflow-wrap: anywhere; }} .metadata dd {{ margin: 0; overflow-wrap: anywhere; }}
.markdown-body {{ min-width: 0; overflow-wrap: anywhere; }}
.markdown-body h1, .markdown-body h2, .markdown-body h3, .markdown-body h4 {{ margin-top: 1.7em; margin-bottom: .55em; line-height: 1.25; letter-spacing: -.02em; }}
.markdown-body h1 {{ font-size: 2em; }} .markdown-body h2 {{ font-size: 1.55em; border-bottom: 1px solid var(--line); padding-bottom: .3em; }}
.markdown-body p, .markdown-body ul, .markdown-body ol, .markdown-body blockquote {{ margin: 1em 0; }}
a {{ color: var(--link); text-underline-offset: .18em; }}
img, video {{ max-width: 100%; height: auto; border-radius: 6px; }}
pre {{ overflow-x: auto; padding: 16px; border-radius: 8px; background: var(--code); border: 1px solid var(--line); }}
code {{ font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: .9em; }} :not(pre) > code {{ padding: .15em .35em; border-radius: 4px; background: var(--code); }}
table {{ width: 100%; border-collapse: collapse; display: block; overflow-x: auto; }} th, td {{ padding: 8px 12px; border: 1px solid var(--line); text-align: left; }} th {{ background: var(--panel); }}
blockquote {{ padding: 0 1em; color: var(--muted); border-left: 3px solid var(--line); }}
hr {{ border: 0; border-top: 1px solid var(--line); margin: 2em 0; }}
.task-list-item {{ list-style: none; }} .task-list-item input {{ margin-right: .5em; }}
.math, .math-display {{ overflow-x: auto; }}
@media (max-width: 600px) {{ .layout {{ width: calc(100% - 28px); margin-top: 24px; }} .metadata-row {{ grid-template-columns: 1fr; gap: 0; }} .topbar {{ padding-left: 14px; padding-right: 14px; }} }}
</style>
<script defer src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.11.1/highlight.min.js"></script>
<script>document.addEventListener('DOMContentLoaded', () => {{ if (window.hljs) window.hljs.highlightAll(); }});</script>
<script>window.MathJax = {{ tex: {{ inlineMath: [['$', '$'], ['\\(', '\\)']], displayMath: [['$$', '$$'], ['\\[', '\\]']] }}, options: {{ skipHtmlTags: ['script','noscript','style','textarea','pre','code'] }} }};</script>
<script defer src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-chtml.js"></script>
</head>
<body>
<header class="topbar"><span class="brand">NERO</span><span>Simple but powerful · Markdown viewer</span></header>
<div class="layout">
<main class="content"><h1 class="document-title">{escaped_title}</h1>{metadata}<article class="markdown-body">{body}</article></main>
</div>
</body>
</html>
"#
    )
}

/// Create a persistent temporary HTML preview file so a separately launched browser
/// can read it even after the CLI/TUI command returns.
pub fn write_browser_preview(
    title: &str,
    source: &str,
    base_dir: Option<&Path>,
) -> Result<PathBuf> {
    let temp_root = std::env::temp_dir();
    let staging_dir = tempfile::Builder::new()
        .prefix("nero-preview-")
        .tempdir_in(&temp_root)?;
    #[cfg(unix)]
    let owner_uid = {
        use std::os::unix::fs::MetadataExt;
        Some(fs::metadata(staging_dir.path())?.uid())
    };
    #[cfg(not(unix))]
    let owner_uid: Option<u32> = None;
    cleanup_stale_previews(&temp_root, owner_uid);

    // `tempfile` creates a private directory on Unix. Keep it after returning so the
    // browser can load the file after Nero exits; stale directories are cleaned later.
    let preview_dir = staging_dir.keep();
    let path = preview_dir.join("index.html");
    let html = render_standalone_html(title, source, base_dir);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(html.as_bytes())?;
    file.flush()?;
    Ok(path)
}

/// Open a file or generated preview in the operating system's default browser.
pub fn open_path_in_browser(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command.arg(path);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    command.spawn().map(|_| ()).map_err(|error| {
        NeroError::Message(format!(
            "could not open {} in the default browser: {error}",
            path.display()
        ))
    })
}

fn cleanup_stale_previews(directory: &Path, owner_uid: Option<u32>) {
    #[cfg(not(unix))]
    let _ = owner_uid;

    const MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().is_ok_and(|kind| kind.is_dir())
            || !path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with("nero-preview-"))
        {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Some(uid) = owner_uid
                && metadata.uid() != uid
            {
                continue;
            }
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if now
            .duration_since(modified)
            .is_ok_and(|age| age.as_secs() > MAX_AGE_SECS)
        {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn file_url(path: &Path) -> String {
    let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let normalized = absolute.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    let normalized = normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_owned();
    let prefix = if normalized.starts_with("//") {
        "file:"
    } else if normalized.starts_with('/') {
        "file://"
    } else {
        "file:///"
    };
    let mut encoded = String::with_capacity(normalized.len());
    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    let url = format!("{prefix}{encoded}");
    if url.ends_with('/') {
        url
    } else {
        format!("{url}/")
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_html_renders_markdown_and_metadata_without_repeating_title() {
        let source = "---\ntitle: Release Plan\ndue: 2026-10-20\ntags: project, release\n---\n\n# Release Plan\n\n## Checklist\n\n- [ ] Package desktop apps\n";
        let html = render_standalone_html("Release Plan", source, None);
        assert!(html.contains("<title>Release Plan · Nero</title>"));
        assert!(html.contains("<dt>due</dt><dd>2026-10-20</dd>"));
        assert!(html.contains("<dt>tags</dt><dd>project, release</dd>"));
        assert!(html.contains("<h2>Checklist</h2>"));
        assert_eq!(html.matches("<h1").count(), 1);
        assert!(html.contains("highlightAll()"));
        assert!(html.contains("window.MathJax"));
        assert!(html.contains("Package desktop apps"));
    }

    #[test]
    fn standalone_html_escapes_titles_and_frontmatter() {
        let source = "---\ntitle: <script>alert(1)</script>\ncustom: <b>unsafe</b>\n---\n\nText.";
        let html = render_standalone_html("<script>alert(1)</script>", source, None);
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;b&gt;unsafe&lt;/b&gt;"));
    }

    #[test]
    fn file_url_encodes_spaces_and_unicode_path_components() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a file").join("naïve folder");
        fs::create_dir_all(&path).unwrap();
        let encoded = file_url(&path);
        assert!(encoded.starts_with("file://"));
        assert!(encoded.contains("a%20file/"));
        assert!(encoded.contains("na%C3%AFve%20folder/"));
    }

    #[test]
    fn browser_preview_is_written_to_a_unique_temporary_file() {
        let first = write_browser_preview("Preview", "# Preview\n\nText.", None).unwrap();
        let second = write_browser_preview("Preview", "# Preview\n\nText.", None).unwrap();
        assert_ne!(first, second);
        let html = fs::read_to_string(&first).unwrap();
        assert!(html.contains("<h1 class=\"document-title\">Preview</h1>"));
        let first_dir = first.parent().map(Path::to_path_buf);
        let second_dir = second.parent().map(Path::to_path_buf);
        let _ = fs::remove_file(&first);
        let _ = fs::remove_file(&second);
        if let Some(directory) = first_dir {
            let _ = fs::remove_dir(directory);
        }
        if let Some(directory) = second_dir {
            let _ = fs::remove_dir(directory);
        }
    }
}
