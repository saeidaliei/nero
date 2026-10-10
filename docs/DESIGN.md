# Nero design notes

## Core rule

**The filesystem is the product.** SQLite, caches, indexes, and UI state are secondary.

## Workspace layout

```text
workspace/
├── projects/           # optional user organization
├── daily/
├── assets/
├── *.md
└── .nero/
    └── index.sqlite
```

Nero recursively discovers `.md` files beneath the workspace root. Directories are ordinary filesystem organization.

## Note identity

A note is identified by its relative path. A title is presentation metadata, not the canonical identifier. This keeps links portable and keeps Git diffs understandable.

Friendly aliases can be layered on later, but they must not destroy path-based portability.

## Internal links

Syntax:

```text
[[Relative Name]]
[[relative/path.md]]
[[Target|Displayed label]]
[[Note#Heading]]
```

The target is resolved in this order:

1. relative to the source note's directory
2. workspace-relative path
3. exact note title
4. filename stem

The optional `#Heading` fragment does not change note identity; heading-aware navigation will be layered on later. `[[target|label]]` keeps the target and display label separate in the core document model.

## Search

```text
Markdown files
      │
      ▼
   parser
      │
      ├───────────────► links table
      │
      ▼
 SQLite FTS5
```

The search index stores only derived information. It can be deleted and rebuilt at any time.

Index refresh is incremental using file modification time, size, and title metadata.

## Markdown and math

Nero stays close to CommonMark/GFM. The intentional extensions are:

- wiki links (`[[...]]`)
- task list rendering
- `$...$` and `$$...$$` math
- `\(...\)` and `\[...\]` math
- simple YAML-like frontmatter extraction

The current HTML renderer is Comrak. The future GUI will pass math nodes through KaTeX for polished typesetting. The TUI will eventually use terminal-friendly math rendering.

## File watching

External editors are first-class citizens. A watcher observes the workspace recursively, ignores `.nero`, and reports Markdown changes. The CLI can use it to keep the index fresh; the TUI will later consume the same mechanism for live reload.

Filesystem watchers are advisory. Nero should always be able to recover from missed events by doing a complete index refresh.

## Frontends

### CLI

The CLI is scriptable and should never require a UI.

### TUI

The TUI is a keyboard-first reader/browser. Press `e` or run `:edit` to hand the selected Markdown file to `$EDITOR`; Nero temporarily restores the normal terminal, then re-enters the TUI and refreshes the index when the editor exits. Editing remains delegated to the user’s editor rather than embedding another editing engine.

### GUI

The GUI is a Tauri desktop shell with a deliberately small HTML/CSS/TypeScript surface. It consumes `nero-core` over typed Tauri commands and receives filesystem changes over Tauri events. It does not become the owner of documents or indexing.

## Things deliberately excluded

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

Every future feature has to justify its complexity against the core idea: **small, local, elegant, useful.**


## GUI architecture

The desktop frontend is intentionally thin:

```text
Tauri / WebView
      │ IPC commands/events
      ▼
  nero-core
      │
      ├── Markdown files
      ├── SQLite FTS5
      └── filesystem watcher
```

The frontend receives derived document data from Rust and never becomes the owner of note storage. Comrak emits math nodes using `data-math-style="inline|display"`; the GUI passes those nodes to KaTeX instead of trying to parse LaTeX itself.

Tauri's command system is the IPC boundary. Live Markdown changes are forwarded as a `workspace-changed` event after the core refreshes its index.

## Editing

The GUI edits Markdown source directly. Preview is derived from the same source; no proprietary block model is introduced.

## 0.7 editor design

The embedded GUI editor remains a normal Markdown text surface rather than a block editor. A styled syntax layer sits behind the real textarea; the textarea remains responsible for selection, input, undo, clipboard, and text editing. This keeps the editor visually richer without inventing a second document representation.

Split mode derives preview from the current draft and synchronizes preview scroll proportionally with the source editor. Saved rendering still comes from the Rust/Comrak document engine, so live preview is an interaction aid rather than the semantic source of truth.

Images are imported through a native file picker into `assets/`. Markdown stores ordinary relative image references. The GUI asks the core to resolve local images and receive data URLs for display, avoiding a dependency on webview filesystem URL permissions while keeping the files portable outside Nero.
## Backup and storage

The live workspace remains plaintext and portable. Backups are derived snapshots. Nero's first backup format is a ZIP archive containing a manifest and SHA-256 hashes, excluding `.nero/` and `.git/`.

Storage and encryption are intentionally separate layers. The current path is:

```text
workspace
   │
   ▼
snapshot
   │
   ▼
optional age encryption
   │
   ├── Git remote
   └── rclone remote
```

Git is for version history; object/cloud storage is for disaster recovery. Neither layer changes the source-of-truth Markdown model.


## 0.9–1.0 resilience layers

Nero now distinguishes three concerns:

```text
Markdown workspace
      │
      ├── Git ───────────► history / remotes
      │
      └── backup snapshot
                │
                ▼
           optional age
                │
                ▼
           rclone transport
```

The live workspace remains ordinary files. The default age X25519 identity is stored outside the workspace so an encrypted backup can remain safe even when the storage provider can read the backup object.

The Git integration delegates to the real `git` executable rather than linking a Git implementation into the core. This keeps Git configuration, credential helpers, SSH keys, signing, hosting providers, and other Git tooling authoritative.

Remote object storage is intentionally not coupled to the age implementation. The rclone adapter uploads the already-encrypted `.age` artifact when a storage profile requires encryption; provider credentials stay in rclone's own configuration.

## Remote storage adapter

Nero delegates transport to the external `rclone` executable instead of embedding provider SDKs. A workspace stores only named target paths and an encrypted-backup policy in `.nero/storage.json`; it never stores provider passwords, OAuth tokens, or API keys there.

Uploads use `rclone copyto` for a single backup object. Nero intentionally does not use `rclone sync` for backups because backup transport must not delete unrelated objects on the destination.


## User-level workspace configuration

The CLI stores default/named workspace paths in `~/.nero/config.json` by default (or `$NERO_HOME/config.json` when overridden), outside each workspace directory. Explicit `--workspace`/`-w` and `NERO_WORKSPACE` overrides take precedence; otherwise local workspace discovery comes before the saved default, which acts as a fallback in arbitrary directories. The config contains paths and names only; it is not a second source of note content and does not travel with a workspace backup.


## Application home and workspaces

The default application home is `~/.nero` (override with the absolute `NERO_HOME` environment variable). It contains `config.json`, `keys/`, `backups/`, and the default `workspaces/` parent. Each workspace remains a normal directory with Markdown as its source of truth and a workspace-local `.nero/` for the disposable index/settings. Keeping the app home separate from workspace metadata avoids mistaking the application root for a note workspace.
