# Changelog

All notable changes to this project are documented here.

## [1.0.0]

First release. A port of the first-party `omarchy.tailscale` panel plugin that
ships with [Omarchy 4 (Quattro)](https://github.com/basecamp/omarchy), moved off
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

### Fixed

- `claim_path` in the Taildrop receiver derived the incoming file's name from
  the caller's variable rather than its own argument, because both assignments
  shared one `local` statement. Upstream has the same shape; it is masked there
  by the caller passing an identically named variable.
