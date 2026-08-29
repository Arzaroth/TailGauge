#!/bin/bash
# Usage: scripts/install.sh [--plasma] [--gnome] [--no-taildrop]
# With no target flag, installs for whichever desktop is running.

set -euo pipefail

root="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
build="$root/build"

PLASMOID_ID="org.tailgauge.plasmoid"
EXTENSION_UUID="tailgauge@arzaroth.github.io"

want_plasma=false
want_gnome=false
want_taildrop=true
explicit=false

while (($# > 0)); do
  case "$1" in
  --plasma)
    want_plasma=true
    explicit=true
    ;;
  --gnome)
    want_gnome=true
    explicit=true
    ;;
  --no-taildrop) want_taildrop=false ;;
  -h | --help)
    sed -n '2,4p' "$0"
    exit 0
    ;;
  *)
    echo "install: unknown option $1" >&2
    exit 2
    ;;
  esac
  shift
done

if ! $explicit; then
  case "${XDG_CURRENT_DESKTOP:-}" in
  *KDE* | *plasma*) want_plasma=true ;;
  *GNOME*) want_gnome=true ;;
  *)
    echo "install: could not detect the desktop, pass --plasma or --gnome" >&2
    exit 2
    ;;
  esac
fi

command -v tailscale >/dev/null 2>&1 ||
  echo "install: warning - the tailscale CLI is not on PATH; the widget will show 'Not installed'" >&2

"$root/scripts/build.sh" >/dev/null

# ---- helpers --------------------------------------------------------------
bindir="$HOME/.local/bin"
mkdir -p "$bindir"
for helper in tailgauge-copy tailgauge-notify tailgauge-file-select tailgauge-send tailgauge-receive tailgauge-update tailgauge-watch; do
  install -m 755 "$root/bin/$helper" "$bindir/$helper"
done
echo "Installed helpers into $bindir"

case ":$PATH:" in
*":$bindir:"*) ;;
*) echo "install: warning - $bindir is not on your PATH" >&2 ;;
esac

# ---- Taildrop receiver ----------------------------------------------------
if $want_taildrop; then
  unitdir="$HOME/.config/systemd/user"
  mkdir -p "$unitdir"
  install -m 644 "$root/systemd/tailgauge-receive.service" "$unitdir/tailgauge-receive.service"
  if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
    systemctl --user daemon-reload
    systemctl --user enable --now tailgauge-receive.service
    echo "Enabled tailgauge-receive.service"
  else
    echo "install: no systemd user session, skipping tailgauge-receive.service" >&2
  fi
fi

# ---- Plasma ---------------------------------------------------------------
if $want_plasma; then
  if ! command -v kpackagetool6 >/dev/null 2>&1; then
    echo "install: kpackagetool6 not found, cannot install the plasmoid" >&2
    exit 1
  fi
  if kpackagetool6 -t Plasma/Applet -l 2>/dev/null | grep -qx "$PLASMOID_ID"; then
    kpackagetool6 -t Plasma/Applet -u "$build/$PLASMOID_ID"
  else
    kpackagetool6 -t Plasma/Applet -i "$build/$PLASMOID_ID"
  fi
  echo "Installed $PLASMOID_ID - add it from Add Widgets."
  # plasmashell is a transient app-plasmashell@<hash>.service under a systemd
  # session and plasma-plasmashell.service elsewhere, so the unit is looked up
  # rather than assumed.
  unit=$(systemctl --user list-units --no-legend 'app-plasmashell@*.service' 2>/dev/null | awk '{print $1}' | head -1)
  [[ -z $unit ]] && systemctl --user cat plasma-plasmashell.service >/dev/null 2>&1 && unit=plasma-plasmashell.service
  echo "QML changes need the shell reloaded:"
  if [[ -n $unit ]]; then
    echo "  systemctl --user restart $unit"
  else
    echo "  plasmashell --replace &"
  fi
fi

# ---- GNOME ----------------------------------------------------------------
if $want_gnome; then
  zip="$build/$EXTENSION_UUID.shell-extension.zip"
  if command -v gnome-extensions >/dev/null 2>&1 && [[ -f $zip ]]; then
    gnome-extensions install --force "$zip"
  else
    target="$HOME/.local/share/gnome-shell/extensions/$EXTENSION_UUID"
    rm -rf "$target"
    mkdir -p "$(dirname "$target")"
    cp -r "$build/$EXTENSION_UUID" "$target"
  fi
  echo "Installed $EXTENSION_UUID - enable it with:"
  echo "  gnome-extensions enable $EXTENSION_UUID"
  echo "On Xorg, restart the shell with Alt+F2 r; on Wayland, log out and back in."
fi
