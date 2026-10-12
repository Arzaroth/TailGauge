# Features

What TailGauge does for a user, one doc per feature. Each says what it does on
every surface and links the architecture it rides on. Back to the root map:
[../BRAIN.md](../BRAIN.md).

| File | What's here | Grep keywords |
|---|---|---|
| [connection.md](connection.md) | The header switch, the optimistic flip, login URL scraping, `tailgauge ctl status/toggle/up/down` | toggle, active, Ui.active, up, down, login, authUrl, ctl |
| [accounts.md](accounts.md) | The Connections section: Tailscale profiles, the operator offer when profile access is refused | connections, account, switch, accountsAccessDenied, pkexec, operator |
| [exit-nodes.md](exit-nodes.md) | Tailnet exit nodes, up to five recent Mullvad regions, the Mullvad picker, setting one by IP or DNS name | exitNodes, mullvad, recent_mullvad_nodes, picker, exit_node_target |
| [machines.md](machines.md) | This device, peer rows, offline sorting, copy options, search past eight machines | self, machines, peer, copyOptions, searchKey, MACHINE_SEARCH_MIN, owner |
| [netbird.md](netbird.md) | NetBird as a second provider: what it gets, what it lacks, the VPN section and cycling providers | netbird, networks, providers, switchProvider, capabilities |
| [taildrop.md](taildrop.md) | Send from a machine row through the file chooser, the receive user service and its notification | send, receive, file-select, Downloads, tailgauge-receive.service |
| [keyboard.md](keyboard.md) | `t` `r` `c` `n` `d` `s`, cursor stops, arrows and Enter per desktop, the GNOME global shortcut | navigation, keys, shortcut, toggle-shortcut |
| [tui.md](tui.md) | `tailgauge tui`: tsui's layout, keys, in-process gathering, where it departs from the desktops | tui, ratatui, render.rs, TICK, SETTLE |
| [updates.md](updates.md) | `--check-update` and its cache, `--update`, the banner, version skew, `--install-frontend` | check-update, update, skew, banner, install-frontend |
