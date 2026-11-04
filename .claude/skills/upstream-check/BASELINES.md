# Upstream baselines

Rewritten at the end of every `upstream-check` run.

## Last checked: 2026-11-04

| Upstream | Baseline | Notes |
| --- | --- | --- |
| basecamp/omarchy | `9b525a93` on `quattro` (2026-11-04) | Local omarchy is `4.0.6-1`, running on Qt `6.12.0` |
| tailscale/tailscale | CLI `v1.104.5` | Local CLI is 1.104.2. Fixtures in `crates/tailgauge-core/tests/fixtures/` still match the live CLI shape |
| netbirdio/netbird | `v0.82.0` (2026-10-29), HEAD `313f71fb` | Local CLI is 0.82.0. Fixtures date from 2026-09-16 (about v0.78.2); `status --json` live still matches them. `networks list` not recaptured: the local client's login has expired |
| GNOME Shell | `51.1` | **`metadata.json` lists 45 to 50; 51.0 shipped 2026-09-14 and is not listed** |
| Plasma | `v6.8.2` (2026-10-27) | The plasmoid declares `X-Plasma-API-Minimum-Version` 6.0; still the 6.x API |
| selvedge | pinned `v0.8.0`, latest `v0.13.0` | 0.9 to 0.13 add brands, plans, usage and catalog for TokenGauge; no updater fix, no bump needed |
| tsui | `2018695b` on `main` (2026-10-25) | |
| trayscale | `67266aa8` on `master` (2026-10-26) | |
| KTailctl | `64ca9beb` on `main` | |

Checked against TailGauge at `f3b8c01` (0.8.0).

## Where we forked from

- Omarchy: the port landed 2026-08-29 (`982b01b`), against omarchy
  `4.0.0.alpha` and `omarchy.tailscale` 1.0.0. The model was their `Model.js`
  of that date plus our own additions; since the Rust port (0.5.0) it lives in
  `crates/tailgauge-core` (`status.rs`, `accounts.rs`, `exit_nodes.rs`,
  `panel.rs`). `tailgauge send` / `tailgauge receive`
  (`crates/tailgauge/src/send.rs`, `receive.rs`) are their
  `omarchy-tailscale-*` scripts with the file chooser and notifier swapped for
  desktop-neutral ones.
- The Omarchy frontend (`omarchy/arzaroth.tailgauge`) landed 2026-09-02 against
  the same shell. It is a third-party plugin and shares no files with theirs
  except the dot-grid icon.

## Backport status (2026-11-04)

### Diverged on purpose: received images still get no notification preview

`d743f8e8` (2026-10-22) reverted the `dc2b1809` change we backported in
`c6ae698`: `omarchy-tailscale-receive` passes `--image` again for image
extensions, reasoning that Taildrop only accepts files from your own devices.
Ours keeps `image: None` in `announce` (`crates/tailgauge/src/receive.rs`): a
compromised or shared device of your own is still a decoder input nobody asked
for, and the click still opens the file. Revisit only if a user asks.

### Open candidates

- **GNOME 51 is refused.** `gnome/tailgauge@arzaroth.github.io/metadata.json`
  stops at `"50"`, so GNOME disables the extension on 51 as incompatible. Add
  `"51"` after `tests/gnome/shell/run.sh --gnome 51` passes.
- **Peer key expiry is never shown.** tsui `9fbd5120` shows "Expired" for an
  expired peer key, and `f9ba9797` added a machine-details submenu with key
  expiry in it. `crates/tailgauge-core` reads neither `KeyExpiry` nor
  `Expired`, and an expired key is exactly the peer that will not answer.
  It would be a field in the expanded machine row in `panel_spec`, plus a
  fixture case, and needs no per-frontend work.

- **Animations ignore reduce-motion.** `8918fbae` / `aaccd247` route every
  animation in their tailscale panel through `Style.duration()` and change the
  phrase swap to 150ms / 200ms `Easing.OutCubic`. Ours
  (`omarchy/arzaroth.tailgauge/Panel.qml`, `phraseSwap` and the busy-row
  `NumberAnimation on rotation`) still uses raw 180 / 260 / 900ms, so they run
  with animations off. Omarchy-only, a few lines.
- **Huge tailnets.** `c872d57c` polls `status --json --peers=false` while the
  panel is closed, publishes arrays only when their content changed, and
  virtualizes the machine list (measured: 4.6s to 0.17s of shell CPU per poll
  on 3,900 peers). Every TailGauge poll runs full `status --json` and
  `panel_spec` emits every machine row; `gather::Ui` carries no "panel open"
  flag. Not hit on a ~50-peer tailnet. A real backport is a `Ui` field, a
  light-status path in `gather.rs` / `providers.rs`, and row virtualization on
  all four surfaces.

### Verified, no action

- `Model.js` unchanged since 2026-10-20.
- `1e2d9f9b` stacks Taildrop toasts by passing `--app-name Taildrop`; ours
  already sends `--app-name=TailGauge` (`crates/tailgauge/src/notify.rs`).
- `d743f8e8` also ordered their receive unit after UWSM's env import, because
  their strings need `OMARCHY_PATH`. Ours are plain English and
  `systemd/tailgauge-receive.service` reads no session env; its `~` shortening
  (`tildify`) was never affected by their quoting bug.
- `shell/Ui` changes to components we use are additive: `KeyboardPanel`
  gained card morphing, live height growth and press feedback;
  `PanelKeyCatcher` a `reorderableSideways` flag; `PanelToolTip` word wrap;
  `BarIconButton` a press give. `Panel.qml`'s `ShellIpc` now gets an empty
  target when `manageIpc` is false, which is what ours sets (it runs its own
  `IpcHandler`). `PluginBarApi` gained `popoutCardFor` / `notePopoutCard`,
  used by `KeyboardPanel` itself. `ScreenMoveRemap` was removed; we never
  imported it.
- `PluginRegistry.qml` (`c726e6a7`, `a0010440`): bytecode-reload and bar-entry
  restore fixes, no schema change. `omarchy-plugin-validate
  build/arzaroth.tailgauge` exits 0 against 4.0.6.
- Tailscale 1.104.1 to 1.104.5: no file under `cmd/tailscale/cli` or
  `ipn/ipnstate` changed (SSH hardening, DNS, magicsock). Live `status --json`
  carries every field we read with `Self.CapMap` file-sharing, `switch --list
  --json` keys are unchanged, the `exit-node list` header matches, and
  `cargo test -p tailgauge-core` passes.
- NetBird 0.78.2 to 0.82.0: on the paths we drive, only `dd38fabc` (opt-in
  `--table` on `status`) and `up` changes that touch no flag we pass.
- Peers, first look (no prior baseline): trayscale's exit-node fixes
  (`212e4007`, `22d7ed36`) live in its LocalAPI layer, which we do not have;
  the rest is a GTK refactor. KTailctl only added a Nix flake and a
  startup-feedback fix. tailray added exit-node selection, which we have.
  tailscale-gnome-qs has not been pushed since 2025-03 and is not watched.

## Known one-way differences, ours ahead, not to be "fixed" toward upstream

- The panel is resolved in `crates/tailgauge-core` (`panel_spec`) and rendered
  identically by every frontend. Upstream assembles its panel in QML.
- Our panel has a This device section, offline machines, the owner on every
  machine row, a machine search past eight machines, and an update banner.
  None of those exist upstream.
- Cursor traversal is one index into `panel.navigation`. Upstream keeps a
  per-section focus state machine.
- Upstream is translated (`omarchy-i18n`, 50 languages); we are English only.
