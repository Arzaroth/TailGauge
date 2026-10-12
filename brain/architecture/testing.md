# Testing

Almost everything is a cargo test. Five layers, each covering a boundary the
others cannot reach; the first four run in CI ([../operations.md](../operations.md)),
the last is local and opt-in. No coverage tooling or floor exists in this repo.

| Layer | What it proves | Run |
| --- | --- | --- |
| `specification.rs` | what the panel should say, from real captures | `cargo test` |
| `parity.rs` | no frontend decides what the panel already decided | `cargo test` |
| `e2e.rs` + inner tests | the shipped binary, the CLI surface, distribution | `cargo test` |
| `tests/qml`, `tests/gnome` | each frontend's service against a recorded panel | `tests/qml/run.sh`, `node --test` |
| `tests/gnome/shell` | the extension's menu in a real GNOME Shell | `tests/gnome/shell/run.sh` |

## Specification (`crates/tailgauge-core/tests/specification.rs`)

The behaviour of `panel_spec` and the parsers, around 110 tests grouped by
banner comments: parsers, sections, machines and search, header / status /
traversal, banner and footer, provider registry, NetBird, the bar, expanded
rows. It resolves panels from captures in `tests/fixtures/`: `status.json`,
`exit-nodes.txt`, `accounts.json` (Tailscale) and `netbird-status.json`,
`netbird-status-idle.json`, `netbird-networks.txt`, at a fixed `NOW`. When a
CLI's shape changes, update the capture first; a suite passing against a stale
capture is how a parser breaks silently. The file's header still describes it
as a port of `test/model.test.ts` beside a differential harness; both are gone.

## Parity (`crates/tailgauge-core/tests/parity.rs`)

Source-grepping guards over the Plasma, GNOME and Omarchy sources (not the TUI):
no user-visible string written in a frontend, no re-derived section visibility,
exit-node list, copy options, status precedence or search haystack, no provider
binary named in an argv, a GNOME slot for every section. Details and the full
list in [panel-spec.md](panel-spec.md).

## The binary (`crates/tailgauge/tests/e2e.rs` and inner tests)

- `e2e.rs` runs `CARGO_BIN_EXE_tailgauge` on a `Machine`: a temp `HOME` whose
  `bin/` holds fake `tailscale` / `netbird` shell scripts that `cat` the core
  fixtures, with `PATH` set to that directory alone (a real `tailscale` once
  answered instead) and the XDG dirs moved inside it. Covers the gather end to
  end: no CLI, a running tailnet, Mullvad regions, the bar, a dead daemon,
  optimistic state, `the_panel_never_waits_on_github`, `ctl` per provider, and
  `every_command_the_frontends_invoke_exists`, which greps the frontend sources
  for `tailgauge` argv literals and `_ctl(...)` calls and runs each with
  `--help`.
- Inner unit tests: `main.rs` (alias splicing, update-flag exclusivity,
  `the_installer_and_the_updater_write_the_same_aliases`), `send.rs`,
  `file_select.rs`, `receive.rs`, `tui.rs` (pane building, search, cursor,
  provider choice), and the distribution tests below.
- **Distribution tests** in `crates/tailgauge/src/project.rs` read
  `release.yml`, the manifests and `scripts/compat/tailgauge-update` as text:
  the published archive names match selvedge's `arch_target()` +
  `ARCHIVE_SUFFIX`, every packaged asset matches an upload glob, the archive
  has `frontends/<id>` for each payload, the plasmoid ships as a zip, the
  Omarchy tarball unpacks under its manifest id (never `omarchy.*`), EGO's
  `version` is an integer, every payload exists and declares the binary's
  version, aliases and legacy names stay disjoint, the 0.4 bootstrap is still
  published and proves the binary before deleting itself. See
  [distribution.md](distribution.md).

## Frontend harnesses

- **QML** (`tests/qml/run.sh`): runs `plasma.qml` and `omarchy.qml` in a plain
  `qml` runtime (`QT_QPA_PLATFORM=offscreen`), with `tests/qml/stubs/` standing
  in for Plasma's `plasma5support` DataSource and Quickshell's `Process` /
  `FileView` / `IpcHandler`; the stubs record command lines in
  `Harness/Registry.qml` and let the test answer them. `Check.qml` is the
  framework: `equal` / `ok` / `fail`, `load()` of a single file (a directory
  import would drag in unresolvable desktop modules), `run(fixture, body)` via
  XHR, a 10 s watchdog, and `Qt.exit` as the result. Each harness loads the
  service, feeds it `tests/qml/fixtures/panel.json`, and checks what it parsed
  and what it sends back (Plasma: the `sh -c` quoting of the `--ui` JSON).
- **GNOME node tests** (`tests/gnome/*.test.js`): `node --import
  ./tests/gnome/register.js` installs `gi-loader.js`, which answers `gi://Gio`,
  `gi://GLib`, `gi://GObject` with `tests/gnome/mocks/`. They import the
  *built* `panel.js` and `provider.js`, so `scripts/build.sh` runs first;
  `provider.test.js` reads the same `panel.json`. They never load
  `extension.js`.
- `tests/qml/fixtures/panel.json` is a hand-recorded `tailgauge panel --json`
  output (seven sections); nothing regenerates it. A change to the panel JSON
  that the harnesses assert on means re-recording it.

## Real GNOME Shell (`tests/gnome/shell/run.sh`)

The only thing that loads `extension.js`. Builds a Fedora image
(`Containerfile`, Fedora = GNOME - 6, so `--gnome 45` is Fedora 39) with
podman or docker, mounts the built extension, a probe extension
(`probe@tailgauge.test`) and a fake `tailgauge` that `cat`s the panel, and
`boot.sh` starts `gnome-shell --headless --wayland --virtual-monitor` (default
1280x720) under dbusmock'd logind and upower. The probe opens the menu, writes
geometry to `probe.txt` and screenshots top and bottom. Fails when the probe
never finishes, records `error:`, or the shell log shows a JS error from the
extension. Flags: `--live` (serve the installed `tailgauge panel --json`),
`--json FILE`, `--gnome N` (45 or later, default 50), `--monitor WxH`, `--out
DIR`. Exists because GNOME 45 drew every section one pixel tall (fixed in
0.6.1) and no other test could see it. Run at 45 and 50 after any
`extension.ts` change.

## Sources

- `crates/tailgauge-core/tests/specification.rs` (`fixture`, `NOW`), `crates/tailgauge-core/tests/fixtures/`
- `crates/tailgauge-core/tests/parity.rs` (`PLASMA`, `GNOME`, `OMARCHY`, `frontends`)
- `crates/tailgauge/tests/e2e.rs` (`Machine::with`, `Machine::run`, `every_command_the_frontends_invoke_exists`)
- `crates/tailgauge/src/project.rs` (`mod tests`, `published_suffixes`), `crates/tailgauge/src/main.rs` (`mod tests`)
- `tests/qml/run.sh`, `tests/qml/Check.qml`, `tests/qml/plasma.qml`, `tests/qml/omarchy.qml`, `tests/qml/stubs/`, `tests/qml/fixtures/panel.json`
- `tests/gnome/register.js`, `tests/gnome/gi-loader.js`, `tests/gnome/mocks/`, `tests/gnome/panel.test.js`, `tests/gnome/provider.test.js`
- `tests/gnome/shell/run.sh`, `tests/gnome/shell/boot.sh`, `tests/gnome/shell/Containerfile`, `tests/gnome/shell/probe@tailgauge.test/`
