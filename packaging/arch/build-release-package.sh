#!/usr/bin/env bash
# Runs inside archlinux:base-devel in the GitHub release workflow.
set -euo pipefail

work_root="${WORK_ROOT:-/work}"
release_dir="$work_root/release-assets"
tag="${RELEASE_TAG:?RELEASE_TAG must be set, for example v1.2.3}"
version="${tag#v}"
cli_archive="$release_dir/nero-cli-${tag}-linux-x86_64.tar.gz"

if [[ ! -f "$cli_archive" ]]; then
  echo "Arch packaging input is missing: $cli_archive" >&2
  exit 1
fi

# Use the Arch toolchain/package database so this is a real pacman package.
pacman -Syu --noconfirm --needed base-devel

if ! id nero-builder >/dev/null 2>&1; then
  useradd --create-home nero-builder
fi
build_root="$(mktemp -d /tmp/nero-bin.XXXXXX)"
trap 'rm -rf "$build_root"' EXIT

tar -xzf "$cli_archive" -C "$build_root"
cp "$work_root/LICENSE" "$build_root/LICENSE"
binary_sha="$(sha256sum "$build_root/nero" | cut -d' ' -f1)"
license_sha="$(sha256sum "$build_root/LICENSE" | cut -d' ' -f1)"

cat > "$build_root/PKGBUILD" <<PKGBUILD
pkgname=nero-bin
pkgver=${version}
pkgrel=1
pkgdesc='Nero local-first Markdown notes command-line and terminal interfaces'
arch=('x86_64')
url='https://github.com/saeidaliei/nero'
license=('MIT')
depends=('glibc' 'gcc-libs')
optdepends=('git: workspace versioning' 'rclone: remote backups')
provides=('nero')
conflicts=('nero')
source=('nero' 'LICENSE')
sha256sums=('${binary_sha}' '${license_sha}')

package() {
  install -Dm755 "\$srcdir/nero" "\$pkgdir/usr/bin/nero"
  install -Dm644 "\$srcdir/LICENSE" "\$pkgdir/usr/share/licenses/\$pkgname/LICENSE"
}
PKGBUILD

chown -R nero-builder:nero-builder "$build_root"
su -s /bin/bash -c "cd '$build_root' && makepkg --nodeps --nosign --cleanbuild --noconfirm" nero-builder

package_file="$(find "$build_root" -maxdepth 1 -type f -name 'nero-bin-*.pkg.tar.zst' -print -quit)"
if [[ -z "$package_file" ]]; then
  echo "makepkg did not produce an Arch package" >&2
  exit 1
fi
cp "$package_file" "$release_dir/"
printf 'Created Arch package: %s\n' "$release_dir/$(basename "$package_file")"
