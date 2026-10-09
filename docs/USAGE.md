# Usage

## Install or build from source

Nero uses the Rust toolchain pinned in `rust-toolchain.toml` (Rust 1.90). Clone the repository and enter its root:

```bash
git clone https://github.com/saeidaliei/nero.git
cd nero
```

Run the tests and build the CLI/TUI binary:

```bash
cargo test --workspace
cargo build --release -p nero
```

Install the binary into Cargo's user bin directory (`~/.cargo/bin`):

```bash
cargo install --path crates/nero --force
```

Make sure `~/.cargo/bin` is on your `PATH`. The desktop GUI is built separately; see [GUI](GUI.md).

## Create and select a workspace

Initialize a workspace and set it as the default once:

```bash
nero init ~/notes
nero workspace set ~/notes
```

Now run Nero from any directory without `cd ~/notes` first:

```bash
nero new "An idea"
```

The everyday commands are, for example:

```bash
nero list
nero find "something"
nero edit "An idea"
nero today
nero tui
```


For several workspaces, initialize each one, register names, and choose a default:

```bash
nero workspace add personal ~/notes
nero init ~/work/research-notes
nero workspace add research ~/work/research-notes
nero workspace use research
nero workspace list
nero -w personal tui
nero --workspace ~/notes find "equation"
```

See [Workspace configuration](WORKSPACES.md) for selection precedence, environment overrides, and config paths.

## Everyday CLI commands

```text
nero init [path]                 Create a workspace
nero workspace set <path>        Set the default workspace
nero workspace add <name> <path> Register a named workspace
nero workspace use <name>        Select a named default
nero workspace list              List configured workspaces
nero new <title>                 Create a Markdown note
nero list                        List notes
nero open <note>                 Print a note
nero edit <note>                 Open a note in $EDITOR or $VISUAL
nero find <query>                Search notes
nero backlinks <note>            Find notes linking to a note
nero today                       Create/locate today's daily note
nero render <note>               Render a note to HTML
nero reindex                     Refresh the search index
nero watch                       Watch Markdown files and refresh the index
nero doctor                      Check workspace health
```

Run `nero --help` for the full command set, including Git and backup commands.

## TUI

```bash
nero tui
```

The TUI is keyboard-first:

- `j` / `k` — move through notes
- `e` — edit the selected note in `$EDITOR`, then return to Nero
- `/` — search
- `:` — command palette (`:edit` also edits the selected note)
- `b` — backlinks
- `c` — context
- `r` — recent notes
- `Tab` — cycle side panels
- `Ctrl-D` / `Ctrl-U` — page down/up
- `q` — quit

Set `$EDITOR` or `$VISUAL` first. In Fish, for example:

```fish
set -Ux EDITOR nvim
```

Nero restores normal terminal mode while the editor runs, then reloads the note and index on return.

## GUI

The GUI lives in `gui/`. Install its dependencies, then run it against the chosen workspace:

```bash
cd gui
npm install
NERO_WORKSPACE="$HOME/notes" npm run tauri:dev
```

For normal usage, `nero gui` launches the installed GUI and passes it the selected workspace. See [GUI](GUI.md) for platform packages and build steps.

## Search and index

Search uses SQLite FTS5. The database at `.nero/index.sqlite` is disposable and can be rebuilt from Markdown:

```bash
nero reindex
rm .nero/index.sqlite
nero reindex
```

Do not delete the Markdown files; only the index is a cache.

## Git, backups, and remote storage

Use Git for version history:

```bash
nero git init
nero git snapshot "Notes checkpoint"
nero git remote add origin git@github.com:you/notes.git
nero git push
```

Create a portable backup and verify it before relying on it:

```bash
nero backup create backup.zip
nero backup verify backup.zip
nero backup restore backup.zip ~/recovered-notes
```

Encrypted backups require a private identity that must be protected separately:

```bash
nero key generate
nero backup create --encrypt backup.age
nero backup recovery-test backup.age
```

Remote backup targets use the user's existing `rclone` configuration. See [Backups](BACKUP.md), [Recovery](RECOVERY.md), [Security](SECURITY.md), and [Storage](STORAGE.md).
