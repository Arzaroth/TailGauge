# Glossary

The words this codebase uses, what each means here, and where it lives. Use
these terms in code, docs and commits; the **Avoid** list names the synonyms
that blur it. Back to the root map: [BRAIN.md](BRAIN.md).

## Providers and the daemon

- **Provider** - a VPN whose CLI TailGauge drives: one row of `PROVIDERS` in
  `providers/mod.rs` (Tailscale, then NetBird, then ZeroTier). See
  [architecture/providers.md](architecture/providers.md).
  Avoid: VPN type, service.
- **Backend** - what a provider does: its `Backend` implementation in
  `providers/<id>.rs`, holding the argv for each thing the panel asks and the
  parser for the answer, `None` for what the provider lacks. The row says what
  the provider is. Avoid: driver, adapter, plugin.
- **Supported / drivable** - supported: TailGauge has a parser for it.
  Drivable: supported and installed on this machine (found on `PATH`).
  Avoid: available, enabled.
- **Active provider** - the provider the panel is about, chosen from the
  frontend's `activeProviderId` or the first drivable one. The bar icon is not
  about it; it describes the machine. Avoid: current, selected VPN.
- **Lead provider** - the provider whose mark the bar icon wears
  (`bar.providerId`): the first one up by connection order, kept in
  `connected.json`. Avoid: main provider, primary VPN.
- **Capability** - a per-provider flag (`Capability`: exit nodes, Mullvad, file
  send, accounts, networks, connection quality, live watch) that decides which
  sections the panel shows for the active provider, narrowed by what the
  daemon reports (`provider_supports`: Taildrop needs the file-sharing cap,
  Mullvad a region list). Which commands a provider has is decided by its
  backend; `ctl` also checks the exit-node capability before some actions.
  Avoid: feature, permission, support level.
- **Access denied** - the provider's CLI refuses this user outright
  (`accessDenied`, ZeroTier without its token), as opposed to a daemon that
  is down. The panel offers the backend's **authorization**: the elevated
  command that lets the user in, and the `auth` row that runs it. Avoid:
  permission error, unauthorized.
- **Switched by its networks** - a provider with no `up` / `down` of its own
  (ZeroTier): on is joined to any network, the switch leaves them all and
  joins back the ones it took. Its **remembered networks** are kept in
  `$XDG_STATE_HOME/tailgauge/networks-<id>.json`, each `wanted` until left by
  hand. See [features/zerotier.md](features/zerotier.md). Avoid: saved
  networks, favourites.
- **Daemon state** - the provider daemon's own state (`daemonState`, from
  Tailscale's `BackendState`): `Running`, `Stopped`, `NeedsLogin`... Avoid:
  backend state (the old field name), status.
- **Watch** - `tailgauge watch`, which blocks until any installed supported
  provider changes (a `live_watch` provider on its event stream, Tailscale's
  `tailscale debug watch-ipn`, when it has it, the rest polled every 2 s) and exits 0 on a change, 2 on expiry, 1 when no
  supported VPN CLI is installed. See
  [architecture/providers.md](architecture/providers.md). Avoid: listener,
  subscription.

## The panel

- **Panel** - the whole resolved surface `panel_spec` returns: bar, header,
  status, sections, footer, navigation. See
  [architecture/panel-spec.md](architecture/panel-spec.md). Avoid: model (the
  pre-0.5 TypeScript `shared/model.ts`, now gone), view, menu.
- **Panel spec** - `panel::panel_spec`, the one function that decides what
  every surface shows. Avoid: resolver, `resolvePanel` (its TypeScript name).
- **Section** - one of the seven ordered blocks (`update`, `providers`, `self`,
  `connections`, `exitNodes`, `networks`, `machines`), always emitted, drawn
  when `visible`. Avoid: group, card, submenu (GNOME draws some as submenus).
- **Row** - one fully formed line in a section: label, sublabel, icon, state,
  actions, copy options, payload. Avoid: item, entry.
- **Parity** - the rule that a user-facing feature lands on all four surfaces
  (Plasma, GNOME, Omarchy and the TUI, which is exempt from the desktops'
  layout) and that none decides anything the panel already decided, enforced
  on the desktops by `parity.rs`. Avoid: consistency, sync.
- **Ui blob** - the `--ui` JSON a frontend sends with every call: the state
  only it knows (`gather::Ui`). Avoid: options, frontend state file.
- **Optimistic toggle** - the switch position a frontend shows after a click,
  sent as `Ui.active` and winning until the daemon agrees. Absent means "ask
  the daemon"; `false` means "the user just turned it off". See
  [features/connection.md](features/connection.md). Avoid: pending state.
- **Busy** - a row with a command in flight, from the frontend's in-flight ids.
  A control is never disabled for it. Avoid: loading, disabled.
- **Cursor stop** - an entry in `panel.navigation`, the keyboard traversal
  order. See [features/keyboard.md](features/keyboard.md). Avoid: focus index,
  tab order.
- **Search key** - a row's pre-lowercased `searchKey`; a frontend filters by
  substring and nothing else. Avoid: haystack (what a frontend must not build).
- **Copy option** - one copyable value of a machine (name, DNS name, IPv4,
  IPv6, key), listed and ordered by the core. Avoid: copy action list.
- **Tsui's shape** - the TUI's layout after tsui: wordmark, menu of sections
  with values, a pane to the right, a status bar. See
  [features/tui.md](features/tui.md).

## Tailscale things

- **Connections** - the section that switches Tailscale profiles; its rows are
  accounts. See [features/accounts.md](features/accounts.md). Avoid: accounts
  section, profiles menu.
- **Exit node** - a node this machine routes its traffic through; the
  tailnet's own (`ownExitNodes`) or a Mullvad one. Avoid: gateway, proxy.
- **Mullvad region** - one city of the Mullvad fleet, keyed by country and
  city. The **recent shortlist** is up to five of them shown before the
  **picker**. See [features/exit-nodes.md](features/exit-nodes.md). Avoid:
  Mullvad server, favourites, history.
- **Taildrop** - Tailscale's file transfer: send from a machine row, receive
  through the user service into the downloads directory. See
  [features/taildrop.md](features/taildrop.md). Avoid: file sharing, drop.
- **Network** (NetBird) - a NetBird routed network you join or leave from the
  Networks section. Avoid: route, subnet.

## The binary and distribution

- **Alias** - a `tailgauge-<sub>` symlink beside the binary that
  `splice_alias` turns into a subcommand. Listed in `ALIASES`, in the
  `for alias in` loops of the installer, the packagers and the package check,
  and in the bootstrap's `ALIASES=(...)`. See [architecture/binary.md](architecture/binary.md). Avoid:
  helper, wrapper.
- **Package** - a `.deb`, `.rpm` or Arch `.pkg.tar.zst` that installs the
  binary, the aliases, the unit and the Plasma and GNOME payloads under `/usr`,
  owned by the package manager. `tailgauge-bin` is the Arch one made from the
  release archive. See [architecture/distribution.md](architecture/distribution.md#packages).
  Avoid: bundle, distro build.
- **Helpers** - the pre-0.5 shell scripts the binary replaced, and the
  `helpers.tar.gz` release asset that still carries the **bootstrap**
  (`scripts/compat/tailgauge-update`) for 0.4.0 installs. Also
  `PanelState::helpers`, the flag that gates Taildrop send, which the binary
  always sets to true. See
  [architecture/distribution.md](architecture/distribution.md),
  [features/taildrop.md](features/taildrop.md). Avoid: compat updater.
- **Attended** - `ctl` run with a terminal on stdout (and the TUI not holding
  the screen): it prints. Unattended, it opens the login page itself and
  reports failure as a notification. Avoid: interactive.
- **Frontend / payload** - a desktop package (`plasma`, `gnome`, `omarchy`);
  the payload is its copy under `frontends/` in the binary archive. Avoid:
  plugin (except Omarchy's own word), widget for the GNOME extension.
- **Skew** - an installed frontend and the binary on different versions,
  reported by `--update` and in the footer. See
  [features/updates.md](features/updates.md). Avoid: drift, mismatch.
- **Gate** - every step of `.github/workflows/ci.yml`, run locally before a
  push or a release. See [operations.md](operations.md). Avoid: CI checks,
  pre-commit.

## Sources

- `crates/tailgauge-core/src/providers/`, `panel.rs`, `panel_state.rs`, `status.rs`, `exit_nodes.rs`
- `crates/tailgauge/src/gather.rs` (`Ui`), `main.rs` (`splice_alias`), `project.rs` (`ALIASES`), `ctl.rs`, `watch.rs`
