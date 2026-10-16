# Markdown format

Nero intentionally stays close to ordinary Markdown. There is no block-editor storage format.

## Standard Markdown

Common Markdown is expected to work: headings, emphasis, links, lists, task checkboxes, blockquotes, tables, strikethrough, and fenced code blocks.

## Math

Comrak handles both dollar and LaTeX-style delimiters:

```markdown
Inline: $E = mc^2$

Display:

$$
\int_{-\infty}^{\infty} f(x) e^{-i\omega x} \, dx
$$

Or LaTeX delimiters:
\(x^2 + y^2\)
\[a^2 + b^2 = c^2\]
```

The GUI typesets Comrak's math nodes with KaTeX. The TUI uses a deliberate terminal-friendly Unicode approximation rather than trying to recreate a browser math layout engine.

## Wiki links

The only major Nero-specific syntax is:

```markdown
[[Fourier Transform]]
[[Signal Processing|DSP notes]]
[[Fourier Transform#Definition]]
```

Resolution is deterministic: note-relative path, workspace path, title, then filename stem. The `#fragment` is preserved so the GUI can navigate to a heading.

## Frontmatter

Nero creates new notes with frontmatter by default. `nero new "Fourier Transform"` starts a file like this (the date below is illustrative; Nero writes the current local date):

```yaml
---
title: Fourier Transform
date: 2026-10-11
---

# Fourier Transform
```

Daily notes created by `nero today` also include `type: daily`. Existing notes are not rewritten automatically; add frontmatter when you want metadata on an older file.

Frontmatter is deliberately lightweight. Nero reads simple `key: value` pairs rather than implementing all of YAML, so use plain scalar values and comma-separated tags:

```yaml
---
title: Prepare Project Release
date: 2026-10-11
due: 2026-10-16
tags: project, release
status: in-progress
type: task
priority: high
project: Nero
---
```

Common useful fields include `title` (display title), `date` (creation or note date), `due` (optional due date), `tags`, `status`, `type`, `priority`, `project`, `author`, `source`, and `aliases`. Apart from `title` and `due`, these are user-defined conventions: Nero preserves/exposes the key-value metadata but does not impose a schema or special behavior on them. `due: YYYY-MM-DD` makes the note appear in `nero today` on that date and thereafter until you change/remove the field. No background process is required. Due dates are optional; don't add one to notes that have no deadline.

See the complete [examples directory on GitHub](https://github.com/saeidaliei/nero/tree/main/examples) for research, project-task, daily-log, and math-note patterns.

## Tasks

Regular Markdown task checkboxes are the task model:

```markdown
- [ ] unfinished
- [x] finished
```

Task counts are derived from the source. There is no separate task database.

## Images and assets

Images are ordinary Markdown references:

```markdown
![diagram](assets/fourier.png)
```

The GUI's native image import copies the source image into the workspace `assets/` directory and inserts a relative Markdown reference. Remote `http(s)` images are not copied into the workspace.

## Source of truth

Editing the Markdown file outside Nero is supported and expected. The file watcher refreshes the index/UI when files change.

## Viewing arbitrary Markdown

`nero view <file>` opens any Markdown file in the standalone terminal reader, including files outside a Nero workspace. `nero FILE.md` is a shorthand for the same reader. Use `nero view --browser <file>` or `nero browse <note|ID>` to create and open a styled HTML preview in the default browser. Browser previews display optional frontmatter, preserve relative asset paths with a local base URL, and render the Markdown body with the shared Comrak engine. Preview HTML lives in a private temporary directory and is cleaned up after seven days. Highlight.js and MathJax are loaded from CDNs for browser code highlighting and math typesetting, respectively, so those enhancements require an internet connection.
