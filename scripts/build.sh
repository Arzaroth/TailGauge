#!/bin/bash
# Assembles the installable packages under build/.
#
# shared/model.js is the only copy of the Tailscale data model. Both QML engines
# load it as a plain shared script, so it carries no module syntax; the GNOME
# extension needs an ES module, so its copy is the same file with the export
# footer appended.

set -euo pipefail

root="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
build="$root/build"

PLASMOID_ID="org.tailgauge.plasmoid"
EXTENSION_UUID="tailgauge@arzaroth.github.io"
PLUGIN_ID="arzaroth.tailgauge"

rm -rf "$build"
mkdir -p "$build"

# ---- Plasma ---------------------------------------------------------------
cp -r "$root/plasma/$PLASMOID_ID" "$build/$PLASMOID_ID"
mkdir -p "$build/$PLASMOID_ID/contents/code"
cp "$root/shared/model.js" "$build/$PLASMOID_ID/contents/code/model.js"

# ---- GNOME ----------------------------------------------------------------
cp -r "$root/gnome/$EXTENSION_UUID" "$build/$EXTENSION_UUID"
cat "$root/shared/model.js" "$root/shared/model.exports.mjs" >"$build/$EXTENSION_UUID/model.js"

# ---- Omarchy --------------------------------------------------------------
# The shell's plugin registry refuses symlinks anywhere inside a plugin folder,
# so the model is copied in beside the QML rather than linked.
cp -r "$root/omarchy/$PLUGIN_ID" "$build/$PLUGIN_ID"
cp "$root/shared/model.js" "$build/$PLUGIN_ID/Model.js"

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
echo "  $build/$PLUGIN_ID"
[[ -f "$build/$EXTENSION_UUID.shell-extension.zip" ]] && echo "  $build/$EXTENSION_UUID.shell-extension.zip"
exit 0
