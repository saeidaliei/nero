# Usage

## Install or build from source

Nero uses the pinned Rust toolchain in `rust-toolchain.toml` (Rust 1.90) and the GUI uses Node 24.

From the repository root:

```bash
cargo test --workspace
cargo run -p nero -- --help
```

For a release build:

```bash
cargo build --workspace --release
```

## Create a workspace

```bash
nero init ~/notes
cd ~/notes
```

A workspace is just a directory containing Markdown files. Nero keeps disposable metadata under `.nero/`.

## Everyday commands

```text
nero init [path]             Create a workspace
nero new <title>             Create a Markdown note
nero list                    List notes
nero open <note>             Print a note
nero edit <note>             Open a note in $EDITOR
nero find <query>            Search notes
nero backlinks <note>        Find notes linking to a note
nero today                   Create/open today's daily note
nero render <note>           Render a note to HTML
nero reindex                 Refresh the search index
nero watch                   Watch Markdown files and refresh the index
nero doctor                  Check workspace health
```

## TUI

```bash
cargo run -p nero -- tui
```

Important keys:

- `j` / `k` — move through notes
- `/` — search
- `:` — command palette
- `b` — backlinks
- `c` — context
- `r` — recent notes
- `Tab` — cycle side panels
- `Ctrl-D` / `Ctrl-U` — page down/up
- `q` — quit

The TUI watches the workspace and refreshes when Markdown files change.

## GUI

The desktop UI lives in `gui/`. From that directory:

```bash
pnpm install
NERO_WORKSPACE=~/notes pnpm tauri dev
```

The GUI and TUI use the same `nero-core` document model. Editing always writes Markdown back to disk.

## Search

Search is backed by SQLite FTS5. The SQLite file is disposable:

```bash
nero reindex
rm .nero/index.sqlite
nero reindex
```

The second command reconstructs the index entirely from the workspace files.

## Editing

`nero edit` uses `$EDITOR`, falling back to `$VISUAL`. For example:

```bash
EDITOR=nvim nero edit "Fourier Transform"
```

The GUI provides an embedded Markdown editor, but the source on disk remains authoritative.

## Git versioning

Git is optional but useful for history and collaboration through ordinary Git hosting:

```bash
nero git init
nero git status
nero git snapshot "Notes checkpoint"
nero git remote add origin git@github.com:you/notes.git
nero git push
```

Use Git for **version history**, not as a replacement for encrypted disaster-recovery backups.

## Backups

Create a local snapshot:

```bash
nero backup create backup.zip
nero backup verify backup.zip
nero backup restore backup.zip ~/recovered-notes
```

Encrypted backup:

```bash
nero key generate
nero backup create --encrypt backup.age
nero backup verify backup.age
nero backup recovery-test backup.age
```

Keep another protected copy of the private identity. An encrypted backup is unrecoverable without it.

## Remote storage

Remote storage is provider-neutral and uses the user's existing `rclone` configuration:

```bash
nero storage add mega mega:nero-backups --encrypt
nero storage test mega
nero backup push mega
nero backup list mega
```

See [Storage](STORAGE.md) for the security model and remote naming rules.

## Health checks

```bash
nero doctor
```

`doctor` checks the `.nero` metadata area, note discovery, index refresh, broken wiki links, and configured storage profiles.
