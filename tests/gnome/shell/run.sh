#!/usr/bin/env bash
# Runs the compiled GNOME extension in a real, headless GNOME Shell in a
# container, opens its popup and screenshots it.
#
# The node tests in tests/gnome load panel.js and provider.js, never
# extension.js: nothing there stubs St or the shell's modules. So nothing but a
# real shell sees how the menu is drawn, and GNOME 45 drew every section in a
# scroll view one pixel tall for as long as it was supported without anyone
# noticing. This is the check for that. It is local and opt-in - it pulls a
# Fedora image with GNOME Shell in it - and it is not part of CI.
#
#   tests/gnome/shell/run.sh [--live | --json FILE] [--gnome VERSION]
#                            [--monitor WxH] [--out DIR]
#
# The panel defaults to tests/qml/fixtures/panel.json. --live serves what the
# installed `tailgauge panel --json` prints instead. --gnome picks the shell
# (45 or later, default 50) through the Fedora release that shipped it. The monitor
# defaults to 1280x720, small enough that the fixture's menu has to scroll.
# Screenshots and logs land in --out, readable by you alone, or in a fresh
# temporary directory.
set -euo pipefail

here="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
root="$(cd "$here/../../.." && pwd)"

json="$root/tests/qml/fixtures/panel.json"
live=0
gnome=50
monitor="1280x720"
out=""
while (($#)); do
  case $1 in
    --live) live=1 ;;
    --json) json="$2"; shift ;;
    --gnome) gnome="$2"; shift ;;
    --monitor) monitor="$2"; shift ;;
    --out) out="$2"; shift ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

if ! [[ $gnome =~ ^[0-9]+$ ]] || ((gnome < 45)); then
  echo "gnome-shell: --gnome takes 45 or later" >&2
  exit 2
fi
# Fedora 39 shipped GNOME 45, and every release since has moved both by one.
fedora=$((gnome - 6))

ext="$root/build"
if [[ ! -f $ext/tailgauge@arzaroth.github.io/extension.js ]]; then
  echo "gnome-shell: the extension is not built - run pnpm build" >&2
  exit 1
fi

engine="$(command -v podman || command -v docker || true)"
if [[ -z $engine ]]; then
  echo "gnome-shell: needs podman or docker" >&2
  exit 1
fi

# The container writes into a directory made for this run alone, because the
# mount relabels everything under it for SELinux; pointing that at a directory
# the caller named would relabel whatever it holds.
work="$(mktemp -d "${TMPDIR:-/tmp}/tailgauge-gnome-shell.XXXXXX")"
trap 'rm -rf "$work"' EXIT
if [[ -n $out ]]; then
  (umask 077 && mkdir -p "$out")
else
  trap - EXIT
  out="$work"
fi

if ((live)); then
  tailgauge panel --json >"$work/panel.json"
else
  cp "$json" "$work/panel.json"
fi
# The extension runs whatever binary its settings name; this one answers every
# command line with the recorded panel.
printf '#!/bin/sh\ncat /data/panel.json\n' >"$work/tailgauge"
chmod +x "$work/tailgauge"

image="tailgauge-gnome-shell:$gnome"
if ! "$engine" build -t "$image" --build-arg "FEDORA=$fedora" \
    -f "$here/Containerfile" "$here" >"$work/image.log" 2>&1; then
  cp "$work/image.log" "$out/" 2>/dev/null || true
  echo "gnome-shell: the image did not build; see $out/image.log" >&2
  exit 1
fi

"$engine" run --rm \
  -e MONITOR="$monitor" \
  -v "$ext/tailgauge@arzaroth.github.io:/ext/tailgauge@arzaroth.github.io:ro,z" \
  -v "$here/probe@tailgauge.test:/probe@tailgauge.test:ro,z" \
  -v "$here/boot.sh:/boot.sh:ro,z" \
  -v "$work/panel.json:/data/panel.json:ro,z" \
  -v "$work/tailgauge:/usr/local/bin/tailgauge:ro,z" \
  -v "$work:/out:z" \
  "$image" /boot.sh >/dev/null 2>&1 || true

if [[ $out != "$work" ]]; then
  for f in version.txt probe.txt shell.log mock.log top.png bottom.png done; do
    if [[ -f $work/$f ]]; then (umask 077 && cp "$work/$f" "$out/"); else rm -f "$out/$f"; fi
  done
fi

echo "$(cat "$out/version.txt" 2>/dev/null || echo 'GNOME Shell ?') at $monitor -> $out"
cat "$out/probe.txt" 2>/dev/null || true

status=0
if [[ ! -f $out/done ]]; then
  echo "gnome-shell: the probe never finished; see $out/shell.log" >&2
  status=1
fi
if grep -q '^error:' "$out/probe.txt" 2>/dev/null; then
  status=1
fi
# Read the log whole rather than through a pipe into `grep -q`: under pipefail
# the first grep dying of SIGPIPE would read as no match.
thrown="$(grep -A12 -E 'JS ERROR|Exception in callback' "$out/shell.log" 2>/dev/null || true)"
if [[ $thrown == *tailgauge@arzaroth.github.io* ]]; then
  echo "gnome-shell: the extension threw:" >&2
  echo "$thrown" >&2
  status=1
fi
exit "$status"
