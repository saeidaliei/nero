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

A small YAML-like frontmatter block can be used for lightweight metadata:

```yaml
---
tags: math, dsp
status: reading
---
```

Nero extracts simple `key: value` pairs. It does not attempt to become a general YAML database.

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
