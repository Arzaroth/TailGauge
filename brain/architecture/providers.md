# Providers, parsers and gathering

How TailGauge knows which VPN CLIs exist, what it asks each one, how it reads
the answers, and how a frontend learns that something changed. The registry
and the parsers live in `tailgauge-core` and shell out to nothing; the binary
(`crates/tailgauge`) does every spawn. The panel those answers become is in
[panel-spec.md](panel-spec.md); the subcommands that run all this are in
[binary.md](binary.md).

## The registry

`PROVIDERS` in `providers/mod.rs` is one row per provider, a
`ProviderDescriptor`: what the provider *is*. What it *does* is its `backend`,
a `&'static dyn Backend` with one implementation per provider in
`providers/tailscale.rs`, `providers/netbird.rs` and `providers/zerotier.rs`.

| Field | Tailscale | NetBird | ZeroTier |
| --- | --- | --- | --- |
| `id`, `label` | `tailscale`, Tailscale | `netbird`, NetBird | `zerotier`, ZeroTier |
| `cli` (probed on PATH) | `tailscale` | `netbird` | `zerotier-cli` |
| `supported` (a parser exists) | true | true | true |
| `icon`, `glyph` | `network-vpn-symbolic`, U+F0EA0 | `network-workgroup-symbolic`, U+F15C6 | `network-wired-symbolic`, U+F0C26 |
| `capabilities` | exit nodes, Mullvad, file send, accounts, connection quality, live watch | networks, connection quality | networks, connection quality |

- **Row order is preference order.** With several installed and no choice made,
  the first `supported` installed one in registry order wins, else the first
  installed (`active_provider`). `DEFAULT_PROVIDER_ID` (`tailscale`) matters
  only on the legacy `providers: None` path of `installed_providers`.
- **`Capabilities` / `Capability`** name each feature; `Capabilities::has` takes
  the enum so a typo is a build error. `live_watch` says the daemon pushes its
  changes (see the watch below); `connection_quality` is declared but nothing
  reads it yet.
- **`provider_supports(state, capability)`** is what `panel.rs` gates sections
  on, always against the *active* provider: the row's capability, narrowed by
  what the daemon reports. `FileSend` also needs `state.file_sharing` (the
  tailnet's file-sharing cap) and `Mullvad` a non-empty `mullvad_regions`, so a
  Headscale server without Mullvad or a tailnet with Taildrop off reaches the
  panel the way a provider without the feature does. A report never grants
  what the row lacks (`what_the_daemon_reports_narrows_what_the_provider_can_do`,
  `a_headscale_tailnet_offers_its_exit_nodes_and_taildrop_but_no_mullvad`).
  [Decision 0009](../decisions.md#0009-each-provider-is-a-backend-behind-one-trait-and-its-capabilities-narrow-at-runtime).
- **The argv lives on the backend.** `status()` and `parse_status(raw)` exist
  for every provider; `read_status(run)` gathers what the parser reads, one
  `status()` call unless the backend overrides it (ZeroTier makes three).
  `switch()` says what the on/off switch drives: `Switch::Connection { up,
  down }`, or `Switch::Networks` for a provider with no connection of its own,
  whose switch leaves and joins back its networks (ZeroTier). `network_id`
  normalizes a typed id to the provider's spelling (ZeroTier lowercases). Every
  other command is `None` by default and `Some` only where the provider has it, so
  nothing can fire one CLI's command at another's daemon
  (`a_command_belongs_to_the_provider_that_has_it`,
  `every_provider_the_registry_calls_drivable_has_what_driving_needs`). The
  descriptor still serializes into row payloads (`backend` is skipped), but the
  argv never crosses to a frontend.
- **`authorization()`** is how a user the CLI refuses is let in: an elevated
  shell line and the row that offers it (see Gathering, step 3).

| Ask | Tailscale argv | NetBird argv | ZeroTier argv |
| --- | --- | --- | --- |
| status | `tailscale status --json` | `netbird status --json` | `zerotier-cli -j info`, `-j listnetworks`, `-j peers` |
| on / off | `tailscale up` / `down` | `netbird up` / `down` | none: every network left and joined again |
| exit-node list | `tailscale exit-node list` | - | - |
| set exit node | `tailscale set --exit-node=<target>` (empty clears) | - | - |
| profiles | `tailscale switch --list --json`, `tailscale switch <id>` | - | - |
| networks | - | `netbird networks list`, `netbird networks select\|deselect <id>` | `zerotier-cli -j listnetworks`, `zerotier-cli join\|leave <id>` |
| events (`live_watch`) | `tailscale debug watch-ipn` | - | - |
| authorize | `pkexec tailscale set --operator="$(id -un)"` | - | `pkexec cat .../authtoken.secret` into a private temp file, moved onto `~/.zeroTierOneAuthToken` |

## Detection and the active provider

- `installed_providers(state)` reads `PanelState.providers` (one
  `ProviderState { id, installed }` per registry row) and returns them in
  **registry order**, however the caller enumerated them. With
  `providers: None`, a legacy caller's `installed: true` stands for Tailscale.
- `active_provider` takes `state.active_provider_id` when it names an installed
  provider, else the first `supported` installed one. A choice naming an
  uninstalled provider falls back rather than blanking the panel
  (`a_selection_naming_a_provider_that_is_gone_falls_back`).
- `drivable_providers` (installed and supported) feeds the VPN switcher;
  `next_provider` wraps through it and is `None` with fewer than two.
  `provider_ready` gates every poll and the header toggle.
- `provider_label` falls back to "TailGauge"; `provider_label_list()` builds
  "Tailscale, NetBird, ZeroTier", which `panel::panel_status` and
  `bar::bar_tooltip` put into "No supported VPN CLI on PATH. Looked for
  Tailscale, NetBird, ZeroTier."
- `parse_provider_probe` reads `which` stdout (absolute paths only, misses go
  to stderr). The binary no longer spawns `which`: `gather::probe` asks
  `selvedge::proc::has` per `cli`, so the parser survives only for its tests.
- `login_plan(needs_login, auth_url, up)` returns an auth URL to open or the up
  command; it is pinned by tests (`login_plan_turns_on_whatever_provider_it_was_handed`)
  but no caller uses it now that `ctl up` scrapes the URL itself
  ([../features/connection.md](../features/connection.md)).

## Parsers

Every parser takes raw CLI text and never fails: empty input is a state, bad
input is a typed error the panel draws.

- **`status.rs` `parse_status`** -> `StatusResult::{Ok, Unavailable, Error}`.
  Empty stdout is `Unavailable` ("Disconnected"), non-JSON is `Error`.
  `running` is `BackendState == "Running"`, `needs_login` is `"NeedsLogin"`, a
  blank state reads "Unknown". Mullvad peers (`.mullvad.ts.net`) are dropped
  from the peer list; online `ExitNodeOption` peers become `exit_nodes`. Peers
  sort online first, then by `peer::collate`. `file_sharing` reads
  `FILE_SHARING_CAP` from `Self.CapMap` or the older `Capabilities` list.
  `peer_from_status` builds the self row the same way as every peer, falling
  back to the top-level `TailscaleIPs` when `Self` has none; `CurAddr` means
  "P2P", a `Relay` alone means "Relayed".
- **`peer.rs`** is the shared `Peer` (serde names match what frontends read, so
  no translation layer) and the helpers: `field` / `text` (missing, null and ""
  are the same answer), `display_host_name` (a `localhost` falls back to the DNS
  label), `clean_dns_name`, `filter_ipv4` / `filter_ipv6` (only `100.` and `fd7a:115c:a1e0:` addresses),
  `non_zero_time` (Go's zero time is "never"), `strip_cidr`, and `collate`
  (case-insensitive, locale-free, lowercase wins a tie).
- **`netbird.rs` `parse_netbird_status`** maps `daemonStatus` (`Connected` is
  running; `NeedsLogin`, `SessionExpired`, `LoginFailed` are needs-login) and
  `peers.details[]`: host is the first FQDN label, `latency` is nanoseconds
  shown as ms (`-1.0` is unmeasured), no OS, no exit nodes, `auth_url` always
  empty (NetBird prints it on the `up` stream). A JSON array is an error, not a
  status (`a_list_is_not_a_status_document`).
  **`parse_netbird_networks`** reads the `- ID:` block format; "no networks" is
  an answer, not a failure.
- **`network.rs`** is the `Network` every provider with networks shares:
  `network_label` prefers its `name` (NetBird's are empty), `network_subtitle`
  the range, then domains, then the status; `auth_url` is set while the
  network waits on a login. `with_left_networks` appends the networks
  remembered but no longer joined as "Not joined" rows.
- **`zerotier.rs` `parse_zerotier_status`** reads the `envelope` of `info`,
  `listnetworks` and `peers` that `read_status` builds. Empty is `Unavailable`
  (the service is down), `ACCESS_DENIED` (`{"accessDenied":true}`, what a CLI
  failing with `is_access_denied` becomes) is `Unavailable` with
  `access_denied`. `running` is joined to any network; `daemon_state` is
  `Running` once one is `OK`, `NeedsLogin` for an SSO network (its
  `authenticationURL` is the `auth_url`), else `Waiting`, `Offline` or
  `Stopped`. Self is the node address with the up networks' addresses, split by
  family. Peers are the reachable `LEAF`s only (a live path or a measured
  latency), named by address; a preferred live path is "P2P", none is
  "Relayed". The joined networks ride on `StatusOk::networks` (empty for the
  other providers). **`parse_zerotier_networks`** names each network and puts
  its addresses in the range once it is `OK`, its state in words before
  (`Waiting for authorization`), and an SSO network's `authenticationURL` in
  `auth_url`. See
  [../features/zerotier.md](../features/zerotier.md).
- **`exit_nodes.rs` `parse_exit_node_list`** slices the fixed-width table at the
  header offsets (CITY holds spaces), keeps only Mullvad hosts, and marks
  `exit_node` from STATUS. `mullvad_region_options`, `recent_mullvad_nodes`,
  the search keys are covered in [../features/exit-nodes.md](../features/exit-nodes.md).
- **`accounts.rs` `parse_accounts`** reads `switch --list --json`, accepting
  each field under every spelling the CLI has used (`id`/`ID`,
  `nickname`/`Nickname`/`name`/`Name`, `loginName`...) via `first_of`.
- **`bar.rs` `summarize_provider`** turns each provider's `StatusResult` into a
  `ProviderSummary` for the bar icon and tooltip, which describe the machine,
  not the provider on screen.

## Fixtures

`crates/tailgauge-core/tests/fixtures/` holds captures of real CLI output:
`status.json`, `accounts.json`, `exit-nodes.txt`, `netbird-status.json`,
`netbird-status-idle.json` (a real idle capture), `netbird-networks.txt`,
`headscale-status.json` and `headscale-exit-nodes.txt` (a client on Headscale
0.29.7), and `zerotier-info.json`, `zerotier-listnetworks.json`,
`zerotier-peers.json` plus their `-denied` variants, recorded by
`tests/zerotier/record.sh` from two ZeroTier nodes in containers.
`tests/specification.rs` parses them through `fixture()` into `LazyLock`
statics; `crates/tailgauge/tests/e2e.rs` writes fake `tailscale` / `netbird` /
`zerotier-cli` shell scripts that `cat` the same files and runs the shipped binary against
them ([testing.md](testing.md)). A new CLI shape starts as a new capture here.

## Gathering (`gather.rs`)

`tailgauge panel --json --ui '<json>'` calls `gather::panel_state(&Ui)`:

1. `probe()`: one `ProviderState` per registry row (`proc::has(cli)`).
2. `statuses()`: **every installed provider** is asked for status at once, one
   thread each, so the bar can say what all of them are doing:
   `status_output` runs the backend's `read_status` through `run_bounded`,
   which kills a read still running after `READ_LIMIT` (5 s) and reads it as
   failed (`a_read_that_hangs_is_cut_off_and_reads_as_failed`), and
   `backend.parse_status` reads the answer.
3. The active provider's status fills `running`, `needs_login`, self, peers,
   `own_exit_nodes`, `file_sharing`, `networks`; an `Unavailable` answer with
   `access_denied` sets `state.access_denied`, and the connections section
   offers the backend's `authorization()` row
   (`a_cli_that_refuses_this_user_asks_for_it_whether_or_not_there_are_accounts`).
   `state.active = ui.active.unwrap_or(running)` is the optimistic toggle
   (`the_optimistic_toggle_is_absent_rather_than_false`).
4. `extras()`, only when running or the provider has accounts: exit-node list,
   accounts and networks in parallel threads, each skipped when the capability
   or argv is absent; networks go through `backend.parse_networks`.
   `fmt::is_profiles_access_denied(stderr)` on the accounts call becomes
   `accounts_access_denied`. A provider switched by its networks skips the
   networks call: its status named them. Whenever it answered, on or off,
   they are remembered (`networks::observe`) and the ones left are added back
   as rows (`with_left_networks`).
5. `update_info()` reads the cached update check only; the panel never waits on
   GitHub (`the_panel_never_waits_on_github`).

`Ui` is everything only the frontend knows (`activeProviderId`, `active`,
`busy`, in-flight ids, `phraseIndex`, `recentRegions`, `mullvadPickerOpen`,
`expandedPeerId`); every field is optional (`the_ui_blob_is_optional_field_by_field`).

## Freshness: `tailgauge watch`

`watch.rs` blocks until any installed supported provider reports a change;
exit 0 changed, 2 expired, 1 only when no supported VPN CLI is installed
(`Outcome::code`). Every source reports into one channel under one deadline:

- **A provider with `live_watch` on its event stream.** The first installed
  one (Tailscale) whose `backend.events()` argv answers `--help`, or whose
  parent command lists it (`has_events`), runs `tailscale debug watch-ipn` in
  `watch_events` and wakes only on lines containing one of its
  `event_markers()` (`"State"`, `"Prefs"`, `"LoginFinished"`,
  `"BackendState"`), never on netmap churn. A stream that hangs up is not a
  change: the provider moves to the poll for what is left of the deadline.
- **Everything else polled.** `poll` re-reads every other installed provider
  (and a live-watch one without its stream) every 2 s through `status_output`
  and `parse_status`, and compares `fingerprint_of_status`: daemon state,
  running, needs-login, `exit_node_id` (`ExitNodeStatus.ID`, so a Mullvad
  node counts though it is not a peer) and each network's id and state, so a
  ZeroTier network joined or left while another stays up counts. A NetBird- or
  ZeroTier-only machine waits and wakes like a Tailscale one
  (`a_netbird_that_connects_moves_its_fingerprint`, e2e
  `watch_wakes_on_a_netbird_change`,
  `watch_wakes_when_a_zerotier_network_is_left_while_another_stays_up`,
  `watch_on_a_quiet_netbird_expires_rather_than_giving_up`).

Frontends refresh on 0, re-arm after 250 ms on 0 or 2, and back off 30 s on
anything else (`WATCH_REARM_MS`, `WATCH_BACKOFF_MS` in GNOME's
`provider.ts`); a poll timer stays underneath as the floor.

`tailscale.rs` is the binary's direct Tailscale reader for `ctl` and the
Taildrop subcommands in `main.rs`: `installed` and `current_exit_node`, which
slices `exit-node list` with its own `first_active_host` (a second column
slicer beside core's).

## Adding a provider

1. Add a `PROVIDERS` row with its `cli`, `capabilities`, icon and glyph, and a
   `Backend` in `providers/<id>.rs`: `status()`, `parse_status`, `switch()`
   (`Switch::Connection { up, down }`, or `Switch::Networks` to be switched by
   its networks, which also needs `networks()`, `parse_networks` and
   `select_network`), and each optional command the provider has. The compiler asks for nothing else; what is left out is
   `None`.
2. Write the parser (a `StatusResult` producer) and capture real output into
   `tests/fixtures/`, scripted where the provider can run in a container
   (`tests/zerotier/record.sh`); pin it in `specification.rs` and `e2e.rs`.
   `gather::statuses`, `ctl::read` and `watch`'s poll go through the backend,
   so nothing else changes.
3. Ship its mark in each frontend and run the parity tests: every desktop
   names every provider's mark
   (`every_desktop_draws_the_bar_as_the_provider_the_core_names`), and no
   frontend names a provider binary in an argv
   (`no_frontend_names_a_provider_binary_in_an_argv`).

## Sources

- `crates/tailgauge-core/src/providers/mod.rs` (`PROVIDERS`, `ProviderDescriptor`, `Backend`, `Switch`, `Authorization`, `Capabilities`, `Capability`, `installed_providers`, `active_provider`, `drivable_providers`, `next_provider`, `provider_ready`, `provider_supports`, `reported`, `parse_provider_probe`, `provider_label_list`, `login_plan`), `providers/tailscale.rs`, `providers/netbird.rs`, `providers/zerotier.rs`
- `crates/tailgauge-core/src/status.rs` (`parse_status`, `peer_from_status`, `FILE_SHARING_CAP`, `StatusUnavailable::access_denied`), `peer.rs` (`Peer`, `collate`, `non_zero_time`, `is_mullvad_host`)
- `crates/tailgauge-core/src/netbird.rs` (`parse_netbird_status`, `parse_netbird_networks`), `network.rs` (`Network`, `network_label`, `network_subtitle`, `with_left_networks`), `zerotier.rs` (`envelope`, `is_access_denied`, `parse_zerotier_status`, `parse_zerotier_networks`), `exit_nodes.rs` (`parse_exit_node_list`), `accounts.rs` (`parse_accounts`, `first_of`), `bar.rs` (`summarize_provider`), `panel_state.rs` (`PanelState`, `ProviderState`)
- `crates/tailgauge-core/tests/fixtures/`, `crates/tailgauge-core/tests/specification.rs`, `crates/tailgauge/tests/e2e.rs`, `tests/zerotier/record.sh`
- `crates/tailgauge/src/gather.rs` (`Ui`, `READ_LIMIT`, `run_bounded`, `probe`, `status_output`, `statuses`, `extras`, `connect`, `panel_state`), `crates/tailgauge/src/networks.rs`
- `crates/tailgauge-core/src/panel.rs` (`panel_status`, `connections_section`), `bar.rs` (`bar_tooltip`)
- `crates/tailgauge/src/ctl.rs` (`read`, `authorize`), `crates/tailgauge/src/watch.rs` (`Outcome`, `run`, `has_events`, `watch_events`, `poll`, `fingerprint`, `fingerprint_of_status`, `a_netbird_that_connects_moves_its_fingerprint`), `crates/tailgauge/src/tailscale.rs` (`current_exit_node`, `first_active_host`)
- `gnome/tailgauge@arzaroth.github.io/provider.ts` (`_watch`, `WATCH_TIMEOUT_SEC`)
