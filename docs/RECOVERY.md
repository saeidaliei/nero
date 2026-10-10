# Nero recovery guide

Nero has two backup scopes. A workspace backup protects one workspace; a full-home backup protects the Nero profile so it can be restored on a new machine.

## 1. Back up the complete Nero profile

```bash
nero backup home create
ARCHIVE="$HOME/.nero/backups/nero-home-backup-1234567890.zip.age"  # replace with the path Nero printed
nero backup home verify "$ARCHIVE"
nero backup home recovery-test "$ARCHIVE"
```

Creation asks for a passphrase twice. Keep it separately from the archive: Nero cannot recover it. The resulting `.zip.age` includes the configuration, workspace registry, notes, assets, workspace-local storage profiles, Git history, and age identity. Previous backups and disposable SQLite indexes are excluded. The printed path is the authoritative archive location.

Copy the archive to an external drive or remote storage. For MEGA with rclone, configure a remote named `mega`, then upload the file printed by Nero:

```bash
ARCHIVE="$HOME/.nero/backups/nero-home-backup-1234567890.zip.age"
rclone copyto "$ARCHIVE" "mega:nero-home-backups/$(basename "$ARCHIVE")"
```

Replace the sample filename with the exact one Nero printed. This transfers one file; it does not mirror or delete other remote data.

## 2. Restore on a new machine

Install Nero and copy/download the `.zip.age` archive to the new machine. If it lives in MEGA, install rclone and configure the `mega` remote with `rclone config` first; rclone credentials are kept outside Nero and are not included in the archive. Then run:

```bash
nero backup home restore /path/to/nero-home-backup.zip.age
nero workspace list
nero workspace use personal
nero today
```

Enter the same passphrase used when creating the archive. By default, Nero restores into `~/.nero`, which must be new or empty. To restore elsewhere, supply a destination and set `NERO_HOME` to that directory for later commands:

```bash
nero backup home restore /path/to/nero-home-backup.zip.age "$HOME/.local/share/nero"
set -Ux NERO_HOME "$HOME/.local/share/nero"  # Fish
```

The backup records its original home path and rewrites registered workspace paths that lived beneath that home. Nero refuses to create the full-home archive if a registered workspace is outside `$NERO_HOME`; move it under the home first or back it up separately with `nero backup create`.

## 3. Back up one workspace

A workspace backup creates a portable ZIP with `manifest.json`, relative paths, file sizes, and SHA-256 hashes. The workspace `.nero/` index/settings and `.git/` history are excluded.

```bash
nero backup create
ARCHIVE="$HOME/.nero/backups/nero-backup-1234567890.zip"  # replace with the path Nero printed
nero backup verify "$ARCHIVE"
nero backup recovery-test "$ARCHIVE"
nero backup restore "$ARCHIVE" ~/recovered-notes
```

For an age-encrypted workspace backup, create/use Nero's identity and keep a separate safe copy of it:

```bash
nero key generate
nero backup create --encrypt
nero backup verify backup.age
nero backup recovery-test backup.age
nero backup restore backup.age ~/recovered-notes
```

Unlike full-home archives, workspace `.age` archives rely on the external Nero identity. They do not contain the identity themselves.

## 4. Git recovery

A workspace with Git versioning can also be cloned independently:

```bash
git clone git@github.com:YOUR_USERNAME/nero-personal.git "$HOME/notes"
nero workspace add personal "$HOME/notes"
ero workspace use personal
nero reindex
```

Git is useful for history and recovering individual changes. A private GitHub repository is access-controlled, but it is not client-side encrypted. See [Git versioning](GIT.md) for the private GitHub setup guide.

## Safety notes

- Test a full-home backup with `nero backup home recovery-test` before trusting it.
- Keep full-home backup passphrases separate from the archive.
- Keep workspace-backup identity files separate from workspace `.age` snapshots.
- Never upload a plain copy of `$NERO_HOME`; it contains notes and the private identity.
- Restores only target a new or empty destination and verify manifest hashes before completion.
