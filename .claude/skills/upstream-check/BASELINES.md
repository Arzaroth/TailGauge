# Upstream baselines

Rewritten at the end of every `upstream-check` run.

## Last checked: 2026-10-20

| Upstream | Baseline | Notes |
| --- | --- | --- |
| basecamp/omarchy | `0a7beb7d` on `quattro` (2026-10-20) | Local omarchy is `4.0.6-1`, running on Qt `6.12.0` |
| tailscale/tailscale | CLI `v1.104.1` | Fixtures in `crates/tailgauge-core/tests/fixtures/` still match the live CLI shape |

Checked against TailGauge at `d1f40d0` (0.7.0).

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

## Backport status (2026-10-20)

### Backported: received images are no longer decoded for a notification preview

`dc2b1809` (2026-10-17) stopped `omarchy-tailscale-receive` passing
`--image` for received images: "Untrusted received files should not be decoded
automatically for a preview." It now always shows the file glyph, and the click
still opens the file. `crates/tailgauge/src/receive.rs` (`announce`, via
`IMAGE_EXTENSIONS`) still sends the path as an `image-path` hint, so any
tailnet peer allowed to Taildrop us gets an image decoded by the notification
daemon. Backported in `c6ae698` (`announce` passes no image).

### Backported: unqualified `Color` under Qt 6.12

`b83d3df0` (PR #14553, 2026-10-08): Qt 6.12 adds a QtQuick `Color` type that
shadows the shell's palette singleton. Upstream now imports
`qs.Commons as Commons` and writes `Commons.Color`, and documents the same
requirement for third-party plugins. Our widget still has 11 bare `Color.`
references across `Panel.qml`, `TailGaugeIcon.qml`, `NetBirdIcon.qml`. On this
machine (Qt 6.12.0, omarchy 4.0.6) the running shell logged no palette warning
from our plugin even before the fix. Backported in `40a3221`: every palette
reference is `Commons.Color`, and the qualified build loads in the live shell
with no warnings.

### Verified, no action

- Their `Model.js` changed only in `84cfb939` (i18n): a `tr` argument threaded
  through `displayHostName`, `accountLabel`, `parseStatus`, `parseAccounts`. No
  parsing logic moved.
- `4c1b4cb3`: `KeyboardPanel` gained an optional lazy `content` Component and
  `loadedContent`. Panels that declare children directly keep the eager
  behaviour, which is what ours does.
- `c231097d`: first-party IPC handlers are now `ShellIpc` and `omarchy-shell`
  goes over a socket first. A plain `IpcHandler` (ours) gets `SKIP` and falls
  back to `qs ipc`, so it still works, about 45ms slower per call.
- `b7e900ce` / `1a161eb5`: bar sections are now `left`, `centerBegin`,
  `center`, `centerEnd`, `tray`, `right`. Our manifest says `right`, still
  valid. `omarchy-plugin-validate build/arzaroth.tailgauge` exits 0 against the
  installed 4.0.6.
- Tailscale 1.102.4 to 1.104.1: under `cmd/tailscale/cli` and `ipn/ipnstate`
  only additive changes (`TailnetStatus.StableID`, whois, netcheck, diag). Live
  `status --json` still carries every field we read, `Self.CapMap` still has
  file-sharing, `switch --list --json` keys are unchanged, and the
  `exit-node list` header is still the column set `parse_exit_node_list`
  slices. `cargo test -p tailgauge-core` passes.

Noted while checking: this machine runs `tailscaled` 1.102.3 under a 1.104.1
client. The skew warning goes to stderr only.

## Known one-way differences, ours ahead, not to be "fixed" toward upstream

- The panel is resolved in `crates/tailgauge-core` (`panel_spec`) and rendered
  identically by every frontend. Upstream assembles its panel in QML.
- Our panel has a This device section, offline machines, the owner on every
  machine row, a machine search past eight machines, and an update banner.
  None of those exist upstream.
- Cursor traversal is one index into `panel.navigation`. Upstream keeps a
  per-section focus state machine.
- Upstream is translated (`omarchy-i18n`, 50 languages); we are English only.
