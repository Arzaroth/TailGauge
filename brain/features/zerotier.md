# ZeroTier

ZeroTier is the third row of the provider registry, driven through
`zerotier-cli`. It has no connection of its own to bring up: the service
always runs and each network is joined or left on its own, so TailGauge's
switch is built from its networks. The registry, the backend trait and the
parser are in [../architecture/providers.md](../architecture/providers.md);
the why is [decision 0010](../decisions.md#0010-zerotiers-switch-leaves-and-rejoins-its-networks).

## What ZeroTier gets

- **Status** from three calls read as one answer (`read_status`):
  `zerotier-cli -j info`, `-j listnetworks` and `-j peers`, joined into an
  `envelope` for `parse_zerotier_status`.
- **The switch.** On is joined to at least one network, even one still
  waiting for its controller. Off (`ctl down`, the header switch) leaves every
  joined network; on (`ctl up`) joins back the ones it took
  (`ctl::rejoin`, `ctl::leave_all`,
  `the_zerotier_switch_leaves_every_network_and_joins_them_again`). Off saves
  what it leaves before leaving anything, and leaves nothing when it cannot.
  With nothing remembered, or every network left by hand, `up` fails and says
  which (`the_zerotier_switch_says_why_it_has_nothing_to_join`).
- **Remembered networks.** ZeroTier forgets a network the moment it is left,
  so the binary keeps every network joined in
  `$XDG_STATE_HOME/tailgauge/networks-zerotier.json`, 0600 in a 0700
  directory (`networks.rs`: `Known` is id, name and `wanted`). Every change
  goes through one locked read-modify-write (`update`). A panel poll only adds
  networks it has not seen (`observe`): its snapshot can be older than a
  leave, so it never sets `wanted`. Only the user does: a row's join or leave,
  or `ctl select-network <id> [--leave]`, sets it (`want`, remembering a
  network never seen, the id lowercased), and the switch marks what it leaves
  as wanted (`take`). A network left by hand stays listed as "Not joined"
  with a Join hint, on or off, and the switch leaves it out when it comes
  back on (`a_zerotier_network_left_by_hand_stays_listed_and_out_of_the_switch`,
  `the_last_zerotier_network_left_by_hand_keeps_its_row_to_join_it_back`,
  `a_poll_adds_what_it_sees_and_never_wants_what_was_left`).
- **Networks**, the same section NetBird uses, drawn from the networks the
  status names (no second call) and kept while the switch is off: labelled by
  the network's name (its id until the controller names it), subtitled by its
  addresses once
  `OK`, else its state in words ("Waiting for authorization", "Requesting
  configuration", "Needs login", ...;
  `a_zerotier_network_waiting_for_its_controller_says_so`). A click runs
  `tailgauge ctl --provider zerotier select-network <id> [--leave]`, which is
  `zerotier-cli join|leave <id>`.
- **SSO.** A network in `AUTHENTICATION_REQUIRED` makes the provider
  needs-login, and its row is the login: action `openUrl` with the
  network's `authenticationURL` as `payload.url`, hinted "Log in", which every
  desktop and the TUI open (`a_network_behind_sso_is_a_login_with_its_url`,
  `a_zerotier_network_behind_sso_is_the_row_that_opens_its_login`).
- **This device** is the node's 10-digit address, with the addresses of the
  networks that are up, split by family.
- **Machines** are the reachable `LEAF` peers, by address: peer names live on
  the controller, which TailGauge does not ask. A peer with neither a live path
  nor a measured latency is infrastructure ZeroTier has heard of and is left
  out. A preferred live path is direct, none is relayed through a root
  (`only_reachable_leaves_are_machines_and_a_preferred_path_is_direct`).
- **Access.** `zerotier-cli` needs the service's `authtoken.secret`, which is
  root's. A refusal (`authtoken.secret not found or readable`, or a `401`) is
  access denied, not a stopped service, and the connections section offers
  "Allow access to ZeroTier": `ctl authorize` copies the token to
  `~/.zeroTierOneAuthToken`, where the CLI looks next
  (`a_zerotier_that_refuses_this_user_offers_to_let_them_in`). Only the read
  is elevated (`pkexec cat`); the user's own shell writes a `mktemp` file and
  `mv -fT`s it into place, because a root write into the user's home follows
  any symlink planted there (`only_the_read_of_the_token_runs_as_root`).
  While refused, the header drops its switch and says "ZeroTier needs your
  permission", the bar wears the warning badge and the tooltip says "needs
  access", and `ctl` names the refusal and `authorize` rather than "not
  answering" (`a_zerotier_that_refuses_this_user_says_so_rather_than_reading_as_off`).
- **Found in `/usr/sbin`.** ZeroTier's packages install `zerotier-cli` there;
  GNOME and Plasma append `/usr/sbin:/sbin` to the PATH they run the binary
  with.
- **Its mark** on every desktop: the Simple Icons ZeroTier path (CC0-1.0) in
  `ZeroTierIcon.qml` on Plasma and Omarchy, and `drawZeroTier` in cairo on
  GNOME. The mark is a filled square, so its disconnected slash sits in a
  cleared band. The glyph is U+F0C26.

## What it does not get

No exit nodes, Mullvad, accounts or Taildrop. No `live_watch`: `watch` polls it
every 2 s like NetBird, and a join, a leave or a network coming up moves its
fingerprint, which carries each network's id and state
(`watch_wakes_when_a_zerotier_network_is_left_while_another_stays_up`). A
hung `zerotier-one` is cut off after 5 s like any status read. No peer names and no per-peer addresses (both live on the
controller); a machine row copies its ZeroTier address.

## Testing without an account

`tests/zerotier/record.sh` runs two `zerotier/zerotier` containers, creates
two networks on one node's built-in controller, and records the member
waiting for authorization and then joined to both with a direct path into
`crates/tailgauge-core/tests/fixtures/zerotier-*.json`, the public address
replaced by a documentation one before it reaches the fixtures. The e2e fake `zerotier-cli` joins and leaves
for real against those captures.

## Sources

- `crates/tailgauge-core/src/providers/zerotier.rs` (`ZeroTier`: `read_status`, `select_network`, `authorization`), `crates/tailgauge-core/src/zerotier.rs` (`envelope`, `is_access_denied`, `parse_zerotier_status`, `parse_zerotier_networks`, `with_left_networks`, `reachable`)
- `crates/tailgauge/src/networks.rs` (`Known`, `update`, `observe`, `take`, `want`, `to_rejoin`, `joined`), `crates/tailgauge/src/ctl.rs` (`up`, `down`, `rejoin`, `leave_all`, `select_network`, `authorize`), `crates/tailgauge/src/gather.rs` (`panel_state`)
- `plasma/org.tailgauge.plasmoid/contents/ui/ZeroTierIcon.qml`, `omarchy/arzaroth.tailgauge/ZeroTierIcon.qml`, `gnome/tailgauge@arzaroth.github.io/extension.ts` (`drawZeroTier`)
- `tests/zerotier/record.sh`, `crates/tailgauge-core/tests/fixtures/zerotier-*.json`, `crates/tailgauge-core/tests/specification.rs` (`a_zerotier_node_shows_its_named_networks_and_the_machines_it_reaches`), `crates/tailgauge/tests/e2e.rs` (`a_zerotier_panel_lists_its_networks_by_name_and_the_machines_it_reaches`)
