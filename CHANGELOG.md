# Changelog

All notable changes to this project are documented here.

## [Unreleased]

### Added

- **One binary, `tailgauge`, replaces the eight shell helpers.** A subcommand
  per job - `ctl`, `watch`, `notify`, `send`, `receive`, `file-select`, `copy`,
  `update` - and a symlink per subcommand beside it, so `tailgauge-ctl toggle`
  on a key binding still resolves: the binary reads the name it was invoked as.
  969 lines of bash are gone, including a `tailscale status --json` parsed with
  `sed` and an exit-node table sliced with `awk`.

- **The release publishes a binary archive per architecture**, x86_64 and
  aarch64, carrying the binary and the three frontend payloads. `tailgauge
  --update` replaces the binary next to itself and reinstalls whichever
  frontends are present from the same archive, so the binary and the QML it
  feeds cannot end up a release apart.

- **`tailgauge --install-frontend <plasma|gnome|omarchy|all>`** installs a
  frontend that is not there yet, from the release the running binary belongs
  to - the path for a machine that changed desktops. `--update` only refreshes
  what is already present.

- **Update reports frontend skew.** An installed frontend that disagrees with
  the binary is the failure the frontend payloads exist to prevent, so
  `--update` says so even when there was nothing to update, and names the
  command that fixes it.

### Fixed

- **The IPN watcher reports the change it saw.** `tailgauge-watch` returned 0
  from inside a `while` that was the last element of a pipeline, so the return
  ended the subshell and the unconditional `return 2` below it answered for the
  function: every state change on the bus was reported as an expired wait. The
  frontends refresh on 0 and only on 0, so the bus has never woken a panel -
  the poll floor has been carrying it.

### Changed

- **Typing in a search field no longer rebuilds the panel.** The model now
  hands each searchable row a `searchKey` and says which field filters it, and
  the three frontends do nothing but test the substring. `resolvePanel` no
  longer takes the machine and region queries, so the panel it returns is the
  same object from one keystroke to the next.

- **Neither store can install the binary**, as neither could install `bin/`, so
  a store-installed TailGauge is still the panel only. Upgrading from 0.4.0 or
  earlier means re-running the install script: the helpers tarball went with
  the helpers, so the old shell updater has no asset left to fetch.

- **Updating is a flag, not a subcommand.** `tailgauge --check-update` prints
  the update status as JSON and `tailgauge --update` installs it, replacing
  `tailgauge-update --check --json` and `--apply`. The panel reads that status
  directly, so the update object it sees is `{current, latest, available,
  checked_ms, notified}`: the banner always offers to install, since no store
  owns any part of TailGauge, and the footer reads the binary's version from
  `current` rather than out of a targets array. There is no `tailgauge-update`
  symlink, because there is no `update` subcommand for it to point at.

## [0.4.0]

### Added

- **The bar icon describes the machine, not the panel's current view.** Every
  installed provider is polled, one idle provider per tick, so switching which
  one the panel shows no longer reads as a disconnection from one that is still
  up. Hovering the icon lists each installed provider and what it is doing,
  the one being viewed first.

- **A refresh control sits in the panel header** on every desktop, rather than
  being a middle click on the bar that nothing advertised.

- **A machine row opens onto what it is actually doing**: how the tunnel is
  carried and its latency, the endpoint, the last handshake, bytes each way,
  and the routes it carries. Both providers report this; only what a provider
  actually sent becomes a row, so a peer that said nothing grows no arrow.

- **NetBird is a second provider TailGauge can drive.** It parses
  `netbird status --json` into the same machine list, self row and copy actions
  Tailscale gets, and adds a Networks section for the routes
  `netbird networks list` reports, which you can join and leave from the panel.
  What NetBird does not have is absent rather than empty: no exit nodes, no
  Mullvad regions, no account switching, no Taildrop.

- **A VPN section switches between installed providers**, shown only when more
  than one is installed and drivable. Switching clears the panel before it
  refreshes, so one provider's machines are never listed under the other's
  name, and the choice is remembered per desktop.

- **TailGauge detects which VPN CLI you actually have.** A provider registry
  names every provider it knows how to drive, the binary that proves the
  provider is installed, and the features that provider has. One `which` per
  refresh probes them all, and the panel is resolved against whichever one is
  active: a provider with no exit nodes, no Mullvad regions, no account
  switching and no Taildrop simply never grows those rows. With nothing
  installed the panel says what it looked for instead of naming one provider.

  Being installed is not the same as being drivable: a provider the model
  cannot parse is named but never driven, and never offered by the switcher.
  On a machine with both installed, Tailscale still wins the automatic choice,
  so nothing changes until you pick the other one.

- **`pnpm coverage`** runs the suite under Node's coverage reporter. The test
  files are excluded from the report, so the percentages describe
  `shared/model.ts` rather than the tests measuring themselves - and the three
  frontends never execute under Node, so what the parity and distribution
  tests assert about them is not in the number either. CI prints the same
  report on every run; no threshold gates the build.

### Fixed

- **A poll belonged to whichever provider was active when it answered.** All
  providers shared one runner per kind while the parser dispatched on the
  current provider, so switching mid-poll fed one provider's output to the
  other's parser, which reads as a connected daemon with no peers and no
  networks. Each poll now records the provider it was launched for, a reply
  that outlives a switch is dropped, and switching reaps the polls in flight
  so the new provider is asked at once.

- **The watcher was Tailscale's.** `tailgauge-watch` blocks on tailscaled's
  event bus and every service ran it whatever was active, so under another
  provider the panel rode an unrelated daemon's events and refreshed at most
  every 300 seconds. The watch command belongs to the provider now, and one
  without it falls back to the poll timer.

### Changed

- **The bar icon no longer toggles the connection.** It describes the machine
  rather than one provider, so right click on Omarchy and middle click on
  Plasma were turning off a provider you could not see, and leaving the icon
  lit when the other one was still up. Both cycle which provider the panel is
  about now, and do nothing when only one is installed.

- **The model no longer describes itself as Tailscale's.** `Peer.TailscaleIPs`
  and `TailscaleIPv6` are `IPv4` and `IPv6`, `tailnetExitNodes` is
  `ownExitNodes`, and `backendState` is `daemonState`. The daemon's own JSON
  keys are untouched: only what the model produces is renamed. Mullvad and
  Taildrop keep their names, being Tailscale features rather than Tailscale
  spellings of a general idea. On disk, `TailscaleService.qml` is
  `ProviderService.qml`, GNOME's `tailscale.ts` is `provider.ts`, and
  `TailscaleIcon` is `TailGaugeIcon`.

- **A provider's commands live with the provider.** Each service used to spell
  `tailscale` into every command it ran, which is how a panel could name one
  provider and drive another. The argv is now part of the registry, and a
  parity test fails on any service that names a provider binary itself.

- **The toolchain is managed with pnpm.** `pnpm-lock.yaml` replaces
  `package-lock.json` at the same resolved versions, the `packageManager`
  field pins pnpm itself, and CI installs from the frozen lockfile.
  `scripts/build.sh` falls back to `npm install` when pnpm is missing, because
  `scripts/install.sh` runs it from a fresh clone and npm is the one package
  manager an end user is guaranteed to already have. That fallback is
  best-effort: npm cannot read the pnpm lockfile, so it re-resolves the ranges
  in `package.json` rather than reproducing what CI built with.

## [0.3.4]

### Fixed

- **The build refuses to package a GNOME extension whose schemas it cannot
  compile.** The payload ships the settings schema as XML and the compiled blob
  is built on the machine that assembles it, but the build only warned and
  carried on when `glib-compile-schemas` was missing, and `scripts/install.sh`
  installed the result anyway. GNOME reads the extension's own schema source
  the moment `schemas/` exists and so never falls back: the extension then
  failed at every enable with `Failed to open file ".../gschemas.compiled"` and
  sat in error in the Extensions app. The build now stops before anything is
  installed and names the package to install (Debian/Ubuntu:
  `libglib2.0-bin`), and compiles `--strict`, which is what
  `gnome-extensions install` and CI already do.
- **A GNOME install that fails halfway no longer leaves nothing installed.**
  The copy `scripts/install.sh` falls back to when `gnome-extensions` is not on
  `PATH` deleted the installed extension before copying the new one, so a copy
  that ran out of disk left neither. It stages the copy and swaps it in now,
  moving the old one aside rather than deleting it and putting it back if the
  swap does not happen, so the window in which nothing is installed closes on a
  rename rather than on a whole copy.

## [0.3.3]

### Fixed

- **An update applied from the Omarchy panel stopped halfway.** The plugin was
  replaced, the helpers were not, and the banner came straight back offering
  the release that had just been installed. The rescan that reloads the plugin
  also kills the panel process the updater runs under, so it now runs dead
  last, after every part is installed. Helpers are renamed into place rather
  than written over, since one of them is the script doing the writing.

### Added

- **Every helper answers `--version`** with its own name and version, and
  `tailgauge-ctl version` does the same.
- **The panel names the version it is running** in a dimmed line at the
  bottom, with the helpers' version beside it when the two disagree - which is
  what a half-applied update leaves behind, and what nothing on the machine
  used to say out loud.

## [0.3.2]

### Changed

- **Everything that is not QML or shell is now TypeScript**, checked under
  `strict`. No behaviour change: the emitted JavaScript differs from what
  0.3.1 shipped only in formatting, and in `fillPreferencesWindow` becoming
  `async`, which is the signature `ExtensionPreferences` already declared.
  `shared/model.ts` compiles once and ships twice - the GNOME extension
  imports the ES module as emitted, and both QML engines get the same file
  with its trailing `export` removed, since a QML shared script cannot carry
  module syntax and a stray `export` there loads as a blank panel. The build
  refuses to strip anything unless that last line really is the export, and
  CI now checks the two copies that ship rather than the source they came
  from. The model targets ES5 against the ES5 library, because the QML
  engines are the oldest runtime it reaches.
- **The types made three things explicit that were only ever implied.**
  `parseStatus()` returned one bag of optionals and now returns a union of
  its three real shapes, so a caller that has ruled out the failure cases is
  handed a status whose fields are all present. Four GIO async callbacks
  dereferenced a source the bindings type as nullable. The tests dereferenced
  `find()` results directly, and now fail with a useful message instead of a
  `TypeError`. Nothing was broken - the code was already type-consistent -
  but none of it was written down.

## [0.3.1]

### Fixed

- **Typing in a search field reached the panel instead of the field.** The
  Omarchy widget never told the shell's key catcher to stand down while an
  editor had focus, and that catcher takes keys before any focused descendant.
  So `h`, `j`, `k`, `l`, `x` and space were swallowed outright and never
  arrived, while `c`, `n`, `d`, `s`, `t` and `r` fired a row action on the way
  through - copying an address, opening a Taildrop file picker, or toggling
  Tailscale. This is also the whole of "the search is case sensitive": the
  catcher matches `"j"` and not `"J"`, so a query typed in capitals arrived
  intact and the same query typed normally lost letters. The filter always
  lowercased both sides, and a test now pins it.
- **The panel reported a tailnet that never went down.** The watchdog that
  reaps a hung `tailscale status` was armed on launch and never disarmed, so it
  fired on a schedule rather than on a symptom - and at the three-second cadence
  of an open panel, whatever it found in flight was nearly always a healthy
  poll. Killing a Quickshell process still emits `exited`, and the kill's exit
  code was read as an answer, so a command that was never allowed to respond
  became "Disconnected" until the next poll landed. It is disarmed on landing
  now, and a reaped poll reports nothing. The plasmoid was silent about it but
  had been discarding one refresh every fifteen seconds.
- **The search field lost focus, then blinked on every keystroke.** Two causes.
  Both QML services assigned freshly parsed arrays on every poll whether or not
  anything had moved, and a new array is a new panel, which rebuilds every row
  it holds - taking the field with it several times a minute. And `panel` is a
  new object on each keystroke anyway, because the query is one of its inputs,
  so the repeaters rebuilt again per character. The services compare before
  assigning, and the repeaters bind by count and read their rows by index, so a
  delegate survives and re-reads. GNOME already worked this way; the two QML
  frontends were the ones out of line, and the parity tests now hold all three
  to it.

## [0.3.0]

### Added

- **An Omarchy frontend**, `omarchy/arzaroth.tailgauge`: the panel TailGauge
  was ported *from*, rebuilt on the shared model and installed as a third-party
  Quickshell widget. It exists because the port has since moved past its
  origin - the local machine as a row of its own, offline machines, the owner
  of each machine, a machine search, the update banner - and none of that
  reaches an Omarchy bar through upstream's copy. `scripts/install.sh
  --omarchy` builds and enables it; `omarchy plugin disable omarchy.tailscale`
  drops the first-party one it duplicates.
- The widget answers to the shell's IPC (`omarchy-shell arzaroth.tailgauge
  <open|close|toggle|refresh|up|down|toggleTailscale|status>`), which the other
  two frontends have no equivalent for.
- `tailgauge-update` learned a fourth target. The Omarchy plugin has no store
  behind it, so a release archive is the only way it updates, and the panel's
  own update banner installs it in place.
- `.claude/skills/upstream-check`: a maintenance skill for the drift review
  that a vendored port needs and nothing else performs. Three frontends now
  read from one upstream that keeps moving.

## [0.2.0]

### Added

- **The owner of every machine**: `tailscale status` names each peer's owner
  only by `UserID`, with the labels held in a separate `User` map, so the panel
  could say what a machine was and where it lived but never whose it was. The
  two are joined into a `UserName` on every normalized peer, Self included, and
  the machine row carries it. On a tailnet with twenty-five people in it, that
  was the missing half of the row. The display name comes before the login
  name: "Alice Doe" fits where "alice.doe@example.com" would only elide.
- **Searching by owner**: the machines search matches the owner alongside the
  name, the MagicDNS name, either address and the OS.

### Changed

- The machine subtitle spends its second slot on the owner instead of the
  MagicDNS name, which mostly repeated the machine name written above it. On a
  50-machine tailnet that takes the widest row from 65 characters to 51 -
  narrower than before the owner was there at all. Appending it as a third part
  was the other option and a worse one: Plasma's sublabel elides inside a
  fixed-width popup, so the owner would have been the first thing cut, and
  GNOME's rides on one line with no ellipsization, so the menu would simply have
  grown. Nothing is lost: the MagicDNS name stays a click away in the copy menu
  and the search still matches it.
- A daemon old enough to return no `User` map keeps the MagicDNS name in that
  slot rather than leaving a bare address.

## [0.1.0]

### Added

- **This device**: the local machine now has its own section above Connections,
  drawn as a machine row and carrying the same copy actions its peers get -
  name, MagicDNS name, IPv4, IPv6. `parseStatus()` already normalized all of it
  and threw it away; the MagicDNS name reached no frontend at all, and the IP
  only ever showed up in a Plasma tooltip that GNOME had no counterpart for.
- **`tailgauge-ctl`**: `status`, `toggle`, `up`, `down`, `exit-node [NAME|off]`
  and `exit-nodes`, for a key binding or a script. There is still no IPC and no
  need for one: both panels block on `tailscale debug watch-ipn`, so whatever
  the helper changes shows up in them within a second. Run without a terminal
  it opens the login page itself and reports failures as a notification.
- **GNOME toggle shortcut**: a configurable global key in the extension's
  preferences, empty by default so it cannot collide with a binding already in
  use. It toggles in-process, so it works on a store install with no helpers on
  `PATH`. Plasma binds `tailgauge-ctl toggle` as a custom command instead.
- **Offline machines** are listed instead of discarded. `parseStatus()` dropped
  every peer that was not up, which made a sleeping laptop indistinguishable
  from one that had been removed from the tailnet, and put its address out of
  reach of the copy actions that would let you wake it. They sort below the
  online ones and say "Offline" in the subtitle; sending files to one is still
  refused, and an exit node that is down is still not offered as a route.
- **Searching the machines list**: past eight machines the section grows a
  search field, matching the name, the MagicDNS name, either address and the
  OS. It appears only when the list is long enough to need one, and stays once
  it is on screen so it cannot vanish from under what is being typed into it.
  Neither it nor its no-match line is a cursor stop.

### Changed

- The single-letter keys follow what the model put on a row rather than the
  row's kind, so `c` / `n` / `d` work anywhere there are copy options and `s`
  no longer fires on a machine that cannot receive files.

## [0.0.2]

### Fixed

- **GNOME: the panel menu no longer runs off the screen.** The sections that
  grow with the tailnet - update, connections, exit nodes, machines - now sit
  in their own scroll view, sized to 60% of the monitor's work area, with the
  toggle, the status line, Refresh and Settings outside it. The shell keeps a
  tall menu on screen by moving it, never by shrinking it, so a large tailnet
  used to push the footer rows below the bottom of the display. Key focus
  follows the scroll, so arrow keys cannot land on a row you cannot see.

## [0.0.1]

First release, and deliberately numbered as one: the Plasma widget has been
run and driven on a real desktop, but the GNOME extension has never been
executed. It is syntax-checked and shares the resolver, the model and the
helpers with the Plasma side, so the risk sits in its St/PopupMenu rendering
and its Gio.Subprocess plumbing.

A port of the first-party `omarchy.tailscale` panel plugin that ships with
[Omarchy 4 (Quattro)](https://github.com/basecamp/omarchy), moved off
Hyprland/Quickshell onto KDE Plasma 6 and GNOME Shell 45+.

### Added

- **KDE Plasma 6 plasmoid**: panel icon drawn natively as the Tailscale 3×3 dot
  grid, click-to-open popup with the on/off switch, connections, exit nodes and
  machines, and a keyboard cursor over the whole panel.
- **GNOME Shell extension** (45-50): the same panel as native `PopupMenu` rows,
  with machines and the Mullvad picker as submenus, plus an Adwaita preferences
  window.
- **Exit nodes**: every tailnet exit node, a shortlist of the Mullvad regions
  you actually use, and a searchable picker for the full Mullvad fleet.
- **Machines**: online peers with IP and MagicDNS name, copy actions for name /
  DNS name / IPv4 / IPv6, and a Taildrop send action where the tailnet allows
  file sharing.
- **Taildrop**: `tailgauge-send` picks files through the desktop's chooser;
  `tailgauge-receive` runs as a systemd user service, delivers into
  `~/Downloads` and announces each file with a click-to-open notification.
- **Connections**: switch between Tailscale profiles, and a prompt to
  `pkexec tailscale set --operator=$USER` when the daemon refuses profile
  access.
- **Login**: the authorization URL is scraped out of `tailscale up` as it prints
  and opened in the browser.
- **Keyboard**: `t` toggle, `r` refresh, and on a machine row `c` / `n` / `d`
  copy IP / name / DNS name and `s` sends files.

### Shared

- `shared/model.js` is the only copy of the Tailscale data model *and* of the
  panel layout. `resolvePanel()` decides which sections exist, in what order,
  which rows they hold, what every row says, and the cursor's traversal order;
  each frontend decides only how a row looks. `test/parity.test.mjs` enforces
  that boundary.

### Distribution

- Ships through store.kde.org and extensions.gnome.org, which both carry their
  own update mechanism, plus GitHub releases for anyone installing from the
  repo. `tailgauge-update --check/--apply` covers the last case, caches its
  GitHub answer for six hours, and refuses to touch a copy that KNewStuff, EGO
  or a distro package owns.
- The update banner is resolved in `resolvePanel()`, so both panels show it.

### Fixed

- The panel toggle was unclickable most of the time: it bound `enabled` to
  `busy`, which was true whenever any command was in flight, including the
  status poll running every few seconds. Upstream shows `busy` on its switch
  but never enforces it. `busy` now covers only work the user asked for, and
  the toggle is gated on nothing but the CLI being present.
- State changed outside the panel - tsui, the CLI, another desktop - took up to
  a full refresh interval to appear. `tailgauge-watch` blocks on
  `tailscale debug watch-ipn` and returns the moment tailscaled reports a
  change; both panels chain it ahead of a refresh. Polling stays as the floor
  and tightens to 3s while the panel is on screen.
- The installer printed a restart command for `plasma-plasmashell.service`,
  which does not exist when the session starts plasmashell as a transient
  `app-plasmashell@<hash>.service`. It now looks the unit up.
- Taildrop's send action appeared even when `tailgauge-send` was not installed,
  which is every store install: neither the KDE Store nor EGO can put anything
  on `PATH`. The panel now checks for the helper before offering the action.
- `claim_path` in the Taildrop receiver derived the incoming file's name from
  the caller's variable rather than its own argument, because both assignments
  shared one `local` statement. Upstream has the same shape; it is masked there
  by the caller passing an identically named variable.
