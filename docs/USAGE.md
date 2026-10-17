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

Create your first workspace under the default `~/.nero/workspaces` directory:

```bash
nero workspace create personal
nero workspace use personal
```

Run the second command if you had already configured an older default. Then run Nero from any directory without `cd` into the workspace first:

```bash
nero new "An idea"
```

The everyday commands are, for example:

```bash
nero list                    # table with persistent note IDs and timestamps
nero list -l                  # also show frontmatter metadata (-l is short for --long)
nero find "something"
nero edit "An idea"           # title/path also work
nero edit 3                   # use the ID shown by `nero list`
nero delete 3                 # IDs work for delete/open/render/backlinks too
nero today
nero config show             # resolved config, keys, remotes, and rclone setup
nero tui
```


For several workspaces, create named directories under the same workspace home and choose the default:

```bash
nero workspace create research
nero workspace use research
nero workspace list
nero -w personal tui
nero --workspace personal find "equation"
```

Use `nero home` to see the application home and important paths. Set `NERO_HOME` to an absolute path to relocate the entire app home; see [Application home](APP_HOME.md) and [Workspace configuration](WORKSPACES.md). See [Due dates](DUE_DATES.md) for the tiny frontmatter-based due-date behavior.

## Everyday CLI commands

```text
nero init [path]                 Create a workspace
nero home                        Show the application home and data directories
nero home path                   Print only the application home path
nero workspace home              Show the shared workspace parent
nero workspace home set <path>  Change the workspace parent
nero workspace create <name>    Create a workspace under that parent
nero workspace set <path>        Set the default workspace
nero workspace add <name> <path> Register an existing workspace
nero workspace use <name>        Select a named default
nero workspace list              List configured workspaces
nero new <title>                 Create a Markdown note
nero list [-l|--long]            List notes with stable IDs and timestamps; -l adds metadata
nero open <note|ID>              Print a note; a numeric ID or #ID works
nero view <file|note|ID|->       Read Markdown in the terminal; --browser opens the browser
nero FILE.md                     Shorthand for `nero view FILE.md`
nero browse <file|note|ID>        Open a styled HTML preview in the default browser
nero edit <note|ID>              Open a note in $EDITOR or $VISUAL
nero config show                 Show paths, workspace config, keys, and remotes
nero config path                 Print the user config file path
nero find <query>                Search notes
nero backlinks <note>            Find notes linking to a note
nero today                       Create/locate today's daily note and list due notes
nero render <note>               Render a note to HTML
nero reindex                     Refresh the search index
nero watch                       Watch Markdown files and refresh the index
nero doctor                      Check workspace health
```

Run `nero --help` for the full command set, including Git and backup commands.

`nero delete <note>` (also `nero rm <note>`) permanently removes the selected Markdown file from the current workspace. `nero list` assigns integer IDs, kept in `.nero/note-ids.sqlite`; IDs stay stable across reindex and aren't reused after `nero delete`. IDs are local to a workspace, so use `-w <workspace>` first when working in another workspace. A `#` prefix is an explicit ID reference (quote it in shells, e.g. `nero open '#3'`); a bare number also resolves as an ID if no note path with that name exists.

`nero config show` summarizes the active application/config paths, registered workspaces, backup identity location and public recipient, workspace storage profiles, Git remotes, and rclone configuration file/remotes. It never prints the private backup identity or reads provider credentials into its output; cloud provider credentials remain managed by rclone.

## Markdown reader and browser preview

Use the reader for ordinary Markdown files even when they are not Nero notes and do not live in a workspace:

```bash
nero view README.md                  # interactive terminal reader (press B for browser preview)
nero README.md                       # shorthand for the same reader
nero view ~/Documents/research.md    # external file
cat report.md | nero view -          # read Markdown from stdin
nero view --browser README.md        # styled HTML preview in the default browser
nero browse "Project Plan"           # browser preview of a note in the selected workspace
```

While the terminal reader is open, press `B` to open that same document in the browser without exiting the reader. The browser preview is written to a private, randomly named directory under the system temporary directory and opened with the operating system's default browser. Old previews are cleaned up after seven days. Relative image paths and links use the source Markdown file's directory. Frontmatter is optional and, when present, appears in a collapsible metadata panel. Code highlighting and math typesetting use Highlight.js and MathJax from their CDNs, so those enhancements require an internet connection; the Markdown content itself is rendered locally by Nero's Comrak renderer.

## TUI

```bash
nero tui
```

The TUI is keyboard-first:

- `j` / `k` — move through notes
- `e` — edit the selected note in `$EDITOR`, then return to Nero
- `B` — open the selected note in the default browser as a read-only HTML preview
- `/` — search
- `:` — command palette (`:edit`, `:browser`, and `:view-browser` are available)
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

When rebuilding the search index, remove only `.nero/index.sqlite`; Markdown files are the source of truth.

## Git, backups, and remote storage

Use Git for version history:

```bash
nero git init
nero git snapshot "Notes checkpoint"
nero git remote add origin git@github.com:you/notes.git
nero git push
```

Create a portable workspace backup and verify it before relying on it:

```bash
nero backup create backup.zip
nero backup verify backup.zip
nero backup restore backup.zip ~/recovered-notes
```

Encrypted workspace backups use Nero's age identity:

```bash
nero key generate
nero backup create --encrypt backup.age
nero backup recovery-test backup.age
```

For a portable backup of the **entire Nero home** (configuration, keys, workspaces, assets, Git history, and workspace storage profiles), use the passphrase-encrypted home format:

```bash
nero backup home create
nero backup home verify "$HOME/.nero/backups/nero-home-backup-1234567890.zip.age"  # replace with the path Nero printed
nero backup home restore /path/to/nero-home-backup.zip.age
```

Home restore targets `~/.nero` by default and requires a new or empty destination. The backup excludes old archives and rebuildable SQLite indexes, and rebases paths inside the restored app home. See [Backups](BACKUP.md), [Recovery](RECOVERY.md), [Security](SECURITY.md), and [Storage](STORAGE.md).
