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
backend (`providers/<id>.rs`, decision 0009). Frontends call `tailgauge ctl
--provider ID ...` and never name a provider binary. Which provider has which
command is decided by which backend implements it; capabilities decide which
sections the panel shows.

**Why.** Each service used to spell `tailscale` into its commands, which is how
a panel could show one provider and drive another (0.4.0).

**Rejected.**
- *Each frontend spelling its own argv*: the incident above, three times over.

See [architecture/providers.md](architecture/providers.md).

### 0008 The bar icon wears the mark of the provider that connected first

**Decision.** The bar icon is drawn as one provider's mark: the one that is
up, or with several up, the one that came up first, with the number up raised
beside it. With nothing up it is the provider asking for a login, else the
preferred installed one. The order is kept in `connected.json` in the cache
directory, rewritten only when it changes, under a lock and stamped on the
clock since boot, so a slow poll cannot write an older order over a newer one
and a wall clock that goes back cannot either.

**Why.** One fixed mark (Tailscale's dots) said nothing about which VPN was
connected once NetBird was supported. Which one came first is only known by
having watched it, and nothing runs between two polls, so the order has to
outlive the call that saw it.

**Rejected.**
- *The active provider's mark*: the panel view would change the icon, against
  [0002](#0002-the-bar-icon-describes-the-machine).
- *Registry order alone*: Tailscale would take the icon from a NetBird that
  had been up for hours the moment it joined.
- *An order the frontend remembers and hands back in `--ui`*: three copies of
  the same state, and the TUI and a fresh desktop session would disagree.

**Consequences.** Only `panel_state` writes the file, so plain `tailgauge` and
`watch` never touch it. A count drawn over the mark was illegible at 16 px; it
sits beside it instead. NetBird's mark runs along the usual slash, so its
disconnected slash goes the other way. See
[architecture/panel-spec.md](architecture/panel-spec.md).

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


### 0009 Each provider is a backend behind one trait, and its capabilities narrow at runtime

**Decision.** `ProviderDescriptor` says what a provider is; its `backend`
(`Backend`, one implementation per provider) says what it does, with every
optional command `None` by default. `provider_supports` answers from the
row's capabilities narrowed by what the daemon reports: Taildrop needs the
file-sharing cap, Mullvad needs a region list.

**Why.** A review asked for a small backend interface in front of the model,
so that Headscale's gaps and NetBird's go through the same code path. The
optional commands each checked `self.id`, and a third provider (ZeroTier)
would have made every one of them a match. Headscale runs the Tailscale CLI,
so its gap (no Mullvad) cannot be a fixed row: it is what the daemon says.
Recording a Headscale tailnet showed it grants file sharing, so Mullvad is its
only gap, and the panel already hid an empty region list; the narrowing makes
that the capability's job rather than the section's.

**Rejected.**
- *A `match` on the id in each method*: every new provider edits every method,
  and a forgotten arm is a silent `None` only by luck.
- *A Headscale provider row*: the same CLI probed twice, and a switcher
  offering two faces of one daemon.
- *Detecting Headscale from the status*: nothing in `tailscale status --json`
  names the control server; what it allows is the only reliable signal.

**Consequences.** Watch's event stream became the `live_watch` capability and
the backend's `events()`, and the operator prompt became any backend's
`authorization()`. See [architecture/providers.md](architecture/providers.md).

### 0010 ZeroTier's switch leaves and rejoins its networks

**Decision.** ZeroTier's backend is `Switch::Networks`: no `up` or `down`. On
is joined to any network; turning it off leaves every joined network, and on
joins back the ones it took. The binary remembers each network joined under
`$XDG_STATE_HOME/tailgauge/networks-zerotier.json`, with a `wanted` flag that
only the user's own join or leave and the switch set, never a poll. Machines
are listed by ZeroTier address.

**Why.** The ZeroTier service always runs and knows no global down. Leaving a
private network keeps its member authorized on the controller, so a rejoin
needs no approval and gets the same addresses (checked against a real
controller). ZeroTier forgets a network the moment it is left, hence the
memory, kept as state since losing it strands the user. Peer names live only
on the controller.

**Rejected.**
- *Stop the service*: needs root or a polkit rule, and a stopped service can
  report nothing to the panel.
- *No switch for ZeroTier*: a header that behaves differently per provider.
- *Ask the controller for member names*: an API token in every frontend's
  settings and network calls from the binary, for a nicer label.

See [features/zerotier.md](features/zerotier.md).

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

### 0007 Packages are made from the release archive

**Decision.** `.deb`, `.rpm` and `tailgauge-bin` repackage the per-arch
archive: its static musl binary and its built payloads, installed under
`/usr`. The Omarchy widget is left out, and a packaged binary is never
replaced by `--update`.

**Why.** The archive is what `--update` installs, so a package cannot carry
different bytes. A static binary is the only one a single package can promise
to Debian 12 and Arch alike; the glibc build needed 2.38. Omarchy reads
third-party plugins from the user's config and nowhere else, and a package
must not write into a home.

**Rejected.**
- *Build each package from source in its distribution*: a toolchain per
  distribution in CI, and packages whose bytes differ from the archive's.
- *Build the glibc binary on an older runner*: still a floor (whatever that
  runner's glibc is), where musl has none.
- *Ship the Omarchy payload under `/usr/share/tailgauge` for the user to copy*:
  a second, unversioned copy that a package upgrade would leave behind.

See [architecture/distribution.md](architecture/distribution.md#packages).

## Taildrop

- **Receive claims each file with a hard link into a staging directory.** No
  check-then-rename race, and it stays on one filesystem. See
  [features/taildrop.md](features/taildrop.md).
- **The GNOME toggle shortcut is bound in the extension, with no default.** It
  collides with nothing. It still needs the binary: the key calls
  `toggleTailscale`, which spawns `tailgauge ctl` and does nothing until the
  binary has answered a panel. See [features/keyboard.md](features/keyboard.md).
- **A received image is never the notification's picture, though Omarchy's is
  again.** Their `omarchy-tailscale-receive` dropped the preview (`dc2b1809`)
  and brought it back two weeks later (`d743f8e8`), on the grounds that
  Taildrop only takes files from your own devices. One of your own devices
  can still be compromised, and a click still opens the file, so the preview
  buys a thumbnail at the price of a decoder running on input nobody opened.
  See [features/taildrop.md](features/taildrop.md).

## Sources

- `CHANGELOG.md` (0.4.0, 0.5.0, 0.5.1, 0.6.0)
- `crates/tailgauge-core/src/panel.rs`, `crates/tailgauge-core/src/providers/mod.rs`
- `crates/tailgauge-core/tests/parity.rs`
- `crates/tailgauge/src/main.rs` (`splice_alias`), `crates/tailgauge/src/project.rs`
- `crates/tailgauge/src/watch.rs`
- `scripts/install.sh`, `scripts/compat/tailgauge-update`
- `gnome/tailgauge@arzaroth.github.io/extension.ts`, `gnome/tailgauge@arzaroth.github.io/provider.ts`
