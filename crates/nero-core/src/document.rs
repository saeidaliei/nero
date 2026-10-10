use std::collections::BTreeMap;

use comrak::{Options, markdown_to_html};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskCounts {
    pub open: usize,
    pub done: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLink {
    /// The target as written inside `[[...]]`, excluding the optional display label.
    pub target: String,
    /// An optional display label from `[[target|label]]`.
    pub label: Option<String>,
}

impl WikiLink {
    pub fn display_label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// The exact Markdown source on disk.
    pub source: String,
    pub title: String,
    pub frontmatter: BTreeMap<String, String>,
    pub wiki_links: Vec<WikiLink>,
    pub tasks: TaskCounts,
    pub math_count: usize,
    /// Comrak-rendered HTML. This remains a derived representation.
    pub html: String,
}

impl Document {
    pub fn parse(body: &str) -> Self {
        parse(body)
    }
}

pub fn parse(body: &str) -> Document {
    let frontmatter = parse_frontmatter(body);
    let title = frontmatter
        .get("title")
        .cloned()
        .or_else(|| first_h1(body))
        .unwrap_or_else(|| "Untitled".to_owned());

    let wiki_links = extract_wiki_links(body);
    let tasks = count_tasks(body);
    let math_count = count_math(body);
    let html = render_html(body);

    Document {
        source: body.to_owned(),
        title,
        frontmatter,
        wiki_links,
        tasks,
        math_count,
        html,
    }
}

pub fn render_html(body: &str) -> String {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.math_dollars = true;
    options.extension.math_latex = true;
    options.extension.wikilinks_title_after_pipe = true;
    options.extension.front_matter_delimiter = Some("---".to_owned());
    options.render.r#unsafe = false;
    options.render.escape = true;
    markdown_to_html(body, &options)
}

fn first_h1(body: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let line = line.trim_start();
        line.strip_prefix("# ").map(|value| value.trim().to_owned())
    })
}

fn parse_frontmatter(body: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let mut lines = body.lines();
    if lines.next().map(str::trim) != Some("---") {
        return values;
    }

    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        values.insert(key.to_owned(), value.to_owned());
    }
    values
}

fn extract_wiki_links(body: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();
    let mut cursor = 0usize;

    while let Some(start_rel) = body[cursor..].find("[[") {
        let start = cursor + start_rel + 2;
        let Some(end_rel) = body[start..].find("]]") else {
            break;
        };
        let raw = body[start..start + end_rel].trim();
        let (target, label) = match raw.split_once('|') {
            Some((target, label)) => (target.trim(), Some(label.trim().to_owned())),
            None => (raw, None),
        };
        if !target.is_empty() {
            links.push(WikiLink {
                target: target.to_owned(),
                label,
            });
        }
        cursor = start + end_rel + 2;
    }

    // Keep document metadata deterministic while preserving first-seen order for distinct links.
    let mut deduped = Vec::with_capacity(links.len());
    for link in links {
        if !deduped
            .iter()
            .any(|existing: &WikiLink| existing.target.eq_ignore_ascii_case(&link.target))
        {
            deduped.push(link);
        }
    }
    deduped
}

fn count_tasks(body: &str) -> TaskCounts {
    let mut tasks = TaskCounts::default();
    for line in body.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("- [") else {
            continue;
        };
        let Some(marker) = rest.chars().next() else {
            continue;
        };
        if !rest.as_bytes().get(1).is_some_and(|value| *value == b']') {
            continue;
        }
        match marker {
            'x' | 'X' => tasks.done += 1,
            ' ' => tasks.open += 1,
            _ => {}
        }
    }
    tasks
}

fn count_math(body: &str) -> usize {
    let mut count = 0;
    let mut cursor = 0usize;
    while let Some(index) = body[cursor..].find("$$") {
        count += 1;
        cursor += index + 2;
    }
    let display_pairs = count / 2;
    let inline_dollars = count_dollar_inline(body).saturating_sub(display_pairs * 2);
    display_pairs + inline_dollars
}

fn count_dollar_inline(body: &str) -> usize {
    let mut count = 0;
    let mut cursor = 0usize;
    while let Some(index) = body[cursor..].find('$') {
        count += 1;
        cursor += index + 1;
    }
    count / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_raw_html_as_text() {
        let document = parse("<script>alert(1)</script>\n");
        assert!(document.html.contains("&lt;script&gt;"));
        assert!(!document.html.contains("<script>"));
    }

    #[test]
    fn parses_title_links_labels_tasks_and_math() {
        let body = "---\ntitle: Fourier Notes\ntags: math, dsp\n---\n\n# Ignored\n\n- [ ] Open\n- [x] Done\n\nSee [[Signal Processing|DSP]]. And [[Complex Analysis]].\n\n$$x^2$$";
        let document = parse(body);
        assert_eq!(document.source, body);
        assert_eq!(document.title, "Fourier Notes");
        assert_eq!(document.frontmatter["tags"], "math, dsp");
        assert_eq!(document.wiki_links[0].target, "Signal Processing");
        assert_eq!(document.wiki_links[0].display_label(), "DSP");
        assert_eq!(document.tasks.open, 1);
        assert_eq!(document.tasks.done, 1);
        assert_eq!(document.math_count, 1);
        assert!(document.html.contains("data-wikilink=\"true\""));
    }
}
