# Git versioning with a private GitHub repository

Git gives a workspace history, diffs, and a way to recover earlier revisions. It is separate from Nero's backup commands: a private GitHub repository controls access, but it is **not client-side encrypted**. Use age-encrypted backups for disaster recovery too.

## Create a private repository

1. Open [GitHub's new repository page](https://github.com/new).
2. Choose a name such as `nero-personal` and set visibility to **Private**.
3. Leave the repository empty—don't initialize it with a README, `.gitignore`, or license because the local workspace already exists.
4. Ensure SSH access is configured and works with `ssh -T git@github.com`.

## Connect a Nero workspace

The following assumes your workspace is named `personal` and uses the default `~/.nero/workspaces/personal` path:

```bash
nero -w personal git init
nero -w personal git snapshot "Initial notes"
nero -w personal git remote add origin git@github.com:YOUR_USERNAME/nero-personal.git
git -C "$HOME/.nero/workspaces/personal" push -u origin HEAD
```

The first push uses ordinary Git directly to set upstream tracking for the branch you're currently on. If you configured a custom `NERO_HOME` or workspace path, replace the Git path with the one shown by `nero -w personal workspace show` (or `nero workspace show` after selecting that workspace).

For later changes, create a snapshot and push:

```bash
nero -w personal git snapshot "Update research notes"
nero -w personal git push origin main
```

Replace `main` with your current branch if it has a different name. Check the branch with `nero -w personal git status`.

## Clone on another machine

```bash
git clone git@github.com:YOUR_USERNAME/nero-personal.git "$HOME/notes"
nero workspace add personal "$HOME/notes"
nero workspace use personal
nero reindex
```

Git history is included in a full-home backup when the repository lives under `$NERO_HOME`. A full-home backup is usually simpler when you want to move the entire Nero profile, including the backup identity and workspace registry. See [Backup and recovery](BACKUP.md).
