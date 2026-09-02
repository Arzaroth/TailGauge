#!/bin/bash
# Assembles the installable packages under build/.
#
# shared/model.ts is the only copy of the Tailscale data model. It compiles once
# and ships twice: as the ES module the GNOME extension imports, and with the
# trailing export statement removed as the plain shared script both QML engines
# load, which cannot carry module syntax.

set -euo pipefail

root="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
build="$root/build"
tsc="$root/node_modules/.bin/tsc"

# scripts/install.sh runs this from a fresh clone, so the toolchain is fetched
# here rather than left as a step to remember.
if [[ ! -x $tsc ]]; then
  if ! command -v npm >/dev/null 2>&1; then
    echo "build: npm is required to compile the TypeScript sources" >&2
    exit 1
  fi
  echo "build: installing the TypeScript toolchain" >&2
  if [[ -f "$root/package-lock.json" ]]; then
    (cd "$root" && npm ci)
  else
    (cd "$root" && npm install)
  fi
fi

if [[ ! -x $tsc ]]; then
  echo "build: $tsc is still missing after npm install" >&2
  exit 1
fi

PLASMOID_ID="org.tailgauge.plasmoid"
EXTENSION_UUID="tailgauge@arzaroth.github.io"
PLUGIN_ID="arzaroth.tailgauge"

rm -rf "$build"
mkdir -p "$build"

# ---- TypeScript -----------------------------------------------------------
"$tsc" -p "$root/tsconfig.model.json"
"$tsc" -p "$root/tsconfig.gnome.json"
"$tsc" -p "$root/test/tsconfig.json"

model_esm="$build/.ts/model/model.js"
model_plain="$build/.ts/model/model.plain.js"

# The compiler emits the export statement as the file's last line, which is the
# one thing the QML engines cannot parse. Dropping it is what separates the two
# shipped copies, so it has to have actually been there.
if [[ $(tail -n 1 "$model_esm") != export\ * ]]; then
  echo "build: the compiled model does not end in its export statement" >&2
  exit 1
fi
sed '$d' "$model_esm" >"$model_plain"

# ---- Plasma ---------------------------------------------------------------
cp -r "$root/plasma/$PLASMOID_ID" "$build/$PLASMOID_ID"
mkdir -p "$build/$PLASMOID_ID/contents/code"
cp "$model_plain" "$build/$PLASMOID_ID/contents/code/model.js"

# ---- GNOME ----------------------------------------------------------------
cp -r "$root/gnome/$EXTENSION_UUID" "$build/$EXTENSION_UUID"
# The .ts sources are the input to the compiler, not part of the package.
rm -f "$build/$EXTENSION_UUID"/*.ts
cp "$build/.ts/gnome/gnome/$EXTENSION_UUID"/*.js "$build/$EXTENSION_UUID/"
cp "$model_esm" "$build/$EXTENSION_UUID/model.js"

# ---- Omarchy --------------------------------------------------------------
# The shell's plugin registry refuses symlinks anywhere inside a plugin folder,
# so the model is copied in beside the QML rather than linked.
cp -r "$root/omarchy/$PLUGIN_ID" "$build/$PLUGIN_ID"
cp "$model_plain" "$build/$PLUGIN_ID/Model.js"

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
