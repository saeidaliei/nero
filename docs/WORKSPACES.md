# Workspace configuration

Nero does not require you to `cd` into your notes directory before every command. Set a default workspace once, or register several named workspaces.

## One workspace

Create a workspace and set it as your default:

```bash
nero init ~/notes
nero workspace set ~/notes
```

After that, commands such as these work from any current directory:

```bash
nero list
nero find "something"
nero edit "An idea"
nero tui
```


`nero workspace set` accepts an initialized workspace (one containing `.nero/`). The path is saved as a canonical absolute path in Nero's per-user config, not in the notes folder.

## Multiple workspaces

Register named workspaces, then select a default:

```bash
nero init ~/work/research-notes
nero workspace add research ~/work/research-notes
nero workspace add personal ~/notes
nero workspace use research
nero workspace list
```

Use `nero workspace use personal` to switch the saved default. `nero workspace show` prints the workspace selected for the current command, and `nero workspace remove research` removes a registered name without deleting any files. `nero workspace clear` clears the saved default; it does not remove registered entries or delete notes.

## One-command override

`-w` and `--workspace` accept either a registered name or a filesystem path:

```bash
nero -w personal list
nero --workspace ~/notes tui
nero find "equation" --workspace research
```

The override applies only to that command; it does not change your saved default.

## Selection precedence

Nero chooses a workspace in this order:

1. Explicit `--workspace PATH_OR_NAME` or `-w PATH_OR_NAME`.
2. `NERO_WORKSPACE`, useful for scripts and launching the GUI.
3. Discovery from the current directory, walking up its parents until a `.nero/` workspace marker is found.
4. The saved default from `nero workspace set` or `nero workspace use`, used when the current directory is not already inside a workspace.

This order lets scripts override the interactive default without silently changing it.

## Configuration location

Nero stores the preference file outside every workspace:

- Linux/BSD: `$XDG_CONFIG_HOME/nero/config.json`, or `~/.config/nero/config.json` when `XDG_CONFIG_HOME` is unset.
- macOS: `~/Library/Application Support/Nero/config.json`.
- Windows: `%APPDATA%\Nero\config.json`.
- `NERO_CONFIG_HOME` can override the application config directory on any platform.

The config stores paths and workspace names, not note contents, cloud credentials, or encryption private keys. Copying the Markdown workspace to another machine does not copy this local preference; set it again there.
