# Nero

> A small place for your thoughts.
>
> Plain Markdown. Local files. Fast search. Beautiful reading.

Nero is a local-first knowledge tool built around a deliberately boring idea:
**Markdown files are the source of truth.** Everything else is an interface or a cache.

The plan is one core with three interfaces:

- `nero` — CLI
- `nero-tui` — terminal UI
- `nero-gui` — desktop GUI backed by the same core

## Design principles

1. **Files first.** A note is a normal `.md` file you can edit with Vim, Emacs, VS Code, or anything else.
2. **Small surface area.** Nero should solve notes, linking, search, and reading exceptionally well before adding anything else.
3. **Unix-friendly.** Commands should compose with other tools, and machine-readable output should be easy to add.
4. **No lock-in.** Deleting Nero must never make your notes unusable.
5. **Beautiful by restraint.** Reading is the primary experience; chrome is secondary.

## Current milestone

The current snapshot is the **1.0 remote-storage milestone**. It includes:

- Comrak-powered Markdown rendering
- dollar and LaTeX-style math parsing
- `[[wiki links]]` parsing
- lightweight frontmatter extraction
- task counts from Markdown checkboxes
- SQLite FTS5 indexing
- incremental index refreshes
- indexed backlinks
- cross-platform file watching
- `render`, `reindex`, and `watch` CLI commands
- a Ratatui TUI reader with search, command palette, backlinks, context/recent panels, terminal math, and live file reload
- a Tauri desktop GUI with the same core document/search/link APIs
- KaTeX typesetting for Comrak math nodes in the GUI
- an embedded Markdown editor with syntax highlighting, split preview, selection preservation, and smart list continuation
- native image import into a workspace `assets/` directory and local image rendering in previews
- portable local backup archives with manifests, SHA-256 verification, and safe restore
- streamed age-compatible encrypted backups with a private identity stored outside the workspace
- key management commands for the default backup identity
- provider-neutral remote backup storage through the external `rclone` command
- named storage targets with an explicit encrypted-backup policy
- remote backup listing, upload, download, and restore commands
- Git helpers for initialization, snapshots, remotes, push, and pull
- a full decrypt/restore/recovery-test path

The index lives at `.nero/index.sqlite` and is disposable. Delete it and Nero can rebuild it from the Markdown files.

## Workspace

```text
~/notes/
├── projects/
├── ideas/
├── daily/
├── assets/
├── *.md
└── .nero/
    └── index.sqlite
```

Any `.md` file beneath the workspace is a note. Directories are organizational, not database objects.

## Example

```bash
nero init ~/notes
cd ~/notes
nero new "Fourier Transform"
nero edit "Fourier Transform"
nero find "frequency"
nero backlinks "Fourier Transform"
nero render "Fourier Transform"
nero reindex
nero today
nero watch
```

A note can contain ordinary Markdown, tasks, internal links, and math:

```markdown
---
tags: math, dsp
---

# Fourier Transform

For a function $f(t)$:

$$
F(\omega) = \int_{-\infty}^{\infty} f(t)e^{-i\omega t}\,dt
$$

- [ ] Read the next chapter
- [x] Understand the transform pair

See also [[Signal Processing]].
```

`nero render "Fourier Transform"` returns the generated HTML. The GUI uses the `data-math-style` nodes emitted by Comrak and typesets them with KaTeX.

## Architecture

```text
                         ┌────────────────────┐
                         │     Markdown       │
                         │       files        │
                         └─────────┬──────────┘
                                   │
                                   ▼
                         ┌────────────────────┐
                         │     nero-core      │
                         │ markdown / links   │
                         │ storage / search   │
                         │ file watching      │
                         └──────┬───────┬─────┘
                                │       │
                         ┌──────┘       └───────┐
                         ▼                      ▼
                     SQLite FTS5              UIs
                                           ┌────┼────┐
                                           ▼    ▼    ▼
                                          CLI  TUI  GUI
```

The core deliberately does not know about the TUI or GUI. They consume the same APIs and the same files.

## Why SQLite?

SQLite is an implementation detail, not the document format. FTS5 gives Nero fast full-text search while keeping notes as ordinary files. The index is always rebuildable.

## Why Comrak?

Nero is not inventing its own Markdown dialect. CommonMark/GFM-compatible Markdown stays the default, while Nero adds only the small amount of syntax that makes local linking useful.

## Development

Use a current stable Rust toolchain. The workspace expects Rust 2024 edition.

```bash
cargo test --workspace
cargo run -p nero -- --help
cargo run -p nero -- tui
```

The original scaffolding environment used to create this repository did not include Rust, so this iteration has not been compiled locally in that environment. Dependency choices were checked against current upstream crate documentation.

## TUI

Run the terminal reader with:

```bash
cargo run -p nero -- tui
```

The TUI is deliberately keyboard-first:

- `j` / `k` — move through notes
- `/` — search
- `:` — command palette
- `b` — backlinks
- `c` — context
- `r` — recent notes
- `Tab` — cycle side panels
- `Ctrl-D` / `Ctrl-U` — page down/up
- `q` — quit

The TUI watches the workspace for Markdown changes and refreshes the current note and search index automatically.


## 0.7 design note

Nero's document model remains the single source of truth for all frontends. The desktop GUI is a thin Tauri shell around the same core APIs used by the CLI and TUI; Markdown remains ordinary files and `.nero/index.sqlite` remains disposable.


## Backup and recovery

Nero keeps the live workspace as plain files. Backups are separate snapshots and can optionally be encrypted with the age format:

```bash
nero key generate
nero backup create --encrypt
nero backup verify backup.age
nero backup recovery-test backup.age
nero backup restore backup.age /tmp/nero-restore
```

The private identity lives outside the workspace by default. Keep another protected copy of it; without the identity, encrypted backups cannot be recovered. See `BACKUP.md`, `RECOVERY.md`, and `SECURITY.md` for the full model.

Git is a complementary versioning layer:

```bash
nero git init
nero git snapshot "Notes checkpoint"
nero git remote add origin git@github.com:you/notes.git
nero git push
```

## GUI

The desktop frontend lives in `gui/` and uses Tauri 2 with a deliberately small vanilla TypeScript/CSS surface. It reads and searches notes through `nero-core`; it does not invent a second document format.

Development:

```bash
cd gui
pnpm install
NERO_WORKSPACE=~/notes pnpm tauri dev
```

The workspace can also be discovered from the current directory when `.nero/` exists. The editor keeps Markdown as the source of truth. Split mode derives a live preview, images are ordinary Markdown references, and imported images are copied into `assets/`.
