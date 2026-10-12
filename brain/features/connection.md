# Connection on/off

The switch in the panel header turns the active provider on and off, and
`tailgauge ctl` does the same from a key binding or a script. Every path ends
in the provider's own `up` / `down` argv ([../architecture/providers.md](../architecture/providers.md)).

## The header switch

- `panel_header` sets `toggle_visible` and `toggle_enabled` from
  `provider_ready` only, **never from `busy`**: a background poll must not make
  the switch unclickable (`the_switch_stays_enabled_while_a_background_poll_runs`).
- `toggle_hint` names the provider: "Turn NetBird off", "Turn Tailscale on", or
  "Authorize this device" when `needs_login`. The `meta` line rotates through
  `ACTIVE_PHRASES` by `phraseIndex` while connected and says "X is
  disconnected" otherwise. `crossed`, `warning` (needs login) and `dimmed`
  drive the icon.
- **Optimistic flip.** A click patches the frontend's local copy of the header
  (`_showOptimistically` in GNOME, the same in Plasma's `ProviderService.qml`
  and Omarchy's `Service.qml`) and stores `_desired` (-1 none, 0 off, 1 on).
  Each `panel --ui` call sends `active` while `_desired` is set, and
  `gather::panel_state` lets it win over the daemon (`ui.active.unwrap_or(running)`).
  When an answer agrees with the click, `_desired` resets to -1. Absent and
  `false` differ (`the_optimistic_toggle_is_absent_rather_than_false`,
  e2e `a_click_shows_what_was_asked_until_the_daemon_agrees`).
- The click runs `tailgauge ctl --provider <active> up|down`, then a delayed
  refresh (`DELAYED_REFRESH_MS` 600 in GNOME). "Turning it on…" shows as the
  action status; a failure's stderr becomes `lastError`, which `panel_status`
  shows in the error tone after any in-flight progress.

## `ctl up`, `down`, `toggle`, `status`

- **Attended** (stdout is a terminal): `up` runs the provider's `up` with the
  terminal's stdio, so the CLI prints its own login URL.
- **Unattended** (key binding, frontend): `first_login_url` runs `up` to
  completion, reading stdout and stderr in two threads and keeping the first
  `https://` token (`login_url`); once `up` has exited it notifies "Authorize
  this device" and `xdg-open`s the URL. Then it re-reads status, and if the
  provider is not running posts a critical "Could not turn X on".
- `down` runs `<cli> down`; failure is a critical notification. `toggle` reads
  status and calls exactly one of them. `status` exits 0 connected, 3 not
  (`idle_line`: "Needs login", "Disconnected", or the daemon's own state word).
- Notifications are skipped when attended (`announce`): the terminal already
  has the output. Every failing `ctl` action follows the same rule.

## Desktop bindings

- Plasma: bind `tailgauge ctl toggle` as a custom command. GNOME: the
  `toggle-shortcut` setting (`Main.wm.addKeybinding`) toggles inside the
  extension, and a **middle click on the GNOME indicator** also toggles.
  Omarchy: `omarchy-shell arzaroth.tailgauge toggleTailscale` (IPC also has
  `up`, `down`, `status`, `diagnose`). On Plasma and Omarchy the bar icon never
  toggles ([netbird.md](netbird.md)).
- The panels watch `tailgauge watch`, so a change made by `ctl`, the CLI or
  another desktop shows up without IPC.

## Sources

- `crates/tailgauge-core/src/panel.rs` (`panel_header`, `toggle_hint`, `ACTIVE_PHRASES`, `panel_status`)
- `crates/tailgauge/src/gather.rs` (`Ui.active`, `panel_state`)
- `crates/tailgauge/src/ctl.rs` (`up`, `down`, `toggle`, `status`, `first_login_url`, `login_url`, `announce`, `attended`, `EXIT_DISCONNECTED`)
- `gnome/tailgauge@arzaroth.github.io/provider.ts` (`_showOptimistically`, `_desired`, `loginOrUp`, `down`, `_ctl`), `extension.ts` (`BUTTON_MIDDLE`, `toggle-shortcut`)
- `plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml`, `omarchy/arzaroth.tailgauge/Service.qml`, `omarchy/arzaroth.tailgauge/Panel.qml` (`IpcHandler`)
- `crates/tailgauge-core/tests/specification.rs` (`the_header_reflects_every_connection_state`, `the_toggle_hint_names_the_provider_it_will_act_on`), `crates/tailgauge/tests/e2e.rs`
