#!/bin/bash
# Assembles the installable packages under build/.
#
# Every frontend reads its panel from the tailgauge binary, so nothing shared
# is compiled here: this assembles three payloads out of the sources, and the
# GNOME extension's TypeScript is the only thing that needs a compiler.

set -euo pipefail

root="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
build="$root/build"
tsc="$root/node_modules/.bin/tsc"

# scripts/install.sh runs this from a fresh clone, so the toolchain is fetched
# here rather than left as a step to remember. pnpm owns the lockfile; npm is
# the fallback because it is the one package manager an end user is guaranteed
# to have. That path is best-effort: npm cannot read pnpm-lock.yaml, so it
# re-resolves the ranges in package.json and may pick a different patch than CI
# built with. --no-package-lock keeps it from leaving a second lockfile behind.
if [[ ! -x $tsc ]]; then
  echo "build: installing the TypeScript toolchain" >&2
  if command -v pnpm >/dev/null 2>&1; then
    (cd "$root" && pnpm install --frozen-lockfile)
  elif command -v npm >/dev/null 2>&1; then
    echo "build: pnpm not found - resolving with npm, which cannot read pnpm-lock.yaml" >&2
    (cd "$root" && npm install --no-package-lock)
  else
    echo "build: pnpm or npm is required to compile the TypeScript sources" >&2
    exit 1
  fi
fi

if [[ ! -x $tsc ]]; then
  echo "build: $tsc is still missing after installing the toolchain" >&2
  exit 1
fi

PLASMOID_ID="org.tailgauge.plasmoid"
EXTENSION_UUID="tailgauge@arzaroth.github.io"
PLUGIN_ID="arzaroth.tailgauge"

rm -rf "$build"
mkdir -p "$build"

# ---- TypeScript -----------------------------------------------------------
# Only the GNOME extension is compiled now. The panel itself is resolved by the
# tailgauge binary, so there is no shared model to emit twice.
"$tsc" -p "$root/tsconfig.gnome.json"

# ---- Plasma ---------------------------------------------------------------
cp -r "$root/plasma/$PLASMOID_ID" "$build/$PLASMOID_ID"

# ---- GNOME ----------------------------------------------------------------
cp -r "$root/gnome/$EXTENSION_UUID" "$build/$EXTENSION_UUID"
# The .ts sources are the input to the compiler, not part of the package.
rm -f "$build/$EXTENSION_UUID"/*.ts
cp "$build/.ts/gnome/gnome/$EXTENSION_UUID"/*.js "$build/$EXTENSION_UUID/"

# ---- Omarchy --------------------------------------------------------------
# The shell's plugin registry refuses symlinks anywhere inside a plugin folder,
# so the model is copied in beside the QML rather than linked.
# No model copy: the Omarchy widget asks the binary for its panel rather than
# resolving one, so there is nothing here for it to import.
cp -r "$root/omarchy/$PLUGIN_ID" "$build/$PLUGIN_ID"

# GNOME reads the extension's own schema source the moment schemas/ exists, so
# a directory holding the XML and no compiled blob is worse than no schemas at
# all: the extension fails at every enable instead of falling back. Refusing
# here is what stops scripts/install.sh replacing a working extension with one
# the shell cannot load.
if ! command -v glib-compile-schemas >/dev/null 2>&1; then
  echo "build: glib-compile-schemas is required to package the GNOME extension" >&2
  echo "build: install the glib2 tools (Debian/Ubuntu: libglib2.0-bin)" >&2
  exit 1
fi
glib-compile-schemas --strict "$build/$EXTENSION_UUID/schemas"

if command -v zip >/dev/null 2>&1; then
  (cd "$build/$EXTENSION_UUID" && zip -qr "$build/$EXTENSION_UUID.shell-extension.zip" .)
fi

echo "Built:"
echo "  $build/$PLASMOID_ID"
echo "  $build/$EXTENSION_UUID"
echo "  $build/$PLUGIN_ID"
[[ -f "$build/$EXTENSION_UUID.shell-extension.zip" ]] && echo "  $build/$EXTENSION_UUID.shell-extension.zip"
exit 0
