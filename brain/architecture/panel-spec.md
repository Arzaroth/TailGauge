# The panel spec

`panel::panel_spec(&PanelState, &ResolveOptions) -> Panel` is the one place
that decides what every surface shows. `tailgauge panel --json --ui '{...}'`
prints it; the three desktops draw it ([frontends.md](frontends.md)) and the TUI
calls it in-process. A frontend decides only what a row looks like. The flow
that feeds it is in [overview.md](overview.md).

## The JSON shape

```
{ bar, header, status, sections: [{id, title, visible, empty, rows}], footer, navigation }
```

Field names are camelCase on the wire (`#[serde(rename)]`), and
`gnome/tailgauge@arzaroth.github.io/panel.ts` declares the same shape by hand,
named exactly as the JSON names it: there is nothing to generate it from.

- **`bar`** (`bar::bar_state`): `connected` if any installed provider's summary
  is running, `warning` if any needs login, `crossed` when neither. With no
  summaries yet it falls back to the active provider's `active` and
  `needsLogin`. `tooltip` is one line per installed provider, active first,
  `Label  state · name · ip`, label column padded, then every line padded with
  no-break spaces to one width so a bar that centres each line still gets a
  left edge. Aggregate on purpose: switching which provider is shown must not
  change the icon.
- **`header`** (`panel_header`): `title` is the machine name when the provider
  is drivable and reported one, else the provider label. `meta` is
  `ACTIVE_PHRASES[phraseIndex rem 10]` while active, else `<Label> is
  disconnected`. `toggleVisible` and `toggleEnabled` are both
  `provider_ready`, **never gated on `busy`**; `toggleChecked` is `active`;
  `crossed` = off and not needing login, `warning` = needs login, `dimmed` = off.
  `toggleHint` (`toggle_hint`) is `Turn <Label> off|on` or `Authorize this
  device`, shared with Plasma's context menu. `actions` holds the `refresh`
  action when a provider is ready. `providerId`, `icon`, `glyph` come from the
  registry.
- **`status`** (`panel_status`), precedence in one place: no CLI on PATH
  (`No supported VPN CLI on PATH. Looked for Tailscale, NetBird.`, tone `dim`),
  installed but unsupported (`dim`), the frontend's `actionStatus` (`dim`), its
  `lastError` (`error`), else empty.
- **`footer`** (`panel_footer`): `TailGauge v<widget version>`, plus
  ` · binary v<x>` when the cached update status reports a different binary
  version (a half-applied update). Empty when the frontend sent no version.
- **`navigation`** (`panel_navigation`): `NavEntry {sectionId, rowId, action,
  searchScope, searchKey}` for the header, then every `navigable` row of every
  visible section in draw order, with an `expanded` row's navigable children
  right after it. Cursor movement is an index into this list, so no frontend
  carries its own focus state machine.

## Sections, in order

`panel_spec` always emits all seven, in this order; `visible` says whether to
draw one. `panel.ts` `SECTION_IDS` must match the list exactly.

| id | title | visible when | rows |
| --- | --- | --- | --- |
| `update` | (none) | `update.available` | one `update` row, action `update`, `busy` = `updating` |
| `providers` | VPN | more than one drivable provider | `provider:<id>`, action `switchProvider` |
| `self` | This device | provider ready, active, and the self peer has copy options | `self`, action `copy` |
| `connections` | Connections | accounts capability and (more than one account, or `accountsAccessDenied`) | `auth` (action `authorize`) first when access is denied, then `account:<id>` (action `switchAccount`) |
| `exitNodes` | Exit nodes | exit-node capability, active, rows not empty | the tailnet's own `exit:<id>` nodes, up to 5 recent or active Mullvad nodes (`recent_mullvad_nodes`), then the `mullvad:add` picker |
| `networks` | Networks | networks capability (NetBird), active, rows not empty | `network:<id>`, action `selectNetwork`, hint `Join` / `Leave` |
| `machines` | Machines | provider ready and active | `machines:search` above 8 peers (`MACHINE_SEARCH_MIN`), `machines:empty`, then `peer:<id>` |

The machines section's `empty` is `No machines found on this tailnet.`; the
others are empty strings. Capabilities come from the active provider's
registry entry, not its name ([providers.md](providers.md)).

## Rows

Every row serializes all 20 fields, so no frontend fills a gap (pinned by
`every_row_is_fully_formed_so_no_frontend_has_to_fill_a_gap`): `id`, `kind`,
`label`, `sublabel`, `icon` (a freedesktop symbolic name), `glyph` (a Nerd Font
codepoint, what Omarchy draws), `action`, `current`, `busy`, `bold`,
`navigable`, `hint`, `actions`, `copyOptions`, `children`, `expanded`,
`searchPlaceholder`, `searchScope`, `searchKey`, `payload`.

- **`action`** is the primary click: `copy`, `switchProvider`, `authorize`,
  `switchAccount`, `setExitNode`, `togglePicker`, `selectNetwork`, `update`
  (the header's is `toggle`). **`actions`** are the secondary buttons,
  `RowAction {id, label, icon, glyph}`: `detail` (Show / Hide details), `send`
  (only when `can_send_files`), `copy`; the header carries `refresh`. A
  frontend's single-letter keys follow the actions a row carries
  (`panel_row_has_action`), not its kind.
- **`copyOptions`** (`peer_copy_options`): `name`, `dns`, `ipv6`, `ip`, `key`, in
  that order, each only when non-empty.
- **`busy`** names the command a row waits on: `switchingAccountId`,
  `settingExitNodeId`, `selectingNetworkId` from the `--ui` state, matched
  against the row's id.
- **`children`**: a peer's detail rows (`peer_detail_rows`, `detail:<field>`,
  `navigable: false`, only fields the provider actually reported; `Last seen`
  only when there is no handshake; `Added` when fewer than three rows) and the
  picker's `region:<id>` rows. `expanded` is set for the peer whose id matches
  `expandedPeerId` and for the picker when `mullvadPickerOpen`.
- **Search.** A row with a `searchScope` (`machines` or `mullvad`) is drawn when
  the query is a substring of its pre-lowercased `searchKey`
  (`machine_search_key`: names, DNS, OS, owner, every IP;
  `mullvad_region_search_key`: city and country). The one keyless row per scope
  (`machines:empty`, `mullvad:empty`) is the message for no match. The search
  field and the empty rows are not cursor stops.
- **`payload`** is the source object serialized (peer, account, network,
  provider, update info). Frontends hand it back unchanged to `tailgauge ctl
  exit-node --peer` or `tailgauge send --peer`.

## The `--ui` state a frontend hands back

`gather::Ui`, every field optional (`#[serde(default)]`, so `{}` is valid and is
clap's default):

| Field | Feeds | Meaning |
| --- | --- | --- |
| `activeProviderId` | `PanelState` | Which provider is shown; empty, or naming one not installed, falls back to the first supported installed one in registry order (`providers::active_provider`) |
| `active` | `PanelState.active` | The optimistic toggle. Absent means "ask the daemon"; `false` means "the user just turned it off" |
| `busy`, `updating`, `actionStatus`, `lastError` | `PanelState` | Work the user asked for, and its words |
| `switchingAccountId`, `settingExitNodeId`, `selectingNetworkId` | `PanelState` | Which row is in flight |
| `version` | `PanelState.version` | The widget's own version, for the footer |
| `phraseIndex` | `ResolveOptions` | Which header phrase to show; frontends tick it every 2.8 s while the panel is open (Plasma and Omarchy only while connected) |
| `recentRegions` | `ResolveOptions` | Mullvad keys `Country\nCity`, most recent first |
| `mullvadPickerOpen`, `expandedPeerId` | `ResolveOptions` | What is unfolded |

`ResolveOptions.now_ms` is filled by the binary (`gather::resolve_options`).
Each frontend drops its override once `header.toggleChecked` agrees with it.

## Strings

English, written once in the core. There are no translations and no `.po`
files; frontends write only desktop conventions (`Refresh`, `Settings`) and a
short, listed set of progress and failure words, enforced by `parity.rs`.

`fmt.rs` holds the wording helpers the panel uses (`format_bytes`,
`format_since`, `is_profiles_access_denied`) plus `elide_status`, `first_url`,
`shell_quote` and `shell_command`, which only their own unit tests call now.

## The guards

`crates/tailgauge-core/tests/parity.rs` reads the frontend sources (Plasma
`FullRep`, `PanelRowView`, `CompactRep`, `main`, `ProviderService`; GNOME
`extension.ts`, `provider.ts`; Omarchy `Panel.qml`, `Service.qml`) and fails
when a decision leaks into a desktop:

| Test | Forbids |
| --- | --- |
| `no_user_visible_string_is_written_in_both_frontends` | an `i18n("...")` and `_('...')` string shared by Plasma and GNOME outside `DESKTOP_ONLY` / `PROGRESS_STRINGS` |
| `frontend_local_strings_are_conventions_and_progress_only` | any translated string outside those two lists |
| `the_omarchy_frontend_writes_no_user_visible_string` | literal `text:`, `title:`, `label:`... values in Omarchy outside the lists |
| `no_frontend_re_derives_which_sections_are_visible` | `showConnections`, `showExitNodes`, `showPeers` |
| `no_frontend_rebuilds_the_exit_node_or_copy_option_lists` | `displayExitNodes`, `peerCopyOptions` |
| `no_frontend_re_derives_the_status_precedence` | ranking `actionStatus` against `lastError` |
| `no_frontend_hands_a_panel_string_to_a_translator` | `Translate` |
| `every_frontend_is_flipped_and_none_has_a_model_left_to_call` | importing `model.js`, calling `resolvePanel` / `parseStatus` / `parseAccounts` / `parseExitNodeList`; requires the `tailgauge panel --json` argv |
| `no_qml_frontend_rebuilds_its_rows_just_to_redraw_them` | handing the sections, rows or children arrays to a Repeater (requires counted models such as `model: root.panel.sections.length`) |
| `every_frontend_keeps_its_search_field_across_a_redraw` | losing `syncRegistration()` in QML, or GNOME rebuilding on anything but its signature (`_applySearch`) |
| `no_frontend_assembles_its_own_search_haystack` | reading `.DisplayName`, `.HostName`, `.DNSName`, `.UserName`, `.City`, `.Country` where it filters; requires `searchKey` |
| `no_frontend_gates_a_control_on_background_work` | an `enabled:` / `enabled =` line mentioning `busy` |
| `every_frontend_tells_the_service_when_the_panel_is_on_screen` | a frontend with no `attentive` |
| `no_frontend_names_a_provider_binary_in_an_argv` | `"tailscale",` or `'netbird',` style argv literals |
| `every_frontend_draws_the_header_controls_it_is_handed` | a frontend that never reads `header.actions` |
| `the_gnome_extension_has_a_slot_for_every_section_the_panel_emits` | `SECTION_IDS` in `panel.ts` differing from what `panel_spec` emits, in order |

`crates/tailgauge-core/tests/specification.rs` says what the panel should do,
case by case (111 tests, ported from the former `model.test.ts` and kept in its
order). It resolves panels from captures of real daemon output in
`crates/tailgauge-core/tests/fixtures/` (`status.json`, `accounts.json`,
`exit-nodes.txt`, `netbird-status.json`, `netbird-status-idle.json`,
`netbird-networks.txt`) at a fixed `NOW` of 2026-09-14T06:00:00Z, grouped as:
the parsers; the sections (fixed order, visibility, exit-node ordering, picker);
the machines section and search (case-insensitive keys, empty-result rows,
search threshold); header, status and traversal (`navigation_visits_the_header_then_every_visible_row_in_draw_order`,
`every_cursor_stop_resolves_back_to_its_row`); banner, footer and busy rows
(`the_switch_stays_enabled_while_a_background_poll_runs`); the provider
registry; NetBird; the bar; and the expanded detail rows.

## Sources

- `crates/tailgauge-core/src/panel.rs` (`panel_spec`, `Panel`, `PanelRow`, `PanelHeader`, `PanelSection`, `NavEntry`, `ResolveOptions`, `panel_header`, `toggle_hint`, `panel_status`, `panel_footer`, `panel_navigation`, `peer_copy_options`, `peer_detail_rows`, `can_send_files`, `MACHINE_SEARCH_MIN`, `ACTIVE_PHRASES`, `panel_row_at`, `panel_has_row`, `panel_row_has_action`, `panel_nav_index_of`)
- `crates/tailgauge-core/src/panel_state.rs` (`PanelState`, `ProviderSummary`, `UpdateInfo`)
- `crates/tailgauge-core/src/bar.rs` (`bar_state`, `bar_tooltip`, `summarize_provider`)
- `crates/tailgauge-core/src/fmt.rs`, `crates/tailgauge-core/src/exit_nodes.rs` (`machine_search_key`, `mullvad_region_search_key`, `recent_mullvad_nodes`)
- `crates/tailgauge/src/gather.rs` (`Ui`, `resolve_options`), `crates/tailgauge/src/main.rs` (`Command::Panel`)
- `gnome/tailgauge@arzaroth.github.io/panel.ts` (`SECTION_IDS`, `Ui`, `emptyPanel`)
- `crates/tailgauge-core/tests/parity.rs`, `crates/tailgauge-core/tests/specification.rs`, `crates/tailgauge-core/tests/fixtures/`
