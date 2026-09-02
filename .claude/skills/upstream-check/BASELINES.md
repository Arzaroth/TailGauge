# Upstream baselines

Rewritten at the end of every `upstream-check` run.

## Last checked: 2026-09-02

| Upstream | Baseline | Notes |
| --- | --- | --- |
| basecamp/omarchy | `7eca64e2` on `quattro` (2026-09-02) | Their `omarchy.tailscale` widget is at `1.0.0`, shipped in omarchy `4.0.2-1` |
| tailscale/tailscale | CLI `1.102.3` | The version `test/fixtures/` was captured from |

## Where we forked from

- Omarchy: the port landed 2026-08-29 (`982b01b`), against omarchy
  `4.0.0.alpha` and `omarchy.tailscale` 1.0.0. `shared/model.ts` is their
  `Model.js` of that date plus our own additions; `bin/tailgauge-send` and
  `bin/tailgauge-receive` are their `omarchy-tailscale-*` scripts with the file
  chooser and notifier swapped for desktop-neutral ones.
- The Omarchy frontend (`omarchy/arzaroth.tailgauge`) landed 2026-09-02 against
  the same shell. It is a third-party plugin and shares no files with theirs
  except the dot-grid icon.

## Backport status (2026-09-02)

Nothing outstanding. This is the first baseline: no upstream review has run yet,
so the entry above records where we stand rather than what was triaged.

Known one-way differences, ours ahead, not to be "fixed" toward upstream:

- The panel is resolved in `shared/model.ts` and rendered identically by three
  frontends. Upstream assembles its panel in QML.
- Our panel has a This device section, offline machines, the owner on every
  machine row, a machine search past eight machines, and an update banner.
  None of those exist upstream.
- Cursor traversal is one index into `panel.navigation`. Upstream keeps a
  per-section focus state machine.
