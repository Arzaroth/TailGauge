#!/bin/sh
# Usage: scripts/check-package.sh
#
# Run on a system with a TailGauge package installed: fails unless the
# package put the binary, every alias, the Taildrop unit and both desktop
# payloads where each is looked for.

set -eu

fail() {
  echo "check-package: $*" >&2
  exit 1
}

tailgauge --version
for alias in ctl watch notify send receive file-select copy tui; do
  [ "$(readlink "/usr/bin/tailgauge-$alias")" = tailgauge ] || fail "no /usr/bin/tailgauge-$alias"
done
tailgauge-ctl --help >/dev/null || fail "tailgauge-ctl does not reach the ctl subcommand"
grep -qx 'ExecStart=/usr/bin/tailgauge receive' /usr/lib/systemd/user/tailgauge-receive.service ||
  fail "the Taildrop unit does not run /usr/bin/tailgauge"
for file in \
  /usr/share/plasma/plasmoids/org.tailgauge.plasmoid/metadata.json \
  /usr/share/plasma/plasmoids/org.tailgauge.plasmoid/contents/ui/main.qml \
  /usr/share/gnome-shell/extensions/tailgauge@arzaroth.github.io/metadata.json \
  /usr/share/gnome-shell/extensions/tailgauge@arzaroth.github.io/schemas/gschemas.compiled; do
  [ -f "$file" ] || fail "no $file"
done
echo "check-package: everything is in place"
