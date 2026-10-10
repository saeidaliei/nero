# Backup and recovery

Nero has two backup scopes. Use a **workspace backup** for one workspace, or a **home backup** to move/restore the entire Nero profile on a new machine.

## Workspace backup

A workspace backup creates a portable ZIP with `manifest.json`, relative paths, file sizes, and SHA-256 hashes. `.nero/` (the disposable index/settings directory) and `.git/` (version history) are excluded.

```bash
nero backup create
ARCHIVE="$HOME/.nero/backups/nero-backup-1234567890.zip"  # replace with the path Nero printed
nero backup verify "$ARCHIVE"
nero backup restore "$ARCHIVE" ~/recovered-notes
nero backup recovery-test "$ARCHIVE"
```

The encrypted workspace form uses the age identity stored at `$NERO_HOME/keys/identity.txt` (default `~/.nero/keys/identity.txt`):

```bash
nero key generate
nero backup create --encrypt
nero backup verify backup.age
nero backup recovery-test backup.age
nero backup restore backup.age ~/recovered-notes
```

Keep the private identity safe. A workspace `.age` snapshot cannot be decrypted without it.

## Full-home backup (recommended for moving to a new machine)

Create a passphrase-encrypted `.zip.age` containing all files under `$NERO_HOME` (default `~/.nero`) except the prior `backups/` directory and disposable SQLite index/WAL files. This includes `config.json`, backup keys, workspace Markdown, assets, `.git/` history, and workspace-local storage profiles.

```bash
nero backup home create
ARCHIVE="$HOME/.nero/backups/nero-home-backup-1234567890.zip.age"  # replace with the path Nero printed
nero backup home verify "$ARCHIVE"
nero backup home recovery-test "$ARCHIVE"
```

Nero asks for a new passphrase twice when creating a backup and asks for it when verifying/restoring. The encrypted home archive uses a passphrase-based age recipient; it does **not** depend on the age identity stored inside the archive. That avoids the circular problem of encrypting the key with itself. Choose a strong passphrase and store it separately—the archive cannot be recovered without it.

The output path is printed after creation. Copy that `.zip.age` file to an external drive or remote storage. Do not upload a plain copy of `~/.nero`; it contains notes and the private backup identity.

### Restore on a new machine

Install Nero, copy the home archive onto the new machine, then run:

```bash
nero backup home restore /path/to/nero-home-backup.zip.age
nero workspace list
nero workspace use personal
nero today
```

The default restore destination is `~/.nero`. It must not already contain files; if it does, restore to a separate empty directory and use that path as `NERO_HOME` on subsequent runs. You can specify a destination explicitly:

```bash
nero backup home restore /path/to/nero-home-backup.zip.age ~/.local/share/nero
```

When restoring elsewhere, Nero rebases workspace paths stored under the original application home. Workspaces explicitly registered **outside** `$NERO_HOME` are not bundled by the home backup, so Nero refuses to create a supposedly complete archive in that state. Back those workspaces up individually with `nero backup create` or move them under the app home first. Nero also refuses a full-home backup if `NERO_CONFIG_HOME` or `NERO_CONFIG_DIR` changes the standard `config.json`/`keys/` locations, because that would make the restored profile incomplete or leave key permissions ambiguous. Unset those advanced overrides before making a portable full-home archive.

`backups/` is excluded to prevent recursively embedding old archives in every new full-home backup. SQLite indexes are excluded because Nero rebuilds them; workspace-local settings such as `storage.json` are retained. Git history is included in full-home backups.

### How it works

```text
$NERO_HOME
    │
    ├── config + keys + workspaces + Git history
    │
    ▼
ZIP snapshot + SHA-256 manifest
    │
    ▼
age passphrase encryption
    │
    ▼
nero-home-backup-1234567890.zip.age
```

The archive records its source home so paths under that root can be rewritten when restoring on another user account or machine. Restore is limited to a new/empty directory, rejects symlinks in source home, checks archive paths against traversal, verifies file hashes, and restores restrictive permissions to the private key on Unix.

## Remote workspace backups with rclone

Configure a remote once, then push encrypted backups of a selected workspace:

```bash
rclone config
nero key generate
nero -w personal storage add mega mega:nero-backups/personal --encrypt
nero -w personal backup push mega
nero -w personal backup list mega
```

This is a **workspace** snapshot, not the full-home format. For a whole-machine restore, upload the `.zip.age` from `nero backup home create` using `rclone copyto` or another file transfer tool.

## Git versioning

Git is for history/diffs and optional collaboration; backups are for disaster recovery. A private GitHub repository is not client-side encrypted. See the [root README quick start](../README.md#quick-start-private-github-repository) for the setup steps.

## Safety rules

- Verify backup artifacts before relying on them.
- Test recovery periodically with `recovery-test`.
- Keep home-backup passphrases separate from the archives.
- Keep workspace-backup identity files separate from `.age` snapshots.
- Never place a backup output inside a workspace; home backups skip the `backups/` folder to avoid recursive inclusion.
- Restore into a new/empty destination; do not overwrite a live workspace or Nero home in place.
