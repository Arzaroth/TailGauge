# Exit nodes and Mullvad

Routing this machine's traffic through another node: the tailnet's own exit
nodes, a short list of recently used Mullvad regions, and a searchable picker
over every Mullvad region. Tailscale only (`exit_nodes` and `mullvad`
capabilities); the section is hidden for NetBird and whenever the connection
is off (`exit_nodes_hide_when_tailscale_is_down`).

## Where the rows come from

- **Tailnet exit nodes** come from `tailscale status --json`: online,
  non-Mullvad peers with `ExitNodeOption`, sorted by `collate`
  (`parse_status_collects_exit_node_options`). An offline one is not offered.
- **Mullvad** comes from `tailscale exit-node list`, a fixed-width table that
  `parse_exit_node_list` slices at the header offsets (CITY can hold spaces).
  Only `.mullvad.ts.net` hosts survive; STATUS other than `-` marks the one in
  use. `mullvad_region_options` collapses servers into one entry per
  country and city (id `mullvad-region:<country>\n<city>`), dropping the `Any`
  city, sorted by country then city.
- `gather::extras` fetches the table only for a provider with exit nodes, and
  only when connected (or when the provider has accounts).

## The section (`exit_nodes_section`)

In order, each an `exitNode` row with `action: setExitNode`, `hint`
"Connect" or "Disconnect", `current` on the node in use, `busy` while
`settingExitNodeId` matches:

1. Every tailnet exit node.
2. The **recent shortlist**, `recent_mullvad_nodes(regions, recentRegions, 5)`:
   the region in use first, then remembered regions still on offer, at most 5.
   A region key is `mullvad_region_key` (`country\ncity`). A remembered key
   that matches no region's full key falls back to the first region in that
   country (`mullvad_region_node`), so a country-only key still yields a node;
   one matching neither is skipped.
3. The **picker**, a `mullvad:add` row ("Choose Mullvad region",
   `kind: mullvadPicker`, `action: togglePicker`, `expanded` from
   `mullvadPickerOpen`, `searchPlaceholder` "Search regions"). Its children are
   a `mullvad:empty` row ("No Mullvad regions found.") and one `mullvadRegion`
   row per region (title city, subtitle country), all with
   `searchScope: "mullvad"` and a lowercased `searchKey` "city country".
   Children are cursor stops only while the picker is expanded.

The frontend filters by substring on `searchKey` locally, because a keystroke
cannot wait on a process ([machines.md](machines.md)); GNOME always asks for
the picker open since its submenu hides itself.

## Setting one

The frontend hands the row's payload back: `tailgauge ctl --provider
tailscale exit-node --peer '<json>'`. `target_of` decides the target, so the
rule lives in one place:

- the node already in use -> empty target, which clears it;
- a Mullvad node -> its first `100.` IPv4 (`exit_node_target`), because the CLI
  will not resolve a Mullvad name;
- a tailnet node -> its DNS name (`peer_address`).

`ctl exit-node NAME` takes a name, and `off` / `none` / `clear` clear it;
with no argument it prints the current node, read from the table's STATUS
column by `tailscale::current_exit_node`.

**Remembering a region** is the frontend's job: on picking a region, GNOME
writes the gsettings key `recent-mullvad-regions` (`rememberMullvadRegion`,
`RECENT_MULLVAD_LIMIT` 5), Plasma `recentMullvadRegions` in its config
(`persistRecentMullvad`), Omarchy `recentMullvadRegions` in its plugin
settings. Each re-implements the front-insert-and-cap that core's
`push_recent_mullvad` specifies; the TUI keeps no list, so its shortlist is the
active region only.

## Sources

- `crates/tailgauge-core/src/exit_nodes.rs` (`parse_exit_node_list`, `mullvad_region_options`, `mullvad_region_key`, `recent_mullvad_nodes`, `mullvad_region_node`, `push_recent_mullvad`, `mullvad_region_search_key`)
- `crates/tailgauge-core/src/panel.rs` (`exit_nodes_section`, `exit_node_rows`, `exit_node_row`, `exit_node_target`, `peer_address`)
- `crates/tailgauge/src/ctl.rs` (`exit_node`, `target_of`, `requested_node`), `crates/tailgauge/src/tailscale.rs` (`current_exit_node`)
- `gnome/tailgauge@arzaroth.github.io/provider.ts` (`rememberMullvadRegion`, `setExitNode`), `plasma/org.tailgauge.plasmoid/contents/ui/main.qml` (`persistRecentMullvad`), `omarchy/arzaroth.tailgauge/Panel.qml`
- `crates/tailgauge-core/tests/fixtures/exit-nodes.txt`, `crates/tailgauge-core/tests/specification.rs` (`exit_nodes_list_the_tailnet_then_recents_then_the_picker`, `the_picker_keys_every_region_and_carries_the_message_for_an_empty_result`), `crates/tailgauge/tests/e2e.rs` (`a_row_hands_its_own_payload_back_to_set_an_exit_node`)
