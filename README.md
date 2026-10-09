# Nero

> Plain Markdown. Local files. Fast search. Beautiful reading. No lock-in.

Nero is a local-first note and knowledge system with a CLI, keyboard-driven terminal UI, and desktop GUI. Notes are ordinary Markdown files. The search index is rebuildable; workspace preferences and storage profiles are kept separately from note content.

## Features

- **Markdown first:** standard Markdown, frontmatter, checklists, code blocks, images, and LaTeX-style mathematics.
- **Wiki links:** link notes with `[[Note Name]]`, optional labels, and heading fragments.
- **Search and context:** ranked full-text search, backlinks, recent notes, and a contextual reader.
- **Three interfaces:** CLI, Ratatui TUI, and Tauri desktop GUI share the same Rust core.
- **Edit your way:** use your preferred `$EDITOR` in the CLI/TUI or the embedded GUI editor.
- **Portable storage:** notes remain files on disk; SQLite FTS5 is a disposable search index.
- **Versioning and recovery:** optional Git helpers, portable ZIP backups, age-encrypted backups, and rclone-backed remote storage.
- **Local-first by design:** no account or cloud service is required to use Nero.

## Install from source

Requirements: Git and Rust 1.90 (pinned in `rust-toolchain.toml`). From a checkout of the repository, build and install the CLI/TUI binary into Cargo's user bin directory:

```bash
git clone https://github.com/saeidaliei/nero.git
cd nero
cargo install --path crates/nero --force
```

Or build directly from the repository:

```bash
cargo build --release -p nero
./target/release/nero --help
```

The desktop GUI is built separately. See [GUI setup](docs/GUI.md) for platform prerequisites and build instructions. Prebuilt desktop packages, when available, are published on the project's GitHub Releases page.

## Quick start

Create a workspace and set it as your default once:

```bash
nero init ~/notes
nero workspace set ~/notes
```

Now Nero can find that workspace from **any directory**:

```bash
nero new "Welcome to Nero"
nero edit "Welcome to Nero"
nero find "welcome"
nero today
nero tui
nero gui
```

You do not need to `cd ~/notes` first. To select another workspace for a single command, use `-w` or `--workspace`:

```bash
nero init ~/work/research-notes
nero workspace add research ~/work/research-notes
nero workspace use research
nero -w ~/notes list
nero --workspace research tui
nero workspace list
```

Selection order is: explicit `--workspace`/`-w`, `NERO_WORKSPACE`, a workspace discovered from the current directory and its ancestors, then the saved default when you are outside any workspace. See [Workspace configuration](docs/WORKSPACES.md).

## Common commands

| Command | Purpose |
|---|---|
| `nero new "Note title"` | Create a Markdown note |
| `nero list` | List notes |
| `nero open "Note title"` | Print a note to the terminal |
| `nero edit "Note title"` | Edit with `$EDITOR` or `$VISUAL` |
| `nero find "query"` | Search note contents |
| `nero backlinks "Note title"` | Find notes linking to a note |
| `nero today` | Create or locate today's daily note |
| `nero tui` | Launch the terminal interface |
| `nero gui` | Launch the installed desktop interface |
| `nero doctor` | Check workspace health and links |
| `nero backup create --encrypt` | Create an encrypted backup |

Run `nero --help` for the complete command list. The TUI uses `j`/`k` to navigate, `/` to search, `:` for commands, `e` to edit the selected note, and `q` to quit.

## Notes are ordinary Markdown

```markdown
---
title: Fourier Transform
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

The workspace is just a directory. Nero stores its rebuildable search index and workspace-local settings (such as storage profiles) in `.nero/`; deleting the index does not delete notes. The default/named workspace preference is stored in your user config outside the workspace. Read the [Markdown guide](docs/MARKDOWN.md) for supported syntax and asset conventions.

## Backup, encryption, and storage

Nero separates version history from disaster recovery:

- **Git** is for note history, diffs, and collaboration through normal Git hosting.
- **Backups** are portable snapshots that can be verified and restored independently.
- **age encryption** protects backup snapshots; keep the private identity safe because encrypted backups cannot be recovered without it.
- **rclone** can send backup artifacts to supported remote storage providers without putting provider credentials in Nero's configuration.

Start with [Backup and recovery](docs/BACKUP.md), [Security](docs/SECURITY.md), and [Remote storage](docs/STORAGE.md).

## Documentation

- [Getting started and usage](docs/USAGE.md)
- [Workspace configuration](docs/WORKSPACES.md)
- [Markdown, math, tasks, and links](docs/MARKDOWN.md)
- [Desktop GUI](docs/GUI.md)
- [Architecture](docs/DESIGN.md)
- [Development and testing](docs/DEVELOPMENT.md)
- [Backup and recovery](docs/BACKUP.md) · [Encryption/security](docs/SECURITY.md) · [Remote storage](docs/STORAGE.md)
- [CI and releases](docs/CI.md) · [Roadmap](docs/ROADMAP.md) · [Changelog](docs/CHANGELOG.md)

The documentation site is built from these canonical Markdown files and deployed to [saeidaliei.github.io/nero](https://saeidaliei.github.io/nero/) by GitHub Actions.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --release
```

The Tauri app has its own Cargo manifest and is built from `gui/`; see [development instructions](docs/DEVELOPMENT.md). Contributions should keep Markdown as the source of truth and avoid adding a second parser or a proprietary note format.

## Project status

Version **1.0.0**. Workspace selection is configurable globally. Desktop and Telegram reminders are not yet implemented; their proposed design is tracked in [the reminders document](docs/REMINDERS.md).
