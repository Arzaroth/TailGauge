# Decisions

The non-obvious design decisions and the "why" behind them: the things you cannot
recover by reading the code, only by reading the rationale. Grouped by area; each
entry links the doc that implements it. Decisions with real alternatives are
numbered `### NNNN`; new ones take the next number. Smaller ones are bullets.

## The panel

### 0001 The panel is resolved in the binary

**Decision.** One call, `tailgauge panel --json --ui '{...}'`, probes the VPN
CLIs, polls every installed one, and returns the whole panel: sections, rows,
labels, visibility, cursor order. The frontends draw it. Strings are English,
written once in the core.

**Why.** A panel was four CLI calls parsed three different ways, one per
frontend, feeding `shared/model.ts`. Resolving it in the binary (0.5.0) made it
one process spawn, removed 2,830 lines of service code, and measured faster on
a live 52-machine tailnet than the calls alone; nothing changed on screen. The
panel is decided in one place, which is what made the TUI (0.6.0) one more
thing that draws it. The strings are English because no translations ever
existed: `_()` and `i18n()` resolved to their own arguments while Omarchy drew
the raw strings.

**Consequences.** A frontend filters search lists itself, because a keystroke
cannot wait on a process; it hands back only what it alone knows (`--ui`).
`parity.rs` enforces the split. See
[architecture/panel-spec.md](architecture/panel-spec.md).

### 0002 The bar icon describes the machine

**Decision.** The bar icon and tooltip describe every installed provider at
once, and a click on it never toggles a connection. Right click on Omarchy and
middle click on Plasma and GNOME cycle which provider the panel is about.

**Why.** When the icon toggled, a click turned off a provider you could not see
and left the icon lit while the other stayed up (0.4.0).

**Rejected.**
- *Toggle the active provider*: acts on something the icon does not show.

**Consequences.** `gather` polls every installed provider, not just the active
one. `no_click_on_the_bar_icon_toggles_a_connection` holds the click handlers
on all three desktops to it (GNOME's middle click toggled until that test
existed). See
[architecture/frontends.md](architecture/frontends.md).

### 0003 A provider's commands live in the registry

**Decision.** Every argv a provider is driven with is a method on its
`ProviderDescriptor`. Frontends call `tailgauge ctl --provider ID ...` and never
name a provider binary. Which provider has which command is decided in
`providers.rs` (the optional methods check `self.id` today); capabilities
decide which sections the panel shows.

**Why.** Each service used to spell `tailscale` into its commands, which is how
a panel could show one provider and drive another (0.4.0).

**Rejected.**
- *Each frontend spelling its own argv*: the incident above, three times over.

See [architecture/providers.md](architecture/providers.md).

- **The optimistic toggle is absent, not `false`.** A frontend sends `active`
  only while it shows a position the daemon has not confirmed. Reading absent
  as `false` gives a panel that never shows itself connected. See
  [features/connection.md](features/connection.md).
- **No control is gated on `busy`.** A background poll is always running; a
  switch disabled for it is a switch that cannot be clicked.
- **Search keys come lowercased from the core.** Since 0.5.0 each searchable
  row carries its `searchKey`, so the panel call no longer takes the queries
  and a keystroke no longer rebuilds the panel; a frontend only tests the
  substring. See
  [features/machines.md](features/machines.md).
- **GNOME draws native `PopupMenu` rows** and ignores `panel.navigation`, so
  arrows, Enter and type-ahead behave as in every other extension. See
  [features/keyboard.md](features/keyboard.md).

## Exit nodes

- **A Mullvad node is set by IP, a tailnet node by DNS name.** The CLI will not
  resolve a Mullvad name. The choice is `panel::exit_node_target`, applied in
  `ctl::target_of` to the row payload the frontend hands back, so no frontend
  computes an address. See [features/exit-nodes.md](features/exit-nodes.md).

## The binary

### 0004 One binary, with aliases spliced into argv

**Decision.** Every job is a subcommand of one `tailgauge` binary. The
installer and the updater write a `tailgauge-<sub>` symlink per subcommand,
and `splice_alias` turns the invoked name into the subcommand.

**Why.** Eight shell helpers (969 lines of bash, including `tailscale status
--json` parsed with `sed`) duplicated the parsing the core does properly. Key
bindings and scripts written against `tailgauge-ctl toggle` must keep working.

**Rejected.**
- *Keeping the shell helpers*: the parsing they did is what the core exists for.
- *Dropping the old names*: breaks every existing key binding.

**Consequences.** `ALIASES` (`project.rs`), the alias loop in `install.sh` and
`ALIASES=(...)` in `scripts/compat/tailgauge-update` must list the same names. See [architecture/binary.md](architecture/binary.md).

- **Updating is a flag, not a subcommand.** A subcommand would want a
  `tailgauge-update` alias, which is the name of the old shell updater; the
  binary refuses that name instead.
- **The TUI is a subcommand, gathering in-process.** One place decides the
  panel; a second binary would be a second place to keep in step.
- **`watch` exits 0 only on a change.** The shell watcher it replaced always
  reported expiry, so changes never reached the panels; and it wakes on IPN
  state keys only, not on netmap churn.

## Distribution and updates

### 0005 The updater is a shared crate, selvedge

**Decision.** Fetching a release, replacing the binary, reinstalling frontend
payloads and finding external programs live in
[selvedge](https://github.com/Arzaroth/selvedge), pinned by tag. TailGauge
keeps only `project.rs`: its name, repository, aliases and frontends.

**Why.** It was 1,656 lines here and much the same in TokenGauge. Moving it
found three bugs one project had been living with (a stale lock file, a check
undoing an update, the old updater left installed).

**Rejected.**
- *Two vendored copies*: the bugs above.

See [architecture/distribution.md](architecture/distribution.md).

- **The binary archive carries the frontend payloads.** `--update` reinstalls
  whichever frontends are present from the same archive, so the binary and the
  QML it feeds cannot end up a release apart. `--install-frontend` installs
  from the binary's own release, not the latest.
- **A replaced payload is moved aside, not deleted,** so a failed install
  leaves the copy that was working.
- **`helpers.tar.gz` stays published.** A 0.4.0 install only knows that asset;
  0.5.0 dropped it and stranded every such install until 0.5.1 put it back
  with a bootstrap. See [operations.md](operations.md).
- **The panel reads only the cached update check.** A render never waits on
  GitHub; the frontends refresh the cache every six hours. See
  [features/updates.md](features/updates.md).

### 0006 One installer, which builds in a clone and fetches when piped

**Decision.** `scripts/install.sh` is both the `curl | bash` installer and the
developer's install-from-checkout. Where its sources come from is the only
thing that differs; every install step after that is shared.

**Why.** The desktop steps (kpackagetool6, the GNOME stage-and-swap, Omarchy's
validate, enable and placement, the Taildrop unit) are most of the script, and
two copies would drift the way the alias lists did. The release archive already
carries every payload in selvedge's layout, so the piped path needs a download
and nothing else.

**Rejected.**
- *A second, release-only script* (TokenGauge's shape): two copies of the
  desktop steps.
- *Install the binary, then `tailgauge --install-frontend`*: downloads the
  archive twice, and selvedge copies a payload without enabling the Omarchy
  plugin or the unit.

See [architecture/distribution.md](architecture/distribution.md#installsh).

## Taildrop

- **Receive claims each file with a hard link into a staging directory.** No
  check-then-rename race, and it stays on one filesystem. See
  [features/taildrop.md](features/taildrop.md).
- **The GNOME toggle shortcut is bound in the extension, with no default.** It
  collides with nothing. It still needs the binary: the key calls
  `toggleTailscale`, which spawns `tailgauge ctl` and does nothing until the
  binary has answered a panel. See [features/keyboard.md](features/keyboard.md).

## Sources

- `CHANGELOG.md` (0.4.0, 0.5.0, 0.5.1, 0.6.0)
- `crates/tailgauge-core/src/panel.rs`, `crates/tailgauge-core/src/providers.rs`
- `crates/tailgauge-core/tests/parity.rs`
- `crates/tailgauge/src/main.rs` (`splice_alias`), `crates/tailgauge/src/project.rs`
- `crates/tailgauge/src/watch.rs`
- `scripts/install.sh`, `scripts/compat/tailgauge-update`
- `gnome/tailgauge@arzaroth.github.io/extension.ts`, `gnome/tailgauge@arzaroth.github.io/provider.ts`
