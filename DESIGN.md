# Nero design notes

## Core rule

**The filesystem is the product.** SQLite, caches, indexes, and UI state are secondary.

## Workspace layout

```text
workspace/
├── notes/              # optional conventional folder; Nero can also use the root
├── daily/
├── assets/
└── .note/
    └── config.toml     # future configuration
```

For the MVP, notes are stored recursively anywhere under the workspace root as `.md` files.

## Note identity

A note is identified by its relative path. This keeps identity deterministic, Git-friendly, and inspectable.

A future alias/frontmatter layer can add friendly names without replacing path identity.

## Internal links

Syntax:

```text
[[Relative Name]]
[[relative/path.md]]
```

MVP link resolution is forgiving: compare exact relative paths first, then `.md` paths, then filename/title stems.

## Search

MVP search is intentionally simple and portable. The planned production search layer is SQLite FTS5:

```text
Markdown files → parser → SQLite FTS5 index
                     └── links table
```

The index must always be rebuildable from source files.

## Markdown and math

The rendering layer should use CommonMark/GFM-compatible Markdown with minimal extensions. The first custom syntax is wiki links (`[[...]]`).

Math remains ordinary Markdown text using standard TeX delimiters such as `$...$` and `$$...$$`.

Planned renderer stack:

- parser: Comrak
- GUI math: KaTeX
- terminal math: tui-math or a small internal fallback

## Frontends

### CLI

The CLI is scriptable and should never require a UI.

### TUI

The TUI is a keyboard-first reader/browser. Editing initially delegates to `$EDITOR` rather than embedding an editor.

### GUI

The GUI will arrive after the core and TUI stabilize. The preferred direction is Tauri with a small TypeScript/HTML/CSS frontend, primarily because web typography and KaTeX make high-quality document rendering inexpensive.

## Things we deliberately do not have yet

- accounts
- cloud sync
- collaboration
- block IDs
- arbitrary database properties
- kanban boards
- calendar databases
- AI agents
- plugin marketplace
- mobile clients

The burden of proof is on every feature.
