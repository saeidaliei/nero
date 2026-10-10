# Nero remote storage

Nero treats cloud storage as a transport layer, not as part of the note format. The live workspace remains ordinary Markdown files. Remote storage receives backup artifacts created by Nero.

## MEGA quick start

1. Install [rclone](https://rclone.org/install/).
2. Run `rclone config`, create a remote named `mega`, choose the Mega backend, and complete authentication. Test it with `rclone lsd mega:`. See the [rclone Mega backend guide](https://rclone.org/mega/).
3. For an encrypted workspace snapshot, create the Nero age key once and add the remote to that workspace:

   ```bash
   nero key generate
   nero -w personal storage add mega mega:nero-backups/personal --encrypt
   nero -w personal backup push mega
   nero -w personal backup list mega
   ```

This stores only the target and encryption setting in the workspace profile; rclone owns provider credentials. The encrypted workspace backup needs the private Nero identity to restore.

For a portable **full-home** backup that includes the identity, run `nero backup home create` and upload the resulting passphrase-encrypted `.zip.age` file using `rclone copyto`. The home archive uses a separate passphrase, so it can recover the identity as well as the workspaces. Use the exact filename printed by Nero:

```bash
ARCHIVE="$HOME/.nero/backups/nero-home-backup-1234567890.zip.age"  # replace with the printed path
rclone copyto "$ARCHIVE" "mega:nero-home-backups/$(basename "$ARCHIVE")"
```

On a new machine, download that object and restore it:

```bash
rclone copyto "mega:nero-home-backups/nero-home-backup-1234567890.zip.age" "$HOME/nero-home-backup.zip.age"
nero backup home verify "$HOME/nero-home-backup.zip.age"
nero backup home restore "$HOME/nero-home-backup.zip.age"
```

Replace the example filename with the actual archive name. See [Backups](BACKUP.md).

## Requirements

Install `rclone` and configure at least one remote with:

```bash
rclone config
```

Nero uses the rclone configuration already available to the user. Nero does not copy provider credentials into the workspace.

## Configure a storage profile

```bash
nero storage add mega mega:nero-backups --encrypt
nero storage list
nero storage test mega
```

The profile contains only the rclone target and whether Nero should force encrypted `.age` backups. It does not contain passwords, OAuth tokens, or other provider secrets.

## Upload a backup

```bash
nero backup push mega
```

For an encrypted profile, Nero creates a temporary age-encrypted backup, uploads it, and removes the temporary local artifact. For a plain profile, the temporary artifact is an ordinary ZIP. You can also force encryption for a plain profile:

```bash
nero backup push mega --encrypt
```

Nero uploads one file with rclone `copyto --checksum --immutable`; it never uses `rclone sync` for backup upload, so an interrupted or misconfigured remote cannot cause Nero to delete unrelated remote data or silently replace an existing snapshot.

## List remote backups

```bash
nero backup list mega
```

## Restore

Restore into a new or empty directory:

```bash
nero backup pull mega nero-backup-1234567890.age ~/recovered-notes
```

Encrypted backups require the Nero age identity:

```bash
nero backup pull mega nero-backup-1234567890.age ~/recovered-notes --identity /path/to/identity.txt
```

## Providers

The provider is selected entirely in `rclone config`. Current rclone documentation lists Mega, Amazon S3, WebDAV, Google Drive, Proton Drive, and many other storage systems as supported backends.

## Design rule

Remote storage is **backup**, not bidirectional synchronization. Nero will not silently merge or delete note files based on remote state. A future synchronization feature, if we ever need one, will be a separate explicit operation.
