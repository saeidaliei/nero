# Application home and configuration

Nero uses `~/.nero` by default on Linux/macOS and `%USERPROFILE%\.nero` on Windows. This is one place for configuration, named workspaces, backup keys, and local backup artifacts.

```text
~/.nero/
├── config.json
├── keys/
│   ├── identity.txt
│   └── recipient.txt
├── backups/
└── workspaces/
    ├── personal/
    │   ├── .nero/
    │   │   ├── index.sqlite
    │   │   └── storage.json
    │   ├── daily/
    │   └── notes.md
    └── research/
        └── ...md
```

- `config.json` stores workspace names/paths and the selected default.
- `keys/` contains the age workspace-backup identity and public recipient. The identity is private.
- `backups/` stores local backup artifacts. Full-home backups exclude this directory so archives do not nest recursively.
- `workspaces/` contains named workspace folders by default.
- A workspace's `.nero/` contains its rebuildable SQLite index and settings such as remote-storage profiles.

## Configuration

Nero creates `config.json` when preferences are first saved. Prefer `nero workspace use`, `nero workspace add`, and `nero workspace home set` over editing the file directly. `nero home` prints active paths; `nero home path` prints the application-home path.

A simplified config looks like:

```json
{
  "format": "nero-config",
  "version": 1,
  "workspace_home_path": null,
  "default_workspace_path": null,
  "default_workspace_name": "personal",
  "workspaces": {
    "personal": "/home/you/.nero/workspaces/personal"
  }
}
```

## Change the complete application home

Set `NERO_HOME` to an absolute path before running Nero. This selects a separate, self-contained profile; Nero will not import config or keys from another location.

Fish:

```fish
set -Ux NERO_HOME "$HOME/.local/share/nero"
nero home
```

Bash/Zsh:

```bash
export NERO_HOME="$HOME/.local/share/nero"
nero home
```

To reset the Fish universal variable, run `set -eU NERO_HOME`. For Bash/Zsh, remove the export from your shell startup file and open a new shell.

## Advanced overrides

`NERO_CONFIG_HOME` changes only where `config.json` is stored. `NERO_CONFIG_DIR` changes only where the age identity is stored. These are mainly for tests and advanced setups; prefer `NERO_HOME` for a complete relocation.

## Backups

Use `nero backup create` for the selected workspace. Use `nero backup home create` for a passphrase-encrypted portable snapshot of the whole Nero home, including its identity and workspaces. The full-home backup omits previous backup archives and rebuildable SQLite indexes. See [Backup and recovery](BACKUP.md).

A new profile starts clean and uses only its active `NERO_HOME`. Full-home backups include files inside that home; they refuse to run if a registered workspace is outside it or if advanced `NERO_CONFIG_HOME`/`NERO_CONFIG_DIR` overrides relocate the config or keys, because the archive would otherwise be incomplete.
