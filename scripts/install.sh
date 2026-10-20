#!/bin/bash
# Usage: scripts/install.sh [--plasma] [--gnome] [--omarchy] [--no-taildrop]
#                           [--placement=left|center|right]
# With no target flag, installs for whichever desktop is running.

set -euo pipefail

PLASMOID_ID="org.tailgauge.plasmoid"
EXTENSION_UUID="tailgauge@arzaroth.github.io"
PLUGIN_ID="arzaroth.tailgauge"

usage() {
  cat <<'EOF'
usage: install.sh [--plasma] [--gnome] [--omarchy] [--no-taildrop]
                  [--placement=left|center|right]

With no target flag, installs for whichever desktop is running.
EOF
}

die() {
  echo "install: $*" >&2
  exit 1
}

main() {
  local want_plasma=false want_gnome=false want_omarchy=false
  local want_taildrop=true explicit=false placement=""

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
    --omarchy)
      want_omarchy=true
      explicit=true
      ;;
    --placement=*) placement="${1#*=}" ;;
    --no-taildrop) want_taildrop=false ;;
    -h | --help)
      usage
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
    *Hyprland* | *omarchy*) want_omarchy=true ;;
    *)
      echo "install: could not detect the desktop, pass --plasma, --gnome or --omarchy" >&2
      exit 2
      ;;
    esac
  fi

  case "$placement" in
  "" | left | center | right) ;;
  *)
    echo "install: --placement must be left, center or right" >&2
    exit 2
    ;;
  esac

  command -v tailscale >/dev/null 2>&1 ||
    echo "install: warning - the tailscale CLI is not on PATH; the widget will show 'Not installed'" >&2

  local root binary plasmoid extension extension_zip plugin unit
  root="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"
  "$root/scripts/build.sh" >/dev/null
  command -v cargo >/dev/null 2>&1 || die "cargo is required to build the tailgauge binary - https://rustup.rs"
  (cd "$root" && cargo build --release --locked)
  binary="$root/target/release/tailgauge"
  plasmoid="$root/build/$PLASMOID_ID"
  extension="$root/build/$EXTENSION_UUID"
  extension_zip="$root/build/$EXTENSION_UUID.shell-extension.zip"
  plugin="$root/build/$PLUGIN_ID"
  unit="$root/systemd/tailgauge-receive.service"

  # ---- the binary ---------------------------------------------------------
  # One executable with a subcommand per job, and a symlink per subcommand
  # under the name the shell helper had, so an existing key binding still works
  # and the binary can read the name it was invoked as.
  local bindir="$HOME/.local/bin"
  mkdir -p "$bindir"
  install -m 755 "$binary" "$bindir/tailgauge"
  # No `update` alias: updating is a flag on the binary, not a subcommand. An
  # install from before that carries the old symlink, and the binary answers
  # `tailgauge-update` with a usage error, so the key binding that used to run
  # it breaks quietly. Take it with us.
  rm -f "$bindir/tailgauge-update"
  for alias in ctl watch notify send receive file-select copy tui; do
    ln -sfn tailgauge "$bindir/tailgauge-$alias"
  done
  echo "Installed tailgauge into $bindir"

  case ":$PATH:" in
  *":$bindir:"*) ;;
  *) echo "install: warning - $bindir is not on your PATH" >&2 ;;
  esac

  # ---- Taildrop receiver --------------------------------------------------
  if $want_taildrop; then
    local unitdir="$HOME/.config/systemd/user"
    mkdir -p "$unitdir"
    install -m 644 "$unit" "$unitdir/tailgauge-receive.service"
    if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
      systemctl --user daemon-reload
      systemctl --user enable --now tailgauge-receive.service
      echo "Enabled tailgauge-receive.service"
    else
      echo "install: no systemd user session, skipping tailgauge-receive.service" >&2
    fi
  fi

  # ---- Plasma -------------------------------------------------------------
  if $want_plasma; then
    command -v kpackagetool6 >/dev/null 2>&1 || die "kpackagetool6 not found, cannot install the plasmoid"
    if kpackagetool6 -t Plasma/Applet -l 2>/dev/null | grep -qx "$PLASMOID_ID"; then
      kpackagetool6 -t Plasma/Applet -u "$plasmoid"
    else
      kpackagetool6 -t Plasma/Applet -i "$plasmoid"
    fi
    echo "Installed $PLASMOID_ID - add it from Add Widgets."
    # plasmashell is a transient app-plasmashell@<hash>.service under a systemd
    # session and plasma-plasmashell.service elsewhere, so the unit is looked
    # up rather than assumed.
    local shell_unit
    shell_unit=$(systemctl --user list-units --no-legend 'app-plasmashell@*.service' 2>/dev/null | awk '{print $1}' | head -1)
    [[ -z $shell_unit ]] && systemctl --user cat plasma-plasmashell.service >/dev/null 2>&1 && shell_unit=plasma-plasmashell.service
    echo "QML changes need the shell reloaded:"
    if [[ -n $shell_unit ]]; then
      echo "  systemctl --user restart $shell_unit"
    else
      echo "  plasmashell --replace &"
    fi
  fi

  # ---- GNOME --------------------------------------------------------------
  if $want_gnome; then
    if command -v gnome-extensions >/dev/null 2>&1 && [[ -f $extension_zip ]]; then
      gnome-extensions install --force "$extension_zip"
    else
      # Stage and swap, so an install that fails halfway leaves the extension
      # that was already there rather than nothing. The old copy is moved aside
      # instead of deleted, which closes the window down to a rename that
      # either happens or does not. Both scratch directories sit a level above
      # extensions/, since the shell reads every child of that one as an
      # extension and would load a half-written copy under a name that is not
      # its UUID.
      local target extdir staging backup
      target="$HOME/.local/share/gnome-shell/extensions/$EXTENSION_UUID"
      extdir="$(dirname "$target")"
      mkdir -p "$extdir"
      staging="$extdir/../.tailgauge-install.$$"
      backup="$extdir/../.tailgauge-backup.$$"
      rm -rf "$staging" "$backup"
      if ! cp -aL "$extension" "$staging"; then
        rm -rf "$staging"
        die "could not stage the extension"
      fi
      if [[ -e $target ]] && ! mv "$target" "$backup"; then
        rm -rf "$staging"
        die "could not move the installed extension aside"
      fi
      if ! mv "$staging" "$target"; then
        rm -rf "$staging"
        echo "install: could not install the extension" >&2
        if [[ -e $backup ]] && ! mv "$backup" "$target"; then
          echo "install: the extension it replaced is left in $backup" >&2
        fi
        exit 1
      fi
      rm -rf "$backup"
    fi
    echo "Installed $EXTENSION_UUID - enable it with:"
    echo "  gnome-extensions enable $EXTENSION_UUID"
    echo "On Xorg, restart the shell with Alt+F2 r; on Wayland, log out and back in."
  fi

  # ---- Omarchy ------------------------------------------------------------
  if $want_omarchy; then
    command -v omarchy-plugin-validate >/dev/null 2>&1 ||
      die "omarchy-plugin-validate not found, this needs Omarchy 4 or newer"
    omarchy-plugin-validate "$plugin"

    # The plugin registry refuses symlinks anywhere inside a plugin folder, so
    # this is a copy. Re-run the installer to pick up local edits.
    # Copy beside the target and swap, so a copy that runs out of disk halfway
    # leaves the installed widget alone rather than deleted.
    local plugindir staging
    plugindir="$HOME/.config/omarchy/plugins/$PLUGIN_ID"
    staging="$plugindir.new.$$"
    mkdir -p "$(dirname "$plugindir")"
    rm -rf "$staging"
    cp -aL "$plugin" "$staging"
    rm -rf "$plugindir"
    mv "$staging" "$plugindir"
    omarchy-shell -q shell rescanPlugins >/dev/null 2>&1 || true

    if omarchy-plugin-list 2>/dev/null | grep -q "^$PLUGIN_ID .*enabled"; then
      echo "Installed $PLUGIN_ID - already enabled, leaving its bar placement alone."
    else
      # shellcheck disable=SC2086 # an empty placement means "use the manifest's".
      omarchy-plugin-enable "$PLUGIN_ID" $placement
      echo "Installed $PLUGIN_ID into the bar."
    fi

    echo "Omarchy also ships omarchy.tailscale, which covers the same ground:"
    echo "  omarchy plugin disable omarchy.tailscale"
    # The registry reloads a changed manifest but keeps the widget it already
    # built, so QML edits only land on a restart.
    echo "QML changes need the shell restarted:"
    echo "  omarchy-restart-shell"
  fi
}

main "$@"
