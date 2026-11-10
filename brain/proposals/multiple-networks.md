# Proposal: several tailnets and several NetBird networks at once

> **Parked (2026-11-10). Nothing here exists in the code.** This is a spec,
> not a description: no `Instance`, `tailgauge net` or `tailgauge mux` exists,
> and the rest of the brain describes TailGauge as it is. Parts marked
> **[spike]** are settled by Phase 0 (section 4) before any code lands. When
> work resumes, start with the open questions in section 11.

Decisions taken with the maintainer (2026-11-09):

1. Address ranges may overlap. No design may require disjoint prefixes.
2. Everything TailGauge adds is per user.
3. TailGauge creates and removes the extra networks itself, as painlessly as it
   can.
4. Naming: see section 5.3.

## 1. Goal

One machine is connected to several overlay networks **at the same time**,
without switching: several Tailscale tailnets (tailscale.com and/or Headscale
servers), several NetBird networks (netbird.io and/or self-hosted), any mix.
TailGauge shows each one, drives each one, and adds or removes them in a couple
of clicks.

Non-goals:

- Our own Tailscale or NetBird protocol client. We run the vendors' daemons.
- Routing between networks (a peer of A reaching a peer of B).
- Windows / macOS.

### Clean room

Built from the vendors' public CLI help and docs, RFC 1928 (SOCKS5) and our own
spikes. Nobody working on it reads the source of maisem/tailmix or
GrowlyX/tailmux (both BSD-3-Clause); their READMEs are the only contact.
Recorded as a decision in `brain/decisions.md` so the provenance holds.

## 2. Terms

| Term | Meaning | Avoid |
| --- | --- | --- |
| **kind** | A `PROVIDERS` registry row: CLI, parser, capabilities. Today's "provider". | - |
| **network** (UI) / **instance** (code) | One running daemon of a kind on its own socket. What a panel row, a switch and a `ctl` call act on. | "account", "profile", "tailnet" for this |
| **system network** | The daemon the vendor's package installed, on its default socket, in kernel mode. What TailGauge drives today. | "main", "primary" in UI text |
| **added network** | A daemon TailGauge created for this user, in userspace mode. | "secondary", "extra" in UI text |
| **profile** | A login inside one daemon (`tailscale switch`, `netbird profile`). Unchanged. | "network" |
| **mux** | `tailgauge mux`, the per-user process that makes added networks reachable by every app (section 6). | "proxy", "daemon" |

Ids: `tailscale` and `netbird` stay the system networks (every existing id,
`--ui` blob and key binding keeps working). Added networks are
`tailscale@<slug>`, `netbird@<slug>`, slug `[a-z0-9-]{1,32}`.

## 3. Why userspace, and what it costs

Overlap is allowed (decision 1), and every tailnet allocates from
`100.64.0.0/10` on its own. Two kernel-mode daemons with overlapping ranges
cannot both own a route to `100.x.y.z`, and both vendors hardcode their kernel
plumbing anyway (Tailscale: table 52, rules 5210-5270, marks `0x80000` /
`0x40000`; NetBird: table 7120, mark `0x1bd00`). So:

- **The system network stays exactly as today**, kernel mode, untouched.
- **Every added network runs in userspace mode**: Tailscale
  `--tun=userspace-networking --socks5-server=...`, NetBird
  `NB_USE_NETSTACK_MODE=true` with `NB_SOCKS5_LISTENER_*`. No tun, no routes,
  no rules, no firewall, no DNS changes: added networks cannot collide with each
  other or with the system one, whatever their addresses.
- Userspace needs no root, which is what makes decision 2 and 3 cheap: each
  added network is a **systemd user service** with its state under the user's
  home. **[spike S1]**: both daemons run unprivileged this way, with state and
  socket under `$XDG_STATE_HOME` / `$XDG_RUNTIME_DIR`.
- The mux (section 6) turns "reachable through a SOCKS5 port" into "reachable
  by every app by name". It needs a tun once, set up by one privileged step.

Costs, stated in the UI and README:

- Throughput is lower than kernel mode (userspace TCP/IP twice).
- ICMP to added-network peers is not forwarded (`ping` answers from the mux
  only, see 6).
- UDP depends on the daemon's SOCKS5 UDP ASSOCIATE. **[spike S2]**: Tailscale
  has an upstream fix for UDP relay in userspace mode (which release?); NetBird
  unknown.
- **Inbound**: a userspace daemon forwards connections to its own address on to
  the machine's localhost, which can expose services bound to `127.0.0.1` to
  that network. **[spike S3]** confirms per kind. Added networks therefore start
  with inbound blocked (Tailscale `--shields-up`, NetBird `--block-inbound`), a
  per-network switch in the panel turns it on.

## 4. Phase 0: spikes (no TailGauge code)

Each ends in a note here and, where it shows a CLI shape, a fixture under
`crates/tailgauge-core/tests/fixtures/`.

- **S1 unprivileged daemons.** `tailscaled` as the user, userspace, socket in
  `$XDG_RUNTIME_DIR`, state in `$XDG_STATE_HOME`, against a Headscale. Same for
  `netbird service run` in netstack mode with `--daemon-addr unix://...` and
  `--config` under the user's home (does it insist on `/var/lib/netbird`,
  `/var/log/netbird`, root?). `status --json` of each through its socket.
- **S2 SOCKS5.** CONNECT and UDP ASSOCIATE by IP to a peer, both kinds, with
  the versions installed here; DNS behaviour of each proxy.
- **S3 inbound.** What reaches the machine on each kind's userspace address, with
  and without shields-up / block-inbound.
- **S4 coexistence.** System Tailscale + system NetBird + one added network of
  each, two of them on overlapping ranges, all connected: each reaches its own
  peers.
- **S5 user-owned tun.** `ip tuntap add tgmux-<user> mode tun user <user>`
  plus address and route, then a process of that user opens it without
  capabilities; resolved per-link DNS on it; what survives a reboot
  (systemd-networkd `.netdev` vs a oneshot unit).

Exit: go/no-go per kind for added networks, and the UDP/inbound caveats as
facts rather than guesses.

## 5. TailGauge

### 5.1 Model: kind + instance

- `ProviderDescriptor` stays the static kind row. New `Instance { id, kind,
  slug, endpoint, origin: System | Added, label }`; `endpoint` is `None` for
  system networks, which keeps today's argv byte for byte.
- Argv methods move to the instance and prepend the endpoint
  (`tailscale --socket=<p> ...`, `netbird --daemon-addr=unix://<p> ...`).
  Capability gating stays on the kind, so
  `a_command_belongs_to_the_provider_that_has_it` holds per instance.
- `PanelState.providers` becomes instances; `active_provider_id` holds an
  instance id. `ctl --provider <instance-id>` keeps its shape; the endpoint never
  crosses to a frontend, like argvs today.

### 5.2 Where added networks live (per user)

- `~/.config/tailgauge/networks/<kind>@<slug>.toml`: kind, slug, server URL,
  label override, inbound flag. The only source of added networks: no socket
  globbing.
- State: `$XDG_STATE_HOME/tailgauge/<kind>@<slug>/`. Socket:
  `$XDG_RUNTIME_DIR/tailgauge/<kind>@<slug>.sock`.
- Service: `tailgauge-net@<kind>@<slug>.service`, a **user** unit template
  shipped by every package and `install.sh`
  (`/usr/lib/systemd/user/`, `~/.local/share/systemd/user/`), whose
  `ExecStart` is `tailgauge net run <kind>@<slug>`: one TailGauge entry point
  that reads the file and execs the right daemon with the right flags, so the
  unit never spells a vendor flag and parity tests can hold that.
- A file whose service is not running shows as "Stopped" with a Start action;
  it never silently disappears.

### 5.3 Naming (proposal)

- **On creation** the user types a short name ("work", "perso"); its slug is the
  id and the DNS label (6). That name is the row title.
- **Subtitle** is what the daemon says it is connected to: Tailscale
  `CurrentTailnet.Name` (a Headscale reports its server, e.g.
  `headscale.example.org`; tailscale.com reports the tailnet), NetBird the
  management URL host. So "work" / "Tailscale - headscale.example.org".
- **System networks** keep their kind name ("Tailscale", "NetBird") while no
  network was added, exactly as today. Once one exists, they read their daemon
  name the same way and can be renamed (label override stored in
  `~/.config/tailgauge/networks/system-<kind>.toml`).
- Tooltip: one line per network, "work (Tailscale): Connected".

### 5.4 Panel

- **VPN switcher** lists every network: system ones first, then added by name,
  each with its kind's glyph. Middle/right click on the bar cycles all of them,
  as it cycles providers today.
- **Bar icon and tooltip** describe every network.
- Every other section acts on the selected network as today (machines, exit
  nodes, profiles, NetBird networks).
- **Added-network header extras**: an inbound switch, a "via mux" / "SOCKS5
  only (mux stopped)" state line, and a Remove action with confirmation.
- **Add network...**: the last row of the switcher opens a small form: kind,
  name, server URL (empty = vendor's cloud). Submit runs `tailgauge net add`,
  then the login flow `ctl up` already has, against the new socket. This is the
  one new text-input form on all four surfaces (GNOME `St.Entry` rows, QML
  `TextField`, Omarchy, TUI prompt), and the parity tests learn it.

### 5.5 `tailgauge net` subcommands

All unprivileged except `mux setup`:

| Command | Does |
| --- | --- |
| `net add <kind> <name> [--server URL]` | writes the file, enables and starts the user unit, waits for the socket, sets shields-up / block-inbound |
| `net remove <id>` | logs out, stops and disables the unit, deletes file, state, socket |
| `net run <id>` | the unit's entry point, execs the vendor daemon |
| `net list --json` | what `gather` reads (cheap, files only) |
| `mux setup` | one pkexec: the user-owned tun, its address, route, resolved domain, persistent across reboots (S5) |
| `mux run` / `mux status --json` | section 6 |

Each new subcommand gets its alias everywhere CLAUDE.md lists (`ALIASES`,
`install.sh`, `package.sh`, `check-package.sh`, PKGBUILDs, the compat
bootstrap), or is declared alias-less like `--update`. Proposal: alias-less,
these are not key-binding targets.

The first `net add` on a machine without the tun runs `mux setup` first: one
password prompt, once per user and machine.

### 5.6 Freshness

`watch` walks instances: one `tailscale --socket=<p> debug watch-ipn` per
Tailscale network on the bus, everything else on the 2 s poll, one channel and
one deadline, exit codes unchanged.

## 6. The mux (`tailgauge mux run`, per user)

A user service, `tailgauge-mux.service`, started by the first `net add`,
stopped when the last added network goes. Not needed to draw the panel: the "no
daemon" rule is about drawing and stays true; without the mux, added networks
are still usable through their SOCKS5 ports.

- **Tun**: `tgmux-<user>`, user-owned (S5), so the mux needs no capabilities.
- **Pool**: a `/20` per user from `198.18.0.0/15` (RFC 2544, outside every
  tailnet's CGNAT range), chosen by `mux setup` and written to the tun's config.
- **Address map**: every peer of every added network gets a stable pool address
  keyed by `(network, peer IP)`, persisted under `$XDG_STATE_HOME`. Real
  addresses never need to be unique across networks, which is decision 1.
- **Names**: `<host>.<slug>.tg.internal` (`.internal` is reserved for private
  use). `mux setup` registers the tun with systemd-resolved as the DNS server
  for the routing-only domain `~tg.internal`, once; adding a network changes
  nothing privileged. The resolver answers on the pool's first address, inside
  the mux's own stack, so it binds no privileged port.
- **Data path**: a userspace TCP/IP stack (smoltcp) on the tun terminates each
  flow, maps the destination back to `(network, real IP)`, and opens SOCKS5
  CONNECT **by IP** (NetBird's proxy resolves hostnames through the host,
  netbirdio/netbird#7572) to that network's daemon, then splices. UDP through
  UDP ASSOCIATE where S2 says it works, else dropped and counted. ICMP echo to
  a pool address is answered by the mux when the mapping is live.
- **Peers' own addresses** are still shown in the panel; copy actions offer both
  the real `100.x` and the `.tg.internal` name for added networks.
- **Status**: `mux status --json` (networks mapped, flows, drops, errors), read
  by `gather`.
- Own crate, `crates/tailgauge-mux`, so core and the panel path build without
  smoltcp / tun; still one static musl binary with `mux` as a subcommand.

## 7. Failure modes the panel shows

| Situation | Panel |
| --- | --- |
| Added network, service stopped | row listed, "Stopped", Start |
| Needs login | today's needs-login path, per network |
| Mux stopped | "SOCKS5 only", Start mux |
| Tun missing (fresh boot went wrong, other user) | "Setup needed", runs `mux setup` |
| Inbound blocked | the switch shows it; no warning, it is the default |
| Operator rights missing on the system network | today's offer, unchanged |

## 8. Tests

- `specification.rs`: panels from fixtures with two Tailscale and two NetBird
  networks, overlapping ranges, labels from `CurrentTailnet.Name`, cursor order,
  the add form rows.
- `e2e.rs`: fake `tailscale` / `netbird` learn `--socket` / `--daemon-addr` and
  answer from a per-socket fixture, on the isolated `PATH`; `net add/remove`
  against a fake `systemctl --user`; `watch` wakes on an added network.
- `parity.rs`: every frontend draws the add form and the added-network header
  extras; no frontend spells an endpoint, a unit name or a vendor flag; unit
  templates installed by every packager.
- mux: unit tests on the address map, the resolver and SOCKS5 framing; one
  netns integration test behind a feature flag (root, CI job of its own).

## 9. Independent and smaller

- **NetBird profiles** as NetBird's `accounts` capability
  (`netbird profile list/select`).
- **Tailscale profiles: add account / rename** (`login --login-server
  --nickname`, `set --nickname`) for people happy to switch.

Either can ship before this.

## 10. Delivery

Branches under 149 files each, layered (mechanical, behaviour, tests, docs and
brain).

1. **Phase 0**: spikes S1-S5. No branch.
2. **Instance model** (mechanical): kind/instance split, system networks only,
   zero behaviour change.
3. **Added networks, no mux**: `net add/remove/run/list`, user unit template,
   discovery, endpoint argvs, `watch` per instance, naming, switcher, inbound
   switch, add form on four surfaces, packaging. Usable through SOCKS5.
4. **Mux**: crate, `mux setup/run/status`, tun, resolver, panel state lines,
   copy actions.
5. **Docs**: README section, brain feature doc `features/networks.md`,
   `architecture/mux.md`, decisions (userspace for added networks, per user,
   clean room, `.tg.internal`), glossary rows. Each branch carries its own brain
   layer; this is the README/CHANGELOG pass at release.

## 11. Still open

1. `.tg.internal` as the name suffix, or something you prefer?
2. Should a system network be adoptable into the list as "just another network"
   (renamable, removable from TailGauge), or always stay the vendor's, out of
   TailGauge's hands? Proposal: renamable, never removable.
3. Branch 3 without the mux is usable but awkward (SOCKS5 only). Ship 3 and 4
   in one release, or 3 alone as a preview?

## Sources

- Code: `crates/tailgauge-core/src/providers.rs` (`ProviderDescriptor`,
  `PROVIDERS`), `crates/tailgauge/src/gather.rs`, `watch.rs`, `ctl.rs`;
  `brain/architecture/providers.md`, `brain/architecture/overview.md`.
- Local CLIs: `tailscale` 1.104.2 (`--socket`, `up --shields-up`,
  `status --json` `CurrentTailnet.Name`), `netbird` 0.82.0 (`--daemon-addr`,
  `--config`, `service install --service`, `up --block-inbound`,
  `--interface-name`, `profile`); `ip rule` on this machine.
- Tailscale Linux router constants: tailscale/tailscale#591.
- Tailscale userspace networking: https://tailscale.com/docs/concepts/userspace-networking
- NetBird netstack env vars: https://docs.netbird.io/client/environment-variables;
  SOCKS5 hostname resolution: netbirdio/netbird discussion #7572.
- NetBird table 7120 / fwmark: netbirdio/netbird#1667.
- Prior art, README level only: maisem/tailmix, GrowlyX/tailmux, offbyone/tailhopper.
