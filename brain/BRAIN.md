# BRAIN - TailGauge knowledge base

The map of the codebase, written so a developer or AI can understand TailGauge
**without reading the source**. Start here, follow the links, stop when you have
the answer. Every leaf doc cites the real source files if you need to go deeper.

> Product: **TailGauge** - Tailscale (and NetBird) in the desktop panel:
> connection state, on/off, account switching, exit nodes including Mullvad
> regions, machines with copy actions, and Taildrop. One Rust binary resolves the
> whole panel; KDE Plasma 6, GNOME Shell, Omarchy and a TUI draw it. Stack
> snapshot in [stack.md](stack.md); the current version is `version` in the
> workspace `Cargo.toml`.

## How to use this

1. Know the topic? Jump straight to its file via the indexes below.
2. Have a question, not a topic? Scan **Find by question** first.
3. Need a term defined? See [glossary.md](glossary.md).
4. Want the "why" behind a design? See [decisions.md](decisions.md).
5. About to change something? The rules a change can break, each with the
   incident behind it, are in the repo's `CLAUDE.md`.

The brain is a tree: this file -> two folder indexes
([architecture/index.md](architecture/index.md),
[features/index.md](features/index.md)) -> leaf docs. Don't grep the whole tree;
use the indexes.

## Top-level docs

| Doc | What's here |
|---|---|
| [stack.md](stack.md) | Rust edition and MSRV, the two crates and their dependencies, selvedge, TypeScript, pnpm, Qt 6, the programs called at runtime. |
| [operations.md](operations.md) | The gate, build and install scripts, the 0.4.0 upgrade path, releases, upstream watching, where each desktop logs. |
| [glossary.md](glossary.md) | Domain and technical terms, with the synonyms to avoid. |
| [decisions.md](decisions.md) | The non-obvious decisions and their rationale. |

## Architecture (the how) - [architecture/index.md](architecture/index.md)

| Doc | What's here |
|---|---|
| [overview.md](architecture/overview.md) | Workspace, data flow from the CLIs to four surfaces, freshness without a daemon, layout rules. Read first. |
| [panel-spec.md](architecture/panel-spec.md) | `panel_spec`, the JSON shape, the seven sections, row fields, the `--ui` hand-back, the parity guards. |
| [frontends.md](architecture/frontends.md) | Plasma, GNOME, Omarchy (and the TUI): data path, clicks, settings, per-desktop quirks. |
| [providers.md](architecture/providers.md) | The provider registry, capabilities, argv, the parsers and fixtures, gathering, `tailgauge watch`. |
| [binary.md](architecture/binary.md) | The `tailgauge` command line, alias dispatch, `ctl`, the helper subcommands. |
| [testing.md](architecture/testing.md) | Specification, parity, e2e, the QML and GNOME harnesses, the real GNOME Shell run. |
| [distribution.md](architecture/distribution.md) | Binary names, the selvedge contract, release assets, stores, `--install-frontend`, the 0.4.0 upgrade. |

## Features (the what) - [features/index.md](features/index.md)

| Doc | What's here |
|---|---|
| [connection.md](features/connection.md) | The header switch, the optimistic flip, login, `tailgauge ctl`. |
| [accounts.md](features/accounts.md) | The Connections section: Tailscale profiles and the operator offer. |
| [exit-nodes.md](features/exit-nodes.md) | Tailnet exit nodes, the recent Mullvad shortlist, the picker. |
| [machines.md](features/machines.md) | This device, peers, offline sorting, copy options, search. |
| [netbird.md](features/netbird.md) | NetBird as a second provider, and switching providers. |
| [taildrop.md](features/taildrop.md) | Sending files from a machine row, the receive service. |
| [keyboard.md](features/keyboard.md) | The letters, `panel.navigation`, per-desktop keys, the global toggle. |
| [tui.md](features/tui.md) | `tailgauge tui`, the panel in a terminal. |
| [updates.md](features/updates.md) | `--check-update`, `--update`, the banner, version skew, `--install-frontend`. |

## Find by question

| If you're asking... | Go to |
|---|---|
| Where is the panel built? Why can't a frontend decide what to show? | [architecture/panel-spec.md](architecture/panel-spec.md) |
| How do I add a section or a row field? | [architecture/panel-spec.md](architecture/panel-spec.md), [architecture/frontends.md](architecture/frontends.md) |
| What does `parity.rs` forbid, and why did my frontend change fail it? | [architecture/panel-spec.md](architecture/panel-spec.md) |
| How does the panel notice a change without a daemon? | [architecture/overview.md](architecture/overview.md), [architecture/providers.md](architecture/providers.md) |
| How do I add a provider, or a capability? | [architecture/providers.md](architecture/providers.md) |
| What does NetBird not support? | [features/netbird.md](features/netbird.md) |
| Why does a click show the switch moved before the daemon agrees? | [features/connection.md](features/connection.md) |
| Why does the bar icon not toggle the connection? | [architecture/frontends.md](architecture/frontends.md), [decisions.md](decisions.md) |
| How is a Mullvad exit node set, and where does the shortlist live? | [features/exit-nodes.md](features/exit-nodes.md) |
| How does search work without rebuilding the panel? | [features/machines.md](features/machines.md), [architecture/panel-spec.md](architecture/panel-spec.md) |
| Why does `tailgauge-ctl` still work? Where is a subcommand's alias? | [architecture/binary.md](architecture/binary.md), [architecture/distribution.md](architecture/distribution.md) |
| Why is Taildrop send missing on a store install? | [features/taildrop.md](features/taildrop.md), [architecture/distribution.md](architecture/distribution.md) |
| How does `--update` keep the binary and the widgets on one release? | [features/updates.md](features/updates.md), [architecture/distribution.md](architecture/distribution.md) |
| Why is `helpers.tar.gz` still published? | [architecture/distribution.md](architecture/distribution.md) |
| What does CI run? How do I cut a release? | [operations.md](operations.md) |
| How is the GNOME menu tested in a real shell, and why GNOME 45? | [architecture/testing.md](architecture/testing.md), [architecture/frontends.md](architecture/frontends.md) |
| Where do Plasma, GNOME and Omarchy log? | [operations.md](operations.md) |

## Maintaining the brain

This brain is **load-bearing documentation**: it must stay true to the code.
When a change makes a brain doc wrong, fix the doc in the same change. When you
discover the brain disagrees with reality, the code wins: correct the brain and
note it. The enforceable version of this rule lives in the repo's `CLAUDE.md`
("The brain (knowledge base)"). New feature -> new `features/<name>.md` + a row
in [features/index.md](features/index.md) + a row here. New subsystem -> an
`architecture/*.md` + rows in its index and here. New term ->
[glossary.md](glossary.md); new decision -> [decisions.md](decisions.md).
