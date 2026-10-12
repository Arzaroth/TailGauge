# Providers, parsers and gathering

How TailGauge knows which VPN CLIs exist, what it asks each one, how it reads
the answers, and how a frontend learns that something changed. The registry
and the parsers live in `tailgauge-core` and shell out to nothing; the binary
(`crates/tailgauge`) does every spawn. The panel those answers become is in
[panel-spec.md](panel-spec.md); the subcommands that run all this are in
[binary.md](binary.md).

## The registry

`PROVIDERS` in `providers.rs` is one row per provider, a `ProviderDescriptor`:

| Field | Tailscale | NetBird |
| --- | --- | --- |
| `id`, `label` | `tailscale`, Tailscale | `netbird`, NetBird |
| `cli` (probed on PATH) | `tailscale` | `netbird` |
| `supported` (a parser exists) | true | true |
| `icon`, `glyph` | `network-vpn-symbolic`, U+F0EA0 | `network-workgroup-symbolic`, U+F15C6 |
| `capabilities` | exit nodes, Mullvad, file send, accounts, connection quality | networks, connection quality |

- **Row order is preference order.** With several installed and no choice made,
  the first `supported` one wins (`DEFAULT_PROVIDER_ID` is `tailscale`).
- **`Capabilities` / `Capability`** name each feature; `Capabilities::has` takes
  the enum so a typo is a build error. `provider_supports(state, capability)` is
  what `panel.rs` gates sections on, always against the *active* provider.
  `connection_quality` is declared but nothing reads it yet.
- **The argv is a method, not data.** `status()`, `up()`, `down()` exist for
  every provider; `exit_node_list()`, `accounts()`, `switch_account()`,
  `set_exit_node()`, `watch()` return `Some` only for Tailscale, `networks()`
  and `select_network()` only for NetBird. A provider without a feature has no
  argv for it, so nothing can fire one CLI's command at another's daemon
  (`a_command_belongs_to_the_provider_that_has_it`). The descriptor still
  serializes into row payloads, but the argv never crosses to a frontend.

| Ask | Tailscale argv | NetBird argv |
| --- | --- | --- |
| status | `tailscale status --json` | `netbird status --json` |
| on / off | `tailscale up` / `down` | `netbird up` / `down` |
| exit-node list | `tailscale exit-node list` | - |
| set exit node | `tailscale set --exit-node=<target>` (empty clears) | - |
| profiles | `tailscale switch --list --json`, `tailscale switch <id>` | - |
| networks | - | `netbird networks list`, `netbird networks select\|deselect <id>` |
| watch | `tailgauge watch <timeout>` | - |

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
  "No supported VPN CLI on PATH. Looked for Tailscale, NetBird."
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
  an answer, not a failure. `network_subtitle` prefers the range, then domains.
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
`netbird-status-idle.json` (a real idle capture), `netbird-networks.txt`.
`tests/specification.rs` parses them through `fixture()` into `LazyLock`
statics; `crates/tailgauge/tests/e2e.rs` writes fake `tailscale` / `netbird`
shell scripts that `cat` the same files and runs the shipped binary against
them ([testing.md](testing.md)). A new CLI shape starts as a new capture here.

## Gathering (`gather.rs`)

`tailgauge panel --json --ui '<json>'` calls `gather::panel_state(&Ui)`:

1. `probe()`: one `ProviderState` per registry row (`proc::has(cli)`).
2. `statuses()`: **every installed provider** is asked for status at once, one
   thread each, so the bar can say what all of them are doing. The parser is
   picked by `id == "netbird"`.
3. The active provider's status fills `running`, `needs_login`, self, peers,
   `own_exit_nodes`, `file_sharing`. `state.active = ui.active.unwrap_or(running)`
   is the optimistic toggle (`the_optimistic_toggle_is_absent_rather_than_false`).
4. `extras()`, only when running or the provider has accounts: exit-node list,
   accounts and networks in parallel threads, each skipped when the capability
   or argv is absent. `fmt::is_profiles_access_denied(stderr)` on the accounts
   call becomes `accounts_access_denied`.
5. `update_info()` reads the cached update check only; the panel never waits on
   GitHub (`the_panel_never_waits_on_github`).

`Ui` is everything only the frontend knows (`activeProviderId`, `active`,
`busy`, in-flight ids, `phraseIndex`, `recentRegions`, `mullvadPickerOpen`,
`expandedPeerId`); every field is optional (`the_ui_blob_is_optional_field_by_field`).

## Freshness: `tailgauge watch`

`watch.rs` blocks until tailscaled reports a change; exit 0 changed, 2 expired,
1 unusable (`Outcome::code`). It runs `tailscale debug watch-ipn` when
`supports_watch_ipn()` and wakes only on lines containing an `INTERESTING` key
(`"State"`, `"Prefs"`, `"LoginFinished"`, `"BackendState"`), never on netmap
churn. Without the bus it polls `tailscale status --json` every 2s, comparing
`fingerprint_of` (BackendState plus `ExitNodeStatus.ID`). One deadline covers
both paths. Frontends refresh on 0, re-arm after 250ms on 0 or 2, and back off
30s on anything else (`WATCH_REARM_MS`, `WATCH_BACKOFF_MS` in GNOME's
`provider.ts`); a poll timer stays underneath as the floor.

`tailscale.rs` is the binary's direct Tailscale reader for `watch` and
`ctl`: `installed`, `status`, `backend_state`, and `current_exit_node`, which
slices `exit-node list` with its own `first_active_host` (a second column
slicer beside core's).

## Adding a provider

1. Add a `PROVIDERS` row with its `cli`, `capabilities`, icon and glyph, and
   teach each argv method its spelling (`None` for what it lacks).
2. Write the parser (a `StatusResult` producer) and capture real output into
   `tests/fixtures/`; pin it in `specification.rs` and `e2e.rs`.
3. Dispatch it: `gather::statuses` and `ctl::read` both branch on
   `id == "netbird"`, so a third provider means turning that branch into a
   match (the registry comment says "nothing else names a provider"; these two
   do).
4. Ship its mark in each frontend and run the parity tests: no frontend may
   name a provider binary in an argv (`no_frontend_names_a_provider_binary_in_an_argv`).

## Sources

- `crates/tailgauge-core/src/providers.rs` (`PROVIDERS`, `ProviderDescriptor`, `Capabilities`, `Capability`, `installed_providers`, `active_provider`, `drivable_providers`, `next_provider`, `provider_ready`, `provider_supports`, `parse_provider_probe`, `login_plan`)
- `crates/tailgauge-core/src/status.rs` (`parse_status`, `peer_from_status`, `FILE_SHARING_CAP`), `peer.rs` (`Peer`, `collate`, `non_zero_time`, `is_mullvad_host`)
- `crates/tailgauge-core/src/netbird.rs` (`parse_netbird_status`, `parse_netbird_networks`, `network_subtitle`), `exit_nodes.rs` (`parse_exit_node_list`), `accounts.rs` (`parse_accounts`, `first_of`), `bar.rs` (`summarize_provider`), `panel_state.rs` (`PanelState`, `ProviderState`)
- `crates/tailgauge-core/tests/fixtures/`, `crates/tailgauge-core/tests/specification.rs`, `crates/tailgauge/tests/e2e.rs`
- `crates/tailgauge/src/gather.rs` (`Ui`, `probe`, `statuses`, `extras`, `panel_state`, `resolve_options`)
- `crates/tailgauge/src/watch.rs` (`Outcome`, `INTERESTING`, `watch_ipn`, `poll_fallback`, `fingerprint_of`), `crates/tailgauge/src/tailscale.rs` (`current_exit_node`, `first_active_host`)
- `gnome/tailgauge@arzaroth.github.io/provider.ts` (`_watch`, `WATCH_TIMEOUT_SEC`)
