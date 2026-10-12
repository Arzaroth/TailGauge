# TailGauge

How TailGauge works lives in `brain/` (start at `brain/BRAIN.md`); this file
holds the rules a change can break. **The brain is load-bearing (hard rule):** a
change that makes a brain doc wrong fixes that doc in the same branch, and an
inconsistency you spot gets fixed with the code as the source of truth. See
"The brain (knowledge base)" below.

## Frontend parity (hard rule)

TailGauge ships one panel on four surfaces. **A user-facing feature lands on
all of them, or it is not done.**

| Surface | Where | Draws the panel |
| --- | --- | --- |
| Plasma | `plasma/org.tailgauge.plasmoid` | yes |
| GNOME | `gnome/tailgauge@arzaroth.github.io` (TypeScript, see below) | yes, as native `PopupMenu` rows |
| Omarchy | `omarchy/arzaroth.tailgauge` | yes |
| TUI | `crates/tailgauge/src/tui.rs` | yes, in tsui's layout rather than the desktops' |

The desktop frontends install separately from the binary, so a gap there is
invisible from the crate that grew the feature.

`crates/tailgauge-core/src/panel.rs` (`panel_spec`) decides **which sections
exist, in what order, when each is visible, which rows it holds, every label and
empty state, and the cursor order**. `tailgauge panel --json --ui '{...}'` is
the one call every desktop renders from; the TUI calls the same functions
in-process. A frontend draws, filters a list by substring as you type, and
hands back the state only it knows (`gather::Ui`). Nothing else. The details
are in `brain/architecture/panel-spec.md`.

This is enforced, not remembered: `crates/tailgauge-core/tests/parity.rs` reads
the frontend sources and fails on each way one has drifted before. The rules
below are the ones with an incident behind them.

- **A frontend never names a provider CLI in an argv.** It runs `tailgauge ctl
  --provider ID ...`; the argv lives on the provider's `ProviderDescriptor`, and
  which provider has which command is decided in `providers.rs` (by id today),
  while capabilities decide which sections exist. Each service once spelled
  `tailscale` into its commands, which is how a panel could show one provider
  and drive another (0.4.0).
  `no_frontend_names_a_provider_binary_in_an_argv`; a missing capability is a
  `None` argv, refused before anything spawns
  (`a_provider_is_never_asked_for_what_it_does_not_have`).
- **`panel.ts` `SECTION_IDS` lists every section `panel_spec` emits, in
  order.** A section GNOME has no slot for is silently not drawn: GNOME could
  not switch provider or join a network because `providers` and `networks`
  had no slot.
  `the_gnome_extension_has_a_slot_for_every_section_the_panel_emits`.
- **No control is gated on `busy`.** A background poll is nearly always in
  flight, and the header switch could not be clicked while it ran.
  `no_frontend_gates_a_control_on_background_work`,
  `the_switch_stays_enabled_while_a_background_poll_runs`.
- **An absent `active` in `--ui` means "ask the daemon"; `false` means "the user
  just turned it off".** Conflating them is a switch that takes a round trip to
  move, or a panel that never shows itself connected; the QML harness found the
  first on all three desktops (0.5.0).
  `the_optimistic_toggle_is_absent_rather_than_false`,
  `a_click_shows_what_was_asked_until_the_daemon_agrees`.
- **Search keys come lowercased from the core; a frontend only tests the
  substring.** The panel call once took the machine and region queries, so
  every keystroke rebuilt the panel; since 0.5.0 the core hands each searchable
  row its `searchKey` and the panel stays the same object from one keystroke to
  the next.
  `no_frontend_assembles_its_own_search_haystack`,
  `both_searches_ignore_case_in_the_query_and_in_what_they_match`.
- **QML never hands the panel's arrays to a `Repeater`.** `panel` is a new
  object on every answer, so a Repeater rebuilt every row, destroying the
  search field being typed into. Counted models and `syncRegistration()`
  instead. `no_qml_frontend_rebuilds_its_rows_just_to_redraw_them`,
  `every_frontend_keeps_its_search_field_across_a_redraw`.
- **Every subcommand a frontend spells exists.**
  `every_command_the_frontends_invoke_exists` (e2e).

### The bar icon describes the machine

The bar icon and tooltip describe every installed provider, so a click on it
never toggles a connection: it toggled a provider you could not see and left
the icon lit while the other stayed up (0.4.0). Right click on Omarchy and
middle click on Plasma and GNOME cycle the provider instead. GNOME's middle
click kept calling `toggleTailscale` until a test looked at the click side.
`the_bar_icon_describes_the_machine_not_the_panel_view` covers the icon,
`no_click_on_the_bar_icon_toggles_a_connection` (`parity.rs`) the click
handlers on all three desktops.

### The GNOME extension is compiled

Edit the `.ts` sources, never the copies under `build/`. `panel.ts` declares
the JSON the binary sends by hand: change a Rust panel struct and change it
too.

**GNOME 45 is a different shell.** Its `St.ScrollView` is an `St.Bin`; tell it
apart by `vadjustment`, never by `add_actor` (an extension that polyfills
`add_actor` sends 46-50 down the 45 path). Every section once drew one pixel
tall on 45. No CI test sees this: run `tests/gnome/shell/run.sh --gnome
45` (and a recent one) on any `extension.ts` change that touches layout.

`scripts/build.sh` refuses to package the extension without compiled schemas:
GNOME reads the schema source the moment `schemas/` exists, and a half-built
directory is worse than none (0.3.4).

## Freshness: no daemon

TailGauge runs no service to draw. Each frontend arms `tailgauge watch 300`,
polls on its refresh interval (3 s while the panel is open), and refreshes once
600 ms after a command. `watch` covers every installed supported provider
(Tailscale on the IPN bus when it has one, the rest polled every 2 s) and exits
**0 on a change, 2 on expiry, 1 when no supported VPN CLI is installed**;
frontends refresh only on 0: the shell watcher it replaced always reported
expiry, so changes never reached the panels.
`a_change_exits_zero_and_an_expiry_does_not`,
`watch_on_a_quiet_netbird_expires_rather_than_giving_up`. See
`brain/architecture/overview.md`.

## One binary, many names

Every job is a subcommand of `tailgauge`; a `tailgauge-<sub>` symlink per
subcommand keeps old key bindings working, turned into the subcommand by
`splice_alias`.

- **A new subcommand gets its alias in all three lists: `ALIASES`
  (`project.rs`), the alias loop in `scripts/install.sh`, and `ALIASES=(...)`
  in `scripts/compat/tailgauge-update` (the 0.4.0 bootstrap).** Each list was
  only checked against the subcommands, so an alias added to `install.sh` alone
  existed on a fresh install and vanished on the first `--update`.
  `the_installer_and_the_updater_write_the_same_aliases`,
  `every_alias_names_a_subcommand_the_binary_has`; the bootstrap is held only
  by a substring check in
  `the_bootstrap_downloads_an_archive_the_release_publishes`, so check its
  array by eye.
- **Updating is a flag, `--update`, and has no alias.** `tailgauge-update` is
  the old shell updater's name; the binary refuses it and an update removes
  it.
- **Notification text always follows `--`.**
  `the_message_is_never_read_as_options`.

## Releases cannot strand an install

- **`helpers.tar.gz` and its bootstrap stay published, under that name.** A
  0.4.0 install updates by fetching exactly that asset; 0.5.0 dropped it and
  left those installs with a panel calling a binary nobody had installed
  (fixed in 0.5.1).
  `the_release_still_publishes_the_helpers_asset_older_installs_ask_for`,
  `the_bootstrap_downloads_an_archive_the_release_publishes`.
- **Every version agrees.** The workspace `Cargo.toml`, the plasmoid
  `metadata.json`, the extension `metadata.json` (`version-name`, plus the
  integer `version` EGO wants) and the Omarchy `manifest.json`. A tag is never
  moved, so a forgotten file costs a patch release.
  `every_payload_declares_the_same_version_as_the_binary`,
  `the_release_refuses_a_tag_the_manifests_disagree_with`,
  `the_extension_carries_the_integer_version_ego_expects`, and CI's "Check the
  versions agree". The release skill lists every place the version lives.
- **The release asset names are selvedge's.** The distribution tests in
  `project.rs` compare the workflow against `arch_target()`, not against a
  second copy of the name.

## The updater is a crate, and it is not in this repository

Fetching a release, replacing the binary and reinstalling frontend payloads is
[selvedge](https://github.com/Arzaroth/selvedge), pinned to a tag and shared
with TokenGauge. A fix to that machinery is a selvedge release and a bump here,
not a patch in this tree. To work on both, copy `.cargo/config.toml.example` to
`.cargo/config.toml` (gitignored). See `brain/architecture/distribution.md`.

## Tests

- **e2e fakes run on an isolated `PATH`.** `crates/tailgauge/tests/e2e.rs`
  runs the real binary with `PATH` set to the fake `tailscale` / `netbird`
  scripts and nothing else, so a real CLI on the machine can never answer
  instead of the fixture. Keep it that way.
- `specification.rs` resolves whole panels from recorded daemon output with a
  fixed `now`; a new behaviour gets a case there against a fixture, not a
  hand-built `PanelState` alone.
- Plasma filters `console.log` out of the journal; a temporary probe uses
  `console.warn`.

## The brain (knowledge base)

`brain/` is the committed knowledge base describing how TailGauge works:
architecture, features, decisions, glossary. It exists so a developer or AI can
understand the project **without reading the source**. `brain/BRAIN.md` is the
entry index - it links to the `architecture/` and `features/` sub-indexes, which
link to leaf docs. Read it before hunting for where something lives, and
navigate via the indexes rather than grepping the whole tree. The `brain` skill
is the procedure.

- It is load-bearing: keep it TRUE to the code. When a change makes a brain doc
  wrong, fix the doc in the same branch, as its own docs layer on top of the
  behaviour commit.
- The code is the source of truth. If the brain disagrees with reality, reality
  wins: correct the brain.
- New feature -> add `brain/features/<name>.md`, plus a row in
  `brain/features/index.md` and the features table in `brain/BRAIN.md`. New
  cross-cutting subsystem -> add or update a `brain/architecture/*.md` and its
  index rows.
- Capture the WHY in `brain/decisions.md` (a decision with real alternatives
  gets the next `NNNN` number) and new or renamed terms in `brain/glossary.md`,
  with an `Avoid:` list, as they happen.
- A new rule a future change could break, with the incident that taught it,
  goes in the matching section of this file; the brain doc links to it rather
  than repeating the incident.
- Don't restate code: cite source paths, end every leaf with `## Sources`, and
  cross-link sibling docs with relative markdown links. Dense, skimmable,
  present tense, no em-dashes.

## Conventions

- Commit subjects start with the branch name in brackets:
  `[feature/x] feat(scope): ...`. Branches are layered: mechanical, then
  behaviour, then tests and docs, each commit green on its own.
- The gate is all of `.github/workflows/ci.yml`; run it before a push. The
  `feature-treatment` and `release` skills spell it out.
- User-visible changes go in `CHANGELOG.md` `[Unreleased]` and, where a
  setting, flag or behaviour is described there, `README.md`.
- No em-dashes in docs, comments or commit messages.
