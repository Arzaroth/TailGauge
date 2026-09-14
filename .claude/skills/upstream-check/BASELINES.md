# Upstream baselines

Rewritten at the end of every `upstream-check` run.

## Last checked: 2026-09-14

| Upstream | Baseline | Notes |
| --- | --- | --- |
| basecamp/omarchy | `b679363b` on `quattro` (2026-09-14) | Their `omarchy.tailscale` widget is still at `1.0.0`; local omarchy is `4.0.3-1` |
| tailscale/tailscale | CLI `1.102.4` | `test/fixtures/` is still captured from `1.102.3`; no CLI surface changed between the two |

Checked against TailGauge at `75bdbfa` (0.3.4).

## Where we forked from

- Omarchy: the port landed 2026-08-29 (`982b01b`), against omarchy
  `4.0.0.alpha` and `omarchy.tailscale` 1.0.0. `shared/model.ts` is their
  `Model.js` of that date plus our own additions; `bin/tailgauge-send` and
  `bin/tailgauge-receive` are their `omarchy-tailscale-*` scripts with the file
  chooser and notifier swapped for desktop-neutral ones.
- The Omarchy frontend (`omarchy/arzaroth.tailgauge`) landed 2026-09-02 against
  the same shell. It is a third-party plugin and shares no files with theirs
  except the dot-grid icon.

## Backport status (2026-09-14)

Nothing outstanding.

Their `Model.js` has not changed since 2026-07-27 (`e57f3b28`, the Taildrop
commit), which predates our fork. Their whole widget directory has been quiet
since 2026-08-26. There is no parser fix upstream that we lack.

### The plugin auth boundary (verified, no action)

`e78d89ee` (2026-09-07, PR #9618) stopped injecting the host `Bar` and shell
into third-party plugins and hands them `Ui/PluginBarApi.qml` and
`services/PluginShellApi.qml` facades instead. It is already live here: omarchy
`4.0.3-1`, installed 2026-09-08.

Checked, all four of our uses survive the facade:

- `bar.foreground`, `bar.urgent`, `bar.fontFamily` are mirrored on `PluginBarApi`.
- `bar.shell.updateEntryInline(id, settings)` is on `PluginShellApi` with the
  same signature.

Two nearby changes that do not reach us:

- `bar.centerHoverRevealSuppressed` became readonly, with a
  `setCenterHoverRevealSuppressed()` setter. Their clock and weather panels were
  updated to prefer the function. We never touch that property.
- The manifest schema grew an optional `omarchy: { capabilities: [...] }`.
  `trustedCapabilities()` returns `[]` for anything not first-party, so a
  third-party plugin cannot self-declare one; it only inherits via
  `omarchy.clonedFrom` naming a first-party plugin. The sole capability so far
  is `authentication`, which we do not need.

All twelve `shell/Ui` components `Panel.qml` imports still resolve, and
`omarchy-plugin-validate build/arzaroth.tailgauge` exits 0.

`41b6cc69` (2026-09-06, video wallpaper) only adds `BackgroundMedia.qml` and
`BackgroundVideo.qml` to `shell/Ui`. Nothing we import moved.

### Tailscale 1.102.4 (verified, no action)

Zero changes under `cmd/tailscale/cli/` or `ipn/ipnstate` between `v1.102.3`
and `v1.102.4`; the four commits are netmap-delta and k8s-operator work.
Verified against the live CLI rather than the notes:

- `parseExitNodeList` takes every column offset from the header via `indexOf`
  and its header regex allows leading whitespace, so the auto-sized columns and
  the leading space in real output are both handled. The fixture's wider columns
  are a property of its content, not a shape difference.
- `status --json` still carries `BackendState`, `Self`, `Peer`, `User`,
  `TailscaleIPs`, and per-node `ID`/`UserID`/`HostName`/`DNSName`/`Online`/
  `ExitNodeOption`/`ExitNode`/`OS`.
- `Self.CapMap` still carries `https://tailscale.com/cap/file-sharing`, so
  `hasFileSharing` resolves on both its paths.

Noted while checking: this machine runs `tailscaled` 1.102.2 under a 1.102.3
client, so the CLI prints a version-skew warning. It cannot reach a parser -
`Service.qml` collects stdout and stderr in separate collectors.

## Known one-way differences, ours ahead, not to be "fixed" toward upstream

- The panel is resolved in `shared/model.ts` and rendered identically by three
  frontends. Upstream assembles its panel in QML.
- Our panel has a This device section, offline machines, the owner on every
  machine row, a machine search past eight machines, and an update banner.
  None of those exist upstream.
- Cursor traversal is one index into `panel.navigation`. Upstream keeps a
  per-section focus state machine.
