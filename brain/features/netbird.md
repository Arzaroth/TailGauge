# NetBird and switching providers

NetBird is the second row of the provider registry: installed and parseable,
so TailGauge drives it with the same panel, minus what NetBird does not have.
The registry, detection and parsers are in
[../architecture/providers.md](../architecture/providers.md).

## What NetBird gets

- **Connection on/off** through `netbird up` / `netbird down`
  ([connection.md](connection.md)). Running is `daemonStatus` "Connected";
  `NeedsLogin`, `SessionExpired` and `LoginFailed` read as needs-login
  (`parse_netbird_status_grades_every_login_shaped_daemon_state`). NetBird's
  status has no auth URL; an unattended `ctl up` scrapes it off the `up` output.
- **This device and Machines** from `peers.details`: name is the first FQDN
  label (the IP when there is none), same ordering, subtitles, copy options
  and search as Tailscale ([machines.md](machines.md)). No OS, so every row
  wears the generic machine glyph.
- **Details**: connection type, latency (nanoseconds shown as ms, `-1` when
  unmeasured), ICE endpoint, relay, bytes, last handshake, routes, public key.
- **Networks**, a section only NetBird has (`networks` capability):
  `netbird networks list` parsed by `parse_netbird_networks`, one
  `network:<id>` row each, subtitle range or domains (`network_subtitle`),
  hint "Join" / "Leave", `busy` while `selectingNetworkId` matches. A click runs
  `tailgauge ctl --provider netbird select-network <id> [--leave]`.

## What it does not get

No exit nodes, no Mullvad, no accounts or operator prompt, no Taildrop
(`exit_nodes`, `mullvad`, `accounts`, `file_send` all false). The sections are
absent rather than empty, and `ctl` refuses before spawning
("NetBird has no exit nodes"; `a_provider_is_never_asked_for_what_it_does_not_have`).
`tailgauge watch 300` covers NetBird too: it polls `netbird status --json`
every 2 s and compares daemon state, running and needs-login
(`fingerprint_of_status`), so a NetBird change wakes the panel (exit 0) and a
quiet one lets the wait expire (exit 2), on a NetBird-only machine as well
(e2e `watch_wakes_on_a_netbird_change`,
`watch_on_a_quiet_netbird_expires_rather_than_giving_up`). See
[../architecture/overview.md](../architecture/overview.md).
`ProviderDescriptor::watch` still returns `None` for NetBird, but nothing reads
it outside its own test.

## Several providers at once

- `tailgauge panel` polls **every** installed provider's status in parallel;
  the bar icon and its tooltip describe all of them, the active one first
  (`the_bar_icon_describes_the_machine_not_the_panel_view`). The rest of the
  panel is the active provider.
- With both installed and no choice, Tailscale wins (registry order). The
  **VPN** section (`providers`) lists the drivable providers with
  `action: switchProvider` and shows only when there are two or more
  (`the_provider_switcher_appears_only_when_there_is_a_choice`).
- Choosing one stores it per desktop (GNOME `active-provider`, Plasma and
  Omarchy `activeProvider`), clears the panel (`Panel.emptyPanel()` in GNOME, `_empty` in the QML services) so one
  provider's machines never sit under the other's name, and refreshes. The id
  goes back as `activeProviderId` in `--ui` and as `ctl --provider`.

**Cycling from the bar.** Right click on Omarchy and middle click on Plasma
and GNOME call `cycleProvider` (`_cycleProvider` on GNOME), which takes the row
after the `current` one in the `providers` section and does nothing with fewer
than two. The bar icon describes the machine, so no click on it toggles a
connection (`no_click_on_the_bar_icon_toggles_a_connection`); middle click on
Omarchy refreshes.

## Sources

- `crates/tailgauge-core/src/providers.rs` (`PROVIDERS`, `drivable_providers`, `next_provider`), `crates/tailgauge-core/src/netbird.rs` (`parse_netbird_status`, `parse_netbird_networks`, `network_subtitle`)
- `crates/tailgauge-core/src/panel.rs` (`providers_section`, `networks_section`), `crates/tailgauge-core/src/bar.rs` (`bar_tooltip`, `bar_state`)
- `crates/tailgauge/src/gather.rs` (`statuses`), `crates/tailgauge/src/ctl.rs` (`select_network`), `crates/tailgauge/src/watch.rs` (`poll`, `fingerprint_of_status`), `crates/tailgauge/tests/e2e.rs` (`watch_wakes_on_a_netbird_change`, `watch_on_a_quiet_netbird_expires_rather_than_giving_up`)
- `omarchy/arzaroth.tailgauge/Panel.qml` (`cycleProvider`, `onPressed`), `plasma/org.tailgauge.plasmoid/contents/ui/main.qml` (`cycleProvider`), `plasma/org.tailgauge.plasmoid/contents/ui/CompactRep.qml`, `gnome/tailgauge@arzaroth.github.io/provider.ts` (`switchProvider`), `gnome/tailgauge@arzaroth.github.io/extension.ts` (`_cycleProvider`); `crates/tailgauge-core/tests/parity.rs` (`no_click_on_the_bar_icon_toggles_a_connection`)
- `crates/tailgauge-core/tests/fixtures/netbird-status.json`, `netbird-status-idle.json`, `netbird-networks.txt`; `crates/tailgauge-core/tests/specification.rs` (`parse_netbird_status_offers_nothing_it_cannot_do`, `the_networks_section_belongs_to_the_provider_that_has_networks`)
