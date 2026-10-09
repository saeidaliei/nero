#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
ALT_WORKSPACE="${TMP}-research"
USER_CONFIG="${TMP}-user-config"
trap 'rm -rf "$TMP" "$ALT_WORKSPACE" "$USER_CONFIG"' EXIT

# This script is a shell-level smoke test for the expected CLI UX.
# It requires a compiled `nero` binary at $NERO_BIN and a working `git` executable.
NERO_BIN="${NERO_BIN:-$ROOT/target/debug/nero}"

if [[ ! -x "$NERO_BIN" ]]; then
  echo "nero binary not found: $NERO_BIN" >&2
  exit 2
fi

"$NERO_BIN" init "$TMP"
cd "$TMP"
"$NERO_BIN" new "Fourier Transform"
mkdir -p math/notes
printf '# Fourier Transform\n\nSee [[math/Signal Processing|the signal note]].\n' > math/notes/fourier-transform.md
printf '---\ntitle: Signal Processing\n---\n' > math/Signal.md
"$NERO_BIN" find fourier-transform
"$NERO_BIN" backlinks "Signal Processing"
"$NERO_BIN" today
"$NERO_BIN" render "math/notes/fourier-transform.md"
"$NERO_BIN" reindex
"$NERO_BIN" doctor

# Global workspace preferences make ordinary commands work without changing directory.
export NERO_CONFIG_HOME="$USER_CONFIG"
"$NERO_BIN" workspace set "$TMP"
cd /tmp
"$NERO_BIN" new "Created from outside workspace"
"$NERO_BIN" list | grep -F "Created from outside workspace"
"$NERO_BIN" init "$ALT_WORKSPACE"
"$NERO_BIN" workspace add research "$ALT_WORKSPACE"
"$NERO_BIN" --workspace research new "Research-only note"
test -f "$ALT_WORKSPACE/research-only-note.md"
"$NERO_BIN" workspace use research
"$NERO_BIN" new "Default alias note"
test -f "$ALT_WORKSPACE/default-alias-note.md"
"$NERO_BIN" workspace set "$TMP"
cd "$TMP"
BACKUP="$TMP/../nero-smoke-backup.zip"
"$NERO_BIN" backup create "$BACKUP"
"$NERO_BIN" backup verify "$BACKUP"
RESTORE="$TMP-restore"
"$NERO_BIN" backup restore "$BACKUP" "$RESTORE"
test -f "$RESTORE/math/Signal.md"

# Encryption + recovery. The identity lives outside the workspace.
export NERO_CONFIG_DIR="$TMP/config"
"$NERO_BIN" key generate
ENCRYPTED="$TMP/../nero-smoke-backup.age"
"$NERO_BIN" backup create --encrypt "$ENCRYPTED"
cd /tmp
"$NERO_BIN" backup verify "$ENCRYPTED"
"$NERO_BIN" backup recovery-test "$ENCRYPTED"
ENCRYPTED_RESTORE="$TMP-encrypted-restore"
"$NERO_BIN" backup restore "$ENCRYPTED" "$ENCRYPTED_RESTORE"
test -f "$ENCRYPTED_RESTORE/math/Signal.md"
cd "$TMP"

# Git versioning. Keep the disposable Nero index out of the repository.
"$NERO_BIN" git init
git config user.name "Nero Smoke Test"
git config user.email "nero-smoke@example.invalid"
"$NERO_BIN" git snapshot "Smoke test checkpoint"
"$NERO_BIN" git status
test -d "$TMP/.git"

# Remote storage smoke test using a tiny fake rclone binary. This validates Nero's
# provider-neutral CLI contract without requiring real cloud credentials.
FAKE_BIN="$TMP/fake-bin"
FAKE_REMOTE="$TMP/fake-remote"
mkdir -p "$FAKE_BIN" "$FAKE_REMOTE"
cat > "$FAKE_BIN/rclone" <<'FAKE_RCLONE'
#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  version) echo 'rclone v-test'; exit 0 ;;
  lsf)
    target="$2"
    target="${target#fake:}"
    if [[ -d "$target" ]]; then
      find "$target" -maxdepth 1 -type f 2>/dev/null | while IFS= read -r file; do basename "$file"; done
    fi
    ;;
  copyto)
    src="$2"
    dst="$3"
    dst="${dst#fake:}"
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
    ;;
  *) echo "fake rclone does not implement: $*" >&2; exit 1 ;;
esac
FAKE_RCLONE
chmod +x "$FAKE_BIN/rclone"
PATH="$FAKE_BIN:$PATH" "$NERO_BIN" storage add smoke "fake:$FAKE_REMOTE" --encrypt
PATH="$FAKE_BIN:$PATH" "$NERO_BIN" storage test smoke
PATH="$FAKE_BIN:$PATH" "$NERO_BIN" backup push smoke
REMOTE_FILE="$(find "$FAKE_REMOTE" -maxdepth 1 -type f -name '*.age' | head -1)"
REMOTE_NAME="$(basename "$REMOTE_FILE")"
test -n "$REMOTE_NAME"
PATH="$FAKE_BIN:$PATH" "$NERO_BIN" backup list smoke | grep -F "$REMOTE_NAME"
REMOTE_RESTORE="$TMP-remote-restore"
PATH="$FAKE_BIN:$PATH" "$NERO_BIN" backup pull smoke "$REMOTE_NAME" "$REMOTE_RESTORE" --identity "$NERO_CONFIG_DIR/backup-identity.txt"
test -f "$REMOTE_RESTORE/math/Signal.md"
