# Overview

The big picture: what lives where, how a reading of the VPN daemon becomes a
panel on four surfaces, and the rules the layout depends on. Read this first,
then jump to [panel-spec.md](panel-spec.md) for the JSON and
[frontends.md](frontends.md) for the desktops. Terms are in
[../glossary.md](../glossary.md).

## Workspace

| Path | What |
| --- | --- |
| `crates/tailgauge-core` | The library: provider registry, CLI parsers, and `panel_spec`. Depends on `chrono`, `serde`, `serde_json` only |
| `crates/tailgauge` | The one binary (`tailgauge`): clap subcommands, the gather, `ctl`, `watch`, Taildrop, clipboard, notifications, the TUI, the selvedge `Project` |
| `plasma/org.tailgauge.plasmoid` | KDE Plasma 6 applet, QML, installed as it sits |
| `gnome/tailgauge@arzaroth.github.io` | GNOME Shell extension, TypeScript compiled to GJS (GNOME 45 to 50) |
| `omarchy/arzaroth.tailgauge` | Omarchy bar widget, Quickshell QML plugin |
| `systemd/tailgauge-receive.service` | User unit running `%h/.local/bin/tailgauge receive` (Taildrop inbox) |
| `scripts/build.sh` | Compiles the extension and assembles the three payloads under `build/` |
| `scripts/install.sh` | Installs for the running desktop: a checked release when piped from curl, the checkout when run in a clone (`--plasma`, `--gnome`, `--omarchy`, `--no-frontend`, `--no-taildrop`, `--placement=`, `--version`) |
| `scripts/compat/tailgauge-update` | Bootstrap shipped in the `helpers` asset so a 0.4.0 install can update itself onto the binary |
| `tests/qml`, `tests/gnome`, `tests/gnome/shell` | Frontend harnesses (QML runtime with stubbed imports, Node with GJS mocked, a real headless GNOME Shell) |
| `.github/workflows/{ci,release}.yml` | The gate and the tag-driven release |

Inside the core crate (`crates/tailgauge-core/src`):

- `providers.rs` - `PROVIDERS` (Tailscale, then NetBird; order is preference),
  `Capabilities`, and every argv a provider is driven with
  (`ProviderDescriptor::status`, `up`, `down`, `set_exit_node`...). See
  [providers.md](providers.md).
- `status.rs`, `netbird.rs`, `accounts.rs`, `exit_nodes.rs`, `peer.rs` - parse
  what `tailscale status --json`, `netbird status --json`,
  `tailscale switch --list --json`, `tailscale exit-node list` and
  `netbird networks list` printed into `StatusResult`, `Peer`, `Account`,
  `Network`.
- `panel_state.rs`, `panel.rs`, `bar.rs`, `fmt.rs` - the input state and the
  resolved panel ([panel-spec.md](panel-spec.md)).

The core does no I/O and reads no clock: it builds argv vectors but never
spawns one, and "now" arrives as `ResolveOptions::now_ms`. That is what lets
`tests/specification.rs` resolve whole panels from recorded fixtures.

## Data flow

```
tailscale / netbird CLIs
   |  gather.rs: probe() PATH lookup, statuses() one thread per installed provider,
   |             extras() accounts / exit-node list / networks in parallel
   v
PanelState (panel_state.rs)  <-- gather::Ui, the --ui JSON the frontend sent
   |  panel::panel_spec(&state, &ResolveOptions)
   v
Panel {bar, header, status, sections, footer, navigation}
   |  `tailgauge panel --json --ui '{...}'` prints it on stdout
   v
Plasma ProviderService.qml | GNOME provider.ts | Omarchy Service.qml   (spawn per tick)
TUI (tui.rs)  calls gather::panel_state + panel_spec in-process on a worker thread
```

- `gather::panel_state` probes every provider in `PROVIDERS` with
  `selvedge::proc::has`, runs `status --json` for **every installed** provider at
  once (the bar describes the machine, not the view), then fills the active
  provider's peers, self row and exit nodes. Extras run only when the provider
  is running or has the accounts capability, and each only if the capability is
  there.
- The update banner reads the selvedge cache only (`gather::update_info`); a
  panel never waits on GitHub (e2e `the_panel_never_waits_on_github`). The
  frontends refresh that cache with `tailgauge --check-update` every six hours.
- Acting goes the other way through the binary too: `tailgauge ctl
  [--provider ID] <up|down|toggle|exit-node --peer JSON|switch-account|select-network|authorize>`,
  `tailgauge send --peer JSON`, `tailgauge copy TEXT`, `tailgauge --update`. A
  row's `payload` goes back as it came, so which address a peer is reached by
  (`panel::exit_node_target`, `peer_address`) stays the core's rule. See
  [binary.md](binary.md).

## No daemon, and how the panel stays fresh

TailGauge runs no service of its own (the Taildrop receiver unit is optional
and unrelated to drawing). Every frontend owns three clocks, the same on all
three desktops:

- **Watch.** `tailgauge watch 300` covers every installed supported provider,
  all under one deadline for the whole call. Tailscale rides the IPN bus when
  `tailscale debug watch-ipn` exists: it wakes on a line carrying `"State"`,
  `"Prefs"`, `"LoginFinished"` or `"BackendState"` (`watch::INTERESTING`). Every
  other installed provider, and Tailscale without the bus (or once the bus
  hangs up), is polled every 2 s on a fingerprint: for Tailscale `fingerprint_of`
  on the raw JSON (`BackendState` plus `ExitNodeStatus.ID`), for the rest
  `fingerprint_of_status` on the parsed status (daemon state, running,
  needs-login), so netmap churn never wakes it. It exits 0 on the first change,
  2 when the deadline passes, and 1 only when no supported VPN CLI is installed.
  Frontends refresh on 0, re-arm after 250 ms on 0 or 2, and back off 30 s
  otherwise. This is why a change made by `tailgauge ctl`, a VPN CLI or another
  desktop shows up at once, and why no IPC is needed on Plasma and GNOME.
- **Poll.** The floor under the watcher: `refreshIntervalSec` /
  `refresh-interval` (default 30, minimum 5), tightened to 3 s while the panel
  is on screen (the `attentive` flag, which parity requires every frontend to
  set).
- **Settle.** 600 ms after any command finishes, one more refresh (GNOME also
  clears the progress line after `ACTION_STATUS_MS`, 2.2 s).

The TUI has no watcher: it re-gathers every 3 s (`tui::TICK`) and 600 ms after a
command (`tui::SETTLE`).

All three frontends arm `tailgauge watch 300` whichever provider is shown. On a
NetBird-only machine the watch polls NetBird and exits 0 on a change or 2 on
expiry (e2e `watch_wakes_on_a_netbird_change`,
`watch_on_a_quiet_netbird_expires_rather_than_giving_up`).
`ProviderDescriptor::watch` still returns `tailgauge watch` for Tailscale and
`None` for NetBird; nothing reads it but its own test.

## What lives where

- **Core** decides everything a user reads: which sections exist, their order
  and visibility, every row and label, the status precedence, the header switch
  state, the bar icon and tooltip, the cursor traversal, the search keys.
- **Binary** reads the machine and acts on it: PATH probing, spawning the CLIs,
  the `--ui` hand-off, `ctl` (including scraping the login URL out of
  `tailscale up` and opening it), `watch`, Taildrop send and receive,
  clipboard (`copy.rs`: wl-copy first on Wayland, last elsewhere, then xclip
  and xsel, trying the next when one fails), notifications, file picking, self-update and frontend install
  through selvedge (`project.rs`, [distribution.md](distribution.md)).
- **Frontends** draw, filter search lists as you type (a keystroke cannot wait
  on a process), hold the state only they know (shown provider, optimistic
  toggle, in-flight ids, picker open, expanded row, phrase index, recent
  regions) and persist settings.

## Rules the layout depends on

- **One panel, resolved once.** A frontend never parses a CLI, never names a
  provider binary in an argv, never re-derives visibility, status precedence,
  exit-node or copy-option lists, or a search haystack. `parity.rs` greps the
  frontend sources for each of these ([panel-spec.md](panel-spec.md)).
- **Every frontend asks the binary.** `parity.rs`
  (`every_frontend_is_flipped_and_none_has_a_model_left_to_call`) requires the
  literal `'tailgauge', 'panel', '--json'` in each; e2e
  `every_command_the_frontends_invoke_exists` checks every subcommand a
  frontend spells exists.
- **The binary must be on PATH.** Plasma and GNOME prefix `PATH` with
  `$HOME/.local/bin:$HOME/bin:/usr/local/bin` (session PATHs often lack them);
  Omarchy spawns `tailgauge` from the session PATH as it is.
  A store install (KDE Store kpackage, EGO zip) cannot put it there; the core
  hides Taildrop's send action unless `PanelState::helpers` is set, which the
  binary always sets since it is the one answering.
- **Versions travel together.** The workspace version, the plasmoid
  `metadata.json`, the extension `version-name` and the Omarchy `manifest.json`
  agree (`project.rs` `every_payload_declares_the_same_version_as_the_binary`,
  and CI "Check the versions agree"); the footer shows `binary vX` when the
  widget and binary disagree after a half-applied update.
- **The TUI** reuses `gather` and `panel_spec` in-process; see
  [../features/tui.md](../features/tui.md).

## Sources

- `Cargo.toml` (workspace, version, MSRV 1.89), `crates/tailgauge-core/Cargo.toml`, `crates/tailgauge/Cargo.toml`
- `crates/tailgauge-core/src/lib.rs`, `crates/tailgauge-core/src/providers.rs` (`PROVIDERS`, `ProviderDescriptor::watch`)
- `crates/tailgauge/src/main.rs` (`Command::Panel`, `run_panel`, `splice_alias`)
- `crates/tailgauge/src/gather.rs` (`Ui`, `probe`, `statuses`, `extras`, `update_info`, `panel_state`, `resolve_options`)
- `crates/tailgauge/src/watch.rs` (`run`, `INTERESTING`, `watch_ipn`, `poll`, `fingerprint`, `fingerprint_of`, `fingerprint_of_status`)
- `crates/tailgauge/src/tui.rs` (`TICK`, `SETTLE`), `crates/tailgauge/src/copy.rs`, `crates/tailgauge/src/project.rs`
- `plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml`, `gnome/tailgauge@arzaroth.github.io/provider.ts`, `omarchy/arzaroth.tailgauge/Service.qml` (the three clocks)
- `crates/tailgauge-core/tests/parity.rs`, `crates/tailgauge/tests/e2e.rs` (`the_panel_never_waits_on_github`, `every_command_the_frontends_invoke_exists`, `watch_wakes_on_a_netbird_change`, `watch_on_a_quiet_netbird_expires_rather_than_giving_up`)
- `systemd/tailgauge-receive.service`, `scripts/build.sh`, `scripts/install.sh`, `scripts/compat/tailgauge-update`
