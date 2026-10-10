# Workspaces

Nero keeps its user-level state in one application home and stores named workspaces underneath it by default. On Linux and macOS the default application home is `~/.nero`; on Windows it is `%USERPROFILE%\.nero`.

```text
~/.nero/
├── config.json       # default workspace and named workspace registry
├── keys/             # age backup identity and public recipient
├── backups/          # default local backup destination
└── workspaces/
    ├── personal/
    │   ├── .nero/    # disposable index and workspace-local settings
    │   ├── daily/
    │   └── ...md
    ├── research/
    │   ├── .nero/
    │   └── ...md
    └── writing/
        ├── .nero/
        └── ...md
```

Each workspace is an independent directory containing ordinary Markdown files. Its `.nero/` directory contains the rebuildable SQLite index and workspace-local settings such as remote-storage profiles. The app home itself is not a workspace.

## Start here

```bash
nero home
nero workspace create personal
nero workspace create research
nero workspace list
nero workspace use personal
```

The first created workspace becomes the default if one has not already been set. You can run Nero from any directory once a default workspace is selected:

```bash
cd ~/Downloads
nero new "An idea"
nero find "Fourier"
nero today
nero tui
nero gui
```

Use `-w` or `--workspace` for a one-command override:

```bash
nero -w research list
nero --workspace personal tui
```

## Application home versus workspace container

`nero home` prints the paths for the application home, config file, keys, workspace container, and default backup destination. `nero home path` prints only the app-home path.

The workspace container defaults to `$NERO_HOME/workspaces`. To change only where new named workspace directories are created:

```bash
nero workspace home
nero workspace home set ~/Documents/nero-workspaces
nero workspace create personal
nero workspace home reset
```

Resetting the workspace container does not move or delete existing workspaces. Registered workspaces keep their existing paths until you move them yourself and update the registration.

## Choose a different application home

Set `NERO_HOME` to an **absolute path** before running Nero. This relocates the config, workspace container, key directory, and default backup destination together.

Fish (persistent for future terminals):

```fish
set -Ux NERO_HOME "$HOME/.local/share/nero"
nero home
```

Bash/Zsh (put the export in your shell startup file to persist it):

```bash
export NERO_HOME="$HOME/.local/share/nero"
nero home
```

To return to the default in Fish, remove the universal variable with `set -eU NERO_HOME`. In Bash/Zsh, remove the export from your shell startup file and open a new shell.

A different `NERO_HOME` is a different Nero profile. Nero will not move your existing workspace folders or registry automatically. Register an existing workspace in the new profile with `nero workspace add <name> <path>`, then select it with `nero workspace use <name>`. This avoids accidentally moving notes or changing paths behind your back.

## Move an existing workspace

Nero does not automatically move workspace directories. To move one into the default workspace container, close Nero first, then move the complete workspace—including its `.nero/` metadata and `.git/` directory if present—and register the new path:

```bash
mkdir -p ~/.nero/workspaces
mv ~/notes ~/.nero/workspaces/personal
nero workspace add personal ~/.nero/workspaces/personal
nero workspace use personal
```

Check `nero list`, `nero find`, and `nero doctor` before deleting any separate copy. If the name already exists in the registry, use `nero workspace remove <name>` first, then re-register it at the new path.

## Workspace selection precedence

Nero chooses a workspace in this order:

1. Explicit `--workspace PATH_OR_NAME` or `-w PATH_OR_NAME`.
2. `NERO_WORKSPACE`, useful for scripts and direct GUI startup.
3. Discovery from the current directory and its ancestors.
4. The saved default workspace, used when running elsewhere.

## Configuration

The default config is `~/.nero/config.json` (or `%USERPROFILE%\.nero\config.json` on Windows). It stores workspace names and paths, not note contents or cloud credentials. `NERO_HOME` selects the whole application home. `NERO_CONFIG_HOME` changes only the config location, and `NERO_CONFIG_DIR` changes only the backup-key directory; those two are advanced/testing overrides.

A new Nero profile starts clean. It does not import config/key files from outside the active app home or automatically move existing workspace folders. Register any workspace you want Nero to use with `nero workspace add <name> <path>`.
