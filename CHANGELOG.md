# Changelog

All notable changes to this project are documented here.

## [0.0.1]

First release, and deliberately numbered as one: the Plasma widget has been
run and driven on a real desktop, but the GNOME extension has never been
executed. It is syntax-checked and shares the resolver, the model and the
helpers with the Plasma side, so the risk sits in its St/PopupMenu rendering
and its Gio.Subprocess plumbing.

A port of the first-party `omarchy.tailscale` panel plugin that ships with
[Omarchy 4 (Quattro)](https://github.com/basecamp/omarchy), moved off
Hyprland/Quickshell onto KDE Plasma 6 and GNOME Shell 45+.

### Added

- **KDE Plasma 6 plasmoid**: panel icon drawn natively as the Tailscale 3×3 dot
  grid, click-to-open popup with the on/off switch, connections, exit nodes and
  machines, and a keyboard cursor over the whole panel.
- **GNOME Shell extension** (45-50): the same panel as native `PopupMenu` rows,
  with machines and the Mullvad picker as submenus, plus an Adwaita preferences
  window.
- **Exit nodes**: every tailnet exit node, a shortlist of the Mullvad regions
  you actually use, and a searchable picker for the full Mullvad fleet.
- **Machines**: online peers with IP and MagicDNS name, copy actions for name /
  DNS name / IPv4 / IPv6, and a Taildrop send action where the tailnet allows
  file sharing.
- **Taildrop**: `tailgauge-send` picks files through the desktop's chooser;
  `tailgauge-receive` runs as a systemd user service, delivers into
  `~/Downloads` and announces each file with a click-to-open notification.
- **Connections**: switch between Tailscale profiles, and a prompt to
  `pkexec tailscale set --operator=$USER` when the daemon refuses profile
  access.
- **Login**: the authorization URL is scraped out of `tailscale up` as it prints
  and opened in the browser.
- **Keyboard**: `t` toggle, `r` refresh, and on a machine row `c` / `n` / `d`
  copy IP / name / DNS name and `s` sends files.

### Shared

- `shared/model.js` is the only copy of the Tailscale data model *and* of the
  panel layout. `resolvePanel()` decides which sections exist, in what order,
  which rows they hold, what every row says, and the cursor's traversal order;
  each frontend decides only how a row looks. `test/parity.test.mjs` enforces
  that boundary.

### Distribution

- Ships through store.kde.org and extensions.gnome.org, which both carry their
  own update mechanism, plus GitHub releases for anyone installing from the
  repo. `tailgauge-update --check/--apply` covers the last case, caches its
  GitHub answer for six hours, and refuses to touch a copy that KNewStuff, EGO
  or a distro package owns.
- The update banner is resolved in `resolvePanel()`, so both panels show it.

### Fixed

- The panel toggle was unclickable most of the time: it bound `enabled` to
  `busy`, which was true whenever any command was in flight, including the
  status poll running every few seconds. Upstream shows `busy` on its switch
  but never enforces it. `busy` now covers only work the user asked for, and
  the toggle is gated on nothing but the CLI being present.
- State changed outside the panel - tsui, the CLI, another desktop - took up to
  a full refresh interval to appear. `tailgauge-watch` blocks on
  `tailscale debug watch-ipn` and returns the moment tailscaled reports a
  change; both panels chain it ahead of a refresh. Polling stays as the floor
  and tightens to 3s while the panel is on screen.
- The installer printed a restart command for `plasma-plasmashell.service`,
  which does not exist when the session starts plasmashell as a transient
  `app-plasmashell@<hash>.service`. It now looks the unit up.
- Taildrop's send action appeared even when `tailgauge-send` was not installed,
  which is every store install: neither the KDE Store nor EGO can put anything
  on `PATH`. The panel now checks for the helper before offering the action.
- `claim_path` in the Taildrop receiver derived the incoming file's name from
  the caller's variable rather than its own argument, because both assignments
  shared one `local` statement. Upstream has the same shape; it is masked there
  by the caller passing an identically named variable.
