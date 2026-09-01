# TailGauge

Tailscale in your **KDE Plasma 6** and **GNOME Shell** panel: connection state, on/off, account switching, exit nodes including Mullvad regions, machine browsing with copy actions, and Taildrop file sending.

It is a port of the first-party `omarchy.tailscale` panel plugin that ships with [Omarchy 4 (Quattro)](https://github.com/basecamp/omarchy), moved off Hyprland/Quickshell onto the two big desktops. See [NOTICE](NOTICE) for what was carried over and what was rewritten.

There is no daemon and no service to run. Both frontends drive the `tailscale` CLI directly and share one data model.

## Features

- **Panel indicator**: the Tailscale mark drawn natively as a 3×3 dot grid, slashed when disconnected, badged when the device needs authorization. Optionally followed by the machine name.
- **On/off**: a switch at the top of the panel. The UI flips optimistically the instant you click, then reconciles with the next `tailscale status`.
- **Login**: when the device needs authorizing, the auth URL is scraped out of `tailscale up` as it prints and opened in your browser.
- **This device**: your own machine at the top of the panel, with the same copy actions its peers get, because copying your own tailnet address is what you came for half the time.
- **Connections**: switch between Tailscale profiles when more than one is signed in. Offers to `pkexec tailscale set --operator=$USER` when the daemon refuses profile access.
- **Exit nodes**: every tailnet exit node, plus a shortlist of the Mullvad regions you actually use, plus a searchable picker for the full Mullvad fleet.
- **Machines**: every peer with its IP and MagicDNS name, copy actions for name / DNS name / IPv4 / IPv6, and a Taildrop send button where the tailnet allows file sharing. Offline machines sort to the bottom and say so, because a sleeping laptop's address is exactly what you need to wake it.
- **Taildrop receive**: a systemd user service parks on `tailscale file get --wait`, delivers into `~/Downloads`, and announces each file with a notification you can click to open.
- **Keyboard**: `t` toggle, `r` refresh, and on any row that offers them `c` copy IP, `n` copy name, `d` copy DNS name, `s` send files. Arrows and Enter work as they do natively on each desktop.

## Requirements

- `tailscale` on `PATH`
- KDE Plasma 6.0+ or GNOME Shell 45+
- `zenity` or `kdialog` for the Taildrop file chooser
- `wl-clipboard`, `xclip`, or `xsel` for the Plasma copy actions (GNOME uses `St.Clipboard`)
- Taildrop enabled for the tailnet, to send files

## Install

```bash
git clone https://github.com/Arzaroth/TailGauge
cd TailGauge
scripts/install.sh
```

The installer detects the running desktop. Pass `--plasma` or `--gnome` to force one, and `--no-taildrop` to skip the receive service.

**Plasma**: adds the widget to your library. Add it from *Add Widgets*. QML changes need the shell reloaded, and the unit name depends on how the session started it:

```bash
systemctl --user restart "$(systemctl --user list-units --no-legend 'app-plasmashell@*.service' | awk '{print $1}' | head -1)"
```

**GNOME**: installs the extension, then enable it:

```bash
gnome-extensions enable tailgauge@arzaroth.github.io
```

On Xorg, restart the shell with `Alt+F2` `r`. On Wayland, log out and back in.

## Outside the panel

Omarchy's plugin answers to `omarchy-shell omarchy.tailscale toggle`. TailGauge has no IPC and needs none: both panels block on `tailscale debug watch-ipn`, so anything that changes tailscaled's state shows up in them within a second, whoever changed it.

```bash
tailgauge-ctl status              # exits 0 connected, 3 not
tailgauge-ctl toggle              # off if it is on, on if it is off
tailgauge-ctl up                  # opens the login page when it needs one
tailgauge-ctl down
tailgauge-ctl exit-node           # print the current one
tailgauge-ctl exit-node de-ber-wg-001.mullvad.ts.net
tailgauge-ctl exit-node off
tailgauge-ctl exit-nodes          # what this tailnet offers
```

Run from a key binding there is no terminal to print on, so `up` opens the login page itself and a failure arrives as a notification instead.

**GNOME**: *Settings* in the panel menu has a **Shortcut** row. It is empty until you set one, and it toggles Tailscale in the running extension, so it works on a store install with no helpers on `PATH`.

**Plasma**: bind the helper as a custom command in *System Settings > Keyboard > Shortcuts > Add > Command*, with `tailgauge-ctl toggle` - `~/.local/bin/tailgauge-ctl` if that directory is not on the session's `PATH`. The widget's own popup is bindable without any of this, from *Configure Keyboard Shortcuts* in its context menu.

## Distribution

TailGauge has no binary, so there is nothing to self-replace the way a compiled tool does. Instead both desktops already ship an update mechanism, and TailGauge uses them.

| Channel | Installs | Updates |
|---|---|---|
| [store.kde.org](https://store.kde.org) | the Plasma widget | *Add Widgets* shows updates; KNewStuff tracks the version |
| [extensions.gnome.org](https://extensions.gnome.org) | the GNOME extension | the Extensions app applies them on next session |
| GitHub releases | everything, including the helpers | `tailgauge-update` |
| Distro package | everything, as one unit | the package manager |

**Neither store can install `bin/` or the systemd unit** - the KDE Store ships a kpackage, EGO ships an extension zip. A store-installed TailGauge is the panel only: status, toggle, connections, exit nodes, machines and copy actions all work; **Taildrop send does not appear**, because the panel checks for `tailgauge-send` before offering it. Install the helpers from the release archive to get it back:

```bash
curl -fsSL https://github.com/Arzaroth/TailGauge/releases/latest/download/tailgauge-v0.0.2-helpers.tar.gz | tar -xz
install -m 755 bin/* ~/.local/bin/
```

### Updating

```bash
tailgauge-update             # is there a newer release?
tailgauge-update --apply     # install it
```

The check is cached for six hours, so the panel polling it costs nothing. Both panels show a banner when an update is out, with an **Install it now** row when TailGauge can do it itself.

It refuses to update anything it did not install. A widget registered with KNewStuff, an extension carrying EGO's `_generated` stamp, or anything under `/usr` is left to whoever owns it, and the banner says so instead of offering the button. `--force` overrides.

### Release assets

Tagging `vX.Y.Z` publishes:

- `tailgauge-vX.Y.Z-plasmoid.plasmoid` - `kpackagetool6 -t Plasma/Applet -i`, and the KDE Store upload
- `tailgauge-vX.Y.Z-gnome-shell-extension.zip` - `gnome-extensions install`, and the EGO upload
- `tailgauge-vX.Y.Z-helpers.tar.gz` - `bin/` and the systemd unit

`test/distribution.test.mjs` fails the build if those names stop matching what `tailgauge-update` downloads, or if the three declared versions drift apart.

## The parity rule

The two frontends do not each decide what to draw. `shared/model.js` resolves the whole panel and hands both of them the same answer:

```js
resolvePanel(state, {t, recentRegions, mullvadQuery, mullvadPickerOpen, phraseIndex})
  -> {header, status, sections: [{id, title, visible, empty, rows}], navigation}
```

It owns **which sections exist, in what order, when each is visible, which rows it holds, every label and empty state, which rows are cursor stops, and in what order**. A row arrives fully formed - label, sublabel, icon, ornament state, busy state, tooltip, its actions and its copy options - and a frontend decides only which widget draws it. Keyboard traversal is an index into `panel.navigation`, so neither desktop carries a focus state machine the other could disagree with.

Strings are chosen by the model and resolved by the caller: Plasma passes `i18n`, GNOME passes `gettext`.

This is enforced, not remembered. `test/parity.test.mjs` fails the build if a user-visible string is written in both frontends, if either re-derives section visibility or the exit-node and copy-option lists, or if the two services stop handing `resolvePanel()` the same snapshot shape.

## Layout

```
shared/model.js          the only copy of the data model AND the panel layout
shared/model.exports.mjs the export footer appended for the GNOME build
plasma/                  the Plasma 6 plasmoid (QML)
gnome/                   the GNOME Shell extension (GJS)
bin/                     tailgauge-ctl / -send / -receive / -copy / -notify / -file-select / -update / -watch
systemd/                 the Taildrop receive user unit
test/                    model and parity tests, run by `node --test`
scripts/build.sh         assembles build/, wiring the shared model into both
scripts/install.sh       builds, then installs helpers, unit and packages
```

`shared/model.js` is written without module syntax so the QML engine can load it as a plain shared script. `scripts/build.sh` copies it into the plasmoid as-is and concatenates it with `model.exports.mjs` to produce the ES module the GNOME extension imports. **Edit `shared/model.js`, never the copies under `build/`.**

## Settings

| Setting | Default | What it does |
|---|---|---|
| Refresh interval | 30s | How often `tailscale status --json` is re-read |
| Show the machine name | off | Puts the machine name next to the panel icon |
| Recent Mullvad regions | - | Shortlist behind the exit node list, cleared from GNOME's preferences |
| Toggle shortcut | none | GNOME only: a global key that turns Tailscale on and off. Plasma binds `tailgauge-ctl toggle` in System Settings instead |

Plasma stores these in the widget's own configuration; GNOME in `org.gnome.shell.extensions.tailgauge`.

## Differences from the Omarchy plugin

- **Right click** opens the desktop's own context menu rather than toggling Tailscale. Both desktops carry the toggle and refresh as menu entries, and **middle click** on the panel icon toggles.
- **GNOME** uses native `PopupMenu` rows rather than a custom keyboard-driven panel, so arrows, Enter and type-ahead behave the way every other extension does. Machines and the Mullvad picker are submenus; the copy actions live inside a machine's submenu.
- **Clipboard** goes through `St.Clipboard` on GNOME and a helper that picks `wl-copy` / `xclip` / `xsel` on Plasma, so the copy actions also work in an X11 session.
- **No IPC**. `tailgauge-ctl toggle` stands in for `omarchy-shell omarchy.tailscale toggle`, but it drives tailscaled rather than the panel: nothing talks to a running widget.
- **A machine's copy actions** are a popup menu on Plasma and a submenu on GNOME. Both list the same options in the same order, because the model resolves them once.

## Development

```bash
scripts/build.sh                    # assemble build/ without installing
node --test 'test/**/*.test.mjs'    # model + parity tests (needs build.sh first)
scripts/install.sh                  # build and install for the running desktop
```

CI runs those on every pull request, along with `qmllint` for QML syntax, `node --check` on the extension, `shellcheck` on the helpers, and a check that the plasmoid and the extension declare the same version. Tagging `vX.Y.Z` builds and publishes the plasmoid tarball, the GNOME extension zip and the helpers.

Plasma logs QML errors under the `plasmashell` identifier rather than a unit, because it usually runs as a transient `app-plasmashell@<hash>.service`:

```bash
journalctl --user --identifier=plasmashell -f
```

`console.log` from a plasmoid maps to `qDebug` and is filtered out by default. Use `console.warn` when adding a temporary probe, or nothing reaches the journal.

GNOME logs extension errors to:

```bash
journalctl --user -u org.gnome.Shell@wayland -f
```

Linting QML needs `qmllint` from the Qt 6 declarative dev package (`qt6-qtdeclarative-devel` on Fedora, `qt6-declarative` on Arch).

## License

MIT. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
