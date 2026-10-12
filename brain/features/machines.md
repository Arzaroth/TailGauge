# Machines and this device

The tailnet (or NetBird mesh) as rows you can read, search, expand and copy
from. Two sections: **This device** (`self`) and **Machines** (`machines`).
Both are visible only while the active provider is ready and connected.

## This device

`self_section` draws `state.self_peer`, which the parsers build the same way
as every peer (`peer_from_status("self", ...)`, NetBird's synthetic self), so
it carries the same copy options a machine row does
(`this_device_carries_the_same_copy_options_a_machine_row_does`). Its
`action` is `copy`; the subtitle is the shared `peer_subtitle`. It vanishes
with the connection (`this_device_disappears_with_the_tailnet_it_belongs_to`).

## Machine rows

- **Order** is the parser's: online first, each half by `collate`
  (case-insensitive, locale-free). Both parsers do it the same way
  (`parse_netbird_status_orders_peers_online_first_like_the_tailscale_one`).
- **Label** `row_label` (display name, host name, "Unknown"); **icon** and
  **glyph** from the OS (`os_icon_name`, `os_icon`).
- **Subtitle** `peer_row_subtitle`: first IPv4, then the owner (or the DNS name
  when there is no owner), joined by ` · `. An offline machine is prefixed
  "Offline" and keeps its copy actions, because a sleeping machine's address is
  what you need to wake it
  (`offline_machines_are_listed_last_marked_and_cannot_be_sent_to`).
- **Actions**, in order: `detail` (Show/Hide details, only when
  `peer_detail_rows` has something), `send` (only when `can_send_files`, see
  [taildrop.md](taildrop.md)), `copy`.
- **Details.** `peer_detail_rows` turns what the provider actually reported
  (connection, latency, endpoint, relay, handshake or last seen, bytes,
  routes...) into non-navigable `detail:*` children; the row is `expanded` when
  its id equals the frontend's `expandedPeerId`.
- **Copy options** `peer_copy_options`, in this order and only when non-empty:
  `name`, `dns`, `ipv6`, `ip`, `key` (public key). Every frontend runs
  `tailgauge copy <label>`, which picks `wl-copy`, `xclip` or `xsel`
  ([../architecture/binary.md](../architecture/binary.md)); the parity tests
  forbid a frontend rebuilding this list.

## Search

- More than `MACHINE_SEARCH_MIN` (8) peers adds a `machines:search` row first
  (`kind: machineSearch`, "Search machines", not navigable). With any peers, a
  `machines:empty` row ("No machines match.", `searchScope: "machines"`, no key)
  is what a frontend shows when the query matches nothing.
- Each peer row carries `searchScope: "machines"` and a pre-lowercased
  `searchKey` from `machine_search_key`: display name, host name, DNS name, OS,
  owner, every IPv4 and IPv6. The frontend only lowercases the query and tests
  for a substring; it never builds its own haystack
  (`no_frontend_assembles_its_own_search_haystack` in `parity.rs`). The
  `navigation` entries repeat the scope and key, so the cursor can skip a row
  the query filtered out
  (`navigation_repeats_the_scope_and_key_of_the_row_it_points_at`).
- With no peers at all the section's own `empty` text applies: "No machines
  found on this tailnet."

## Sources

- `crates/tailgauge-core/src/panel.rs` (`self_section`, `machines_section`, `MACHINE_SEARCH_MIN`, `row_label`, `peer_subtitle`, `peer_row_subtitle`, `peer_copy_options`, `peer_detail_rows`, `os_icon`, `panel_navigation`)
- `crates/tailgauge-core/src/exit_nodes.rs` (`machine_search_key`), `crates/tailgauge-core/src/peer.rs` (`collate`), `crates/tailgauge-core/src/status.rs` (`parse_status`)
- `crates/tailgauge/src/copy.rs`, `gnome/tailgauge@arzaroth.github.io/provider.ts` (`copyToClipboard`)
- `crates/tailgauge-core/tests/specification.rs` (`the_machines_search_appears_only_for_a_list_long_enough_to_need_it`, `a_machine_key_matches_every_field_its_row_shows_and_the_os`, `machine_rows_carry_their_subtitle_icon_copy_options_and_actions`), `crates/tailgauge-core/tests/parity.rs`
