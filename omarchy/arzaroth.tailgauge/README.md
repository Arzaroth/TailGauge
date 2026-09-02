# TailGauge for Omarchy

A bar widget for the Omarchy 4 Quickshell shell: connection state, on/off,
account switching, exit nodes including Mullvad regions, machine browsing with
copy actions, and Taildrop file sending.

Omarchy ships its own `omarchy.tailscale` widget, which is where all of this
came from. This one is the same panel rebuilt on TailGauge's shared model, so it
carries what the Plasma and GNOME frontends carry and moves when they do: the
local machine as a row of its own, offline machines, the owner of each machine,
a machine search, and an update banner. Both can sit in the bar at once; drop
theirs with `omarchy plugin disable omarchy.tailscale`.

The QML is strictly a display. `Model.js` - the shared model, copied in by the
build - decides which sections exist, what every row says and where the cursor
can land; `Service.qml` drives the `tailscale` CLI and the `tailgauge-*`
helpers. Nothing here re-derives panel content.

## Install

From a checkout:

```bash
scripts/install.sh --omarchy
```

That builds the plugin, installs the helpers into `~/.local/bin`, enables the
Taildrop receive service, copies this folder to
`~/.config/omarchy/plugins/arzaroth.tailgauge/`, and enables the widget. Pass
`--placement=left|center|right` to choose a bar section; without it the
manifest's `right` applies. Add `--no-taildrop` to skip the receive service.

## Panel

- **Hero** - the machine name, the connection state, and the on/off switch. The
  switch flips optimistically the instant you click it, then reconciles with the
  next `tailscale status`.
- **This device** - your own machine, with the same copy actions its peers get.
- **Connections** - Tailscale profiles, when more than one is signed in. Offers
  to `pkexec tailscale set --operator=$USER` when the daemon refuses profile
  access.
- **Exit nodes** - every tailnet exit node, the Mullvad regions you actually
  use, and a searchable picker for the full fleet.
- **Machines** - every peer with its address and owner, copy actions, and a
  Taildrop send button where the tailnet allows file sharing. Offline machines
  sort to the bottom and say so. Past eight machines the section grows a search
  field.
- **Update banner** - when a newer TailGauge release is out, with an install
  button when `tailgauge-update` can apply it in place.

## Interactions

- Bar icon: left = panel, right = toggle Tailscale, middle = refresh.
- Panel: `j`/`k` or arrows move the cursor, Enter activates, `t` toggles, `r`
  refreshes, `c` / `n` / `d` copy the selected machine's IP, name and MagicDNS
  name, `s` sends files to it, Tab moves to the neighbouring bar panel, Esc
  closes.
- IPC: `omarchy-shell arzaroth.tailgauge <open|close|toggle|refresh|up|down|toggleTailscale|status>`.

## Settings

Widget settings live inline on its entry in `~/.config/omarchy/shell.json`:

| Key | Default | What it does |
|---|---|---|
| `refreshIntervalSec` | `30` | Floor under the poll when the panel is closed. An open panel polls every 3s, and `tailgauge-watch` reports changes within a second either way |

Numbers need `--json`, or they land in `shell.json` as strings:

```bash
omarchy bar set arzaroth.tailgauge refreshIntervalSec 60 --json
```

The recent Mullvad regions are written back to the same entry as you use them.

## Developing

The shell watches the plugin folder and logs `Local plugin changed, reloading`,
but it keeps the already-instantiated widget: **QML edits only take effect after
`omarchy-restart-shell`.** Manifest changes are picked up by
`omarchy-shell shell rescanPlugins`.

Read the shell's own log for QML errors:

```bash
qs log --pid "$(pgrep -f 'quickshell -n -p /usr/share/omarchy/shell')" -t 50
```

Symlinks are refused anywhere inside a plugin folder, so the installer copies
this directory rather than linking it, and `scripts/build.sh` copies the
JavaScript it compiles from `shared/model.ts` in as `Model.js`. `Model.js` is
build output, not a source file. **Edit `shared/model.ts`, never the copy.**
Re-run `scripts/install.sh --omarchy` to push local edits.
