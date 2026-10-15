# Arch Linux package

The release workflow produces `nero-bin-<version>-1-x86_64.pkg.tar.zst` using
`makepkg` inside an Arch Linux container. It contains the **CLI and TUI** binary
and installs it as `/usr/bin/nero`; it does not contain the Tauri desktop GUI.

Install a downloaded package with:

```bash
sudo pacman -U ./nero-bin-<version>-1-x86_64.pkg.tar.zst
```

Optional integrations remain optional: install `git` for Git versioning and
`rclone` for remote storage. For the desktop GUI, use the AppImage when present
or a Linux GUI package (`.deb`/`.rpm`) where appropriate.
