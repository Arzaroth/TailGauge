#!/bin/bash
# Usage: scripts/package.sh <tailgauge-vX.Y.Z-linux-<arch>.tar.gz> <dist>
#
# Builds the .deb and the .rpm for a release archive into <dist>, from the
# archive itself, so a package carries the bytes `--update` would install.
# Needs nfpm on PATH. The Omarchy widget is left out: the shell loads
# third-party plugins from ~/.config/omarchy/plugins only.

set -euo pipefail

die() {
  echo "package: $*" >&2
  exit 1
}

archive="${1:-}"
dist="${2:-}"
if [[ ! -f $archive || -z $dist ]]; then
  echo "usage: scripts/package.sh <tailgauge-vX.Y.Z-linux-<arch>.tar.gz> <dist>" >&2
  exit 2
fi
[[ $(basename "$archive") =~ ^tailgauge-v([0-9]+\.[0-9]+\.[0-9]+)-linux-(x86_64|aarch64)\.tar\.gz$ ]] ||
  die "$archive is not a linux release archive"
version="${BASH_REMATCH[1]}"
case "${BASH_REMATCH[2]}" in
x86_64) arch=amd64 ;;
aarch64) arch=arm64 ;;
esac
command -v nfpm >/dev/null 2>&1 || die "nfpm is required - https://nfpm.goreleaser.com"

root="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
archive="$(readlink -f "$archive")"
mkdir -p "$dist"
dist="$(readlink -f "$dist")"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

mkdir -p "$stage/archive"
tar -xzf "$archive" -C "$stage/archive"
cp "$root/README.md" "$root/CHANGELOG.md" "$stage/archive/"

# The unit runs the binary from ~/.local/bin, where install.sh puts it.
sed 's|^ExecStart=%h/\.local/bin/tailgauge |ExecStart=/usr/bin/tailgauge |' \
  "$stage/archive/systemd/tailgauge-receive.service" >"$stage/tailgauge-receive.service"
grep -qx 'ExecStart=/usr/bin/tailgauge receive' "$stage/tailgauge-receive.service" ||
  die "the unit's ExecStart is not the one this rewrites"

cp "$root/packaging/nfpm.yaml" "$stage/nfpm.yaml"
for alias in ctl watch notify send receive file-select copy tui; do
  printf '  - src: tailgauge\n    dst: /usr/bin/tailgauge-%s\n    type: symlink\n' "$alias"
done >>"$stage/nfpm.yaml"

for format in deb rpm; do
  (cd "$stage" && ARCH="$arch" VERSION="$version" \
    nfpm package --config nfpm.yaml --packager "$format" --target "$dist/")
done
