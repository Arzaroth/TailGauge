#!/bin/bash
# Assembles the installable packages under build/.
#
# shared/model.js is the only copy of the Tailscale data model. The QML engine
# loads it as a plain shared script, so it carries no module syntax; the GNOME
# extension needs an ES module, so its copy is the same file with the export
# footer appended.

set -euo pipefail

root="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
build="$root/build"

PLASMOID_ID="org.tailgauge.plasmoid"
EXTENSION_UUID="tailgauge@arzaroth.github.io"

rm -rf "$build"
mkdir -p "$build"

# ---- Plasma ---------------------------------------------------------------
cp -r "$root/plasma/$PLASMOID_ID" "$build/$PLASMOID_ID"
mkdir -p "$build/$PLASMOID_ID/contents/code"
cp "$root/shared/model.js" "$build/$PLASMOID_ID/contents/code/model.js"

# ---- GNOME ----------------------------------------------------------------
cp -r "$root/gnome/$EXTENSION_UUID" "$build/$EXTENSION_UUID"
cat "$root/shared/model.js" "$root/shared/model.exports.mjs" >"$build/$EXTENSION_UUID/model.js"

if command -v glib-compile-schemas >/dev/null 2>&1; then
  glib-compile-schemas "$build/$EXTENSION_UUID/schemas"
else
  echo "build: glib-compile-schemas not found, skipping schema compilation" >&2
fi

if command -v zip >/dev/null 2>&1; then
  (cd "$build/$EXTENSION_UUID" && zip -qr "$build/$EXTENSION_UUID.shell-extension.zip" .)
fi

echo "Built:"
echo "  $build/$PLASMOID_ID"
echo "  $build/$EXTENSION_UUID"
[[ -f "$build/$EXTENSION_UUID.shell-extension.zip" ]] && echo "  $build/$EXTENSION_UUID.shell-extension.zip"
exit 0
