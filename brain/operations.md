# Operations

How TailGauge is built, tested, installed, released and debugged. There is no
task runner: cargo, pnpm and the scripts in `scripts/` are the interface.

## The gate

"The gate" is every step of `.github/workflows/ci.yml`, in its order. The
`release` skill keeps a runnable local copy (section 4 of
`.claude/skills/release/SKILL.md`) and `feature-treatment` runs that copy before
a PR. Run it in bash with `shopt -s globstar nullglob`.

| CI job | Steps |
| --- | --- |
| `binary` (x86_64 on `ubuntu-latest`, aarch64 on `ubuntu-24.04-arm`) | `cargo fmt --check`; `cargo clippy --all-targets --locked -- -D warnings`; `cargo test --locked` |
| `frontends` | `pnpm install --frozen-lockfile`; `scripts/build.sh`; `qmllint` over `build/org.tailgauge.plasmoid/contents/ui/*.qml build/arzaroth.tailgauge/*.qml` (a `[syntax]` line fails, the exit code does not); `pnpm typecheck`; `tests/qml/run.sh`; `node --import ./tests/gnome/register.js --test 'tests/gnome/**/*.test.js'`; `node --input-type=module --check` per built extension `.js`; JSON parse of every manifest; `glib-compile-schemas --strict --dry-run` on the gschema and an XML parse of Plasma's `main.xml`; the four versions agree (plasmoid `KPlugin.Version`, GNOME `version-name`, Omarchy `version`, `Cargo.toml`) |
| `helpers` | `bash -n`, `shellcheck --severity=warning` and an executable-bit check on `scripts/*.sh scripts/compat/*`; `systemd-analyze verify --user` on the Taildrop unit (only a parse error fails) |

- The QML lint, the GNOME extension check, the manifest check and the shell
  syntax check fail on an empty glob ("the glob has gone stale"). The helpers
  job's ShellCheck and "scripts are executable" steps have no such guard: under
  `nullglob` an empty match passes them silently.
- Not in CI, run by hand: `tests/gnome/shell/run.sh --gnome 45` and
  `--gnome 50` whenever `extension.ts` changed, since nothing else loads it
  ([architecture/testing.md](architecture/testing.md)).
- `qmllint`, `qml` and `shellcheck` are often missing locally; say which step
  was skipped rather than implying it passed.

## Build and install

- `scripts/build.sh` (also `pnpm build`): installs the TypeScript toolchain if
  `node_modules/.bin/tsc` is missing, compiles the extension, then assembles
  `build/org.tailgauge.plasmoid`, `build/tailgauge@arzaroth.github.io` (`.ts`
  removed, schemas compiled, plus a `.shell-extension.zip` when `zip` exists)
  and `build/arzaroth.tailgauge` (copied, never symlinked: Omarchy's plugin
  registry refuses symlinks).
- `scripts/install.sh [--plasma] [--gnome] [--omarchy] [--no-taildrop]
  [--placement=left|center|right]`: with no target flag it picks from
  `XDG_CURRENT_DESKTOP`. It runs `build.sh`, `cargo build --release --locked`,
  installs `~/.local/bin/tailgauge`, removes a stale `tailgauge-update`, writes
  the `tailgauge-<alias>` symlinks, then the Taildrop unit (unless
  `--no-taildrop`), then each desktop payload: Plasma through `kpackagetool6`
  (`-u` when installed, `-i` otherwise); GNOME through `gnome-extensions install
  --force` on the zip, falling back to a stage-and-swap that moves the old copy
  aside so a failed copy keeps it; Omarchy copied to a staging directory, then
  the old plugin `rm -rf`'d and the new one `mv`'d in. `--placement` is Omarchy
  only.
- Taildrop unit: `systemd/tailgauge-receive.service`, a user unit
  (`ExecStart=%h/.local/bin/tailgauge receive`, `Restart=always`,
  `WantedBy=graphical-session.target`). Installed and enabled by
  `install.sh`, refreshed by the 0.4 bootstrap, shipped in each binary archive
  under `systemd/`. See [features/taildrop.md](features/taildrop.md).
- From a release, `tailgauge --update` and `--install-frontend` replace
  `install.sh` ([features/updates.md](features/updates.md),
  [architecture/distribution.md](architecture/distribution.md)).

## Upgrading from 0.4.0 or earlier

Those installs update by running their own `tailgauge-update`, which fetches a
`helpers` asset. The release still publishes one, carrying
`scripts/compat/tailgauge-update`: a bootstrap that works out the architecture,
downloads `tailgauge-$tag-linux-$arch.tar.gz`, proves the binary runs
(`--version`) before renaming it into place, links the aliases, refreshes the
unit, and only then deletes itself. It also answers `--json` with
`available: true` so a 0.4 panel offers the migration. Users run
`tailgauge-update` twice. Guarded by
`the_release_still_publishes_the_helpers_asset_older_installs_ask_for` and
`the_bootstrap_downloads_an_archive_the_release_publishes` in
`crates/tailgauge/src/project.rs`.

## Release

The `release` skill is the procedure. Semver, pre-1.0: minor for a new way to
use the thing, patch for fix-only. Steps: docs sweep (`CHANGELOG.md`
`[Unreleased]`, `README.md`, `omarchy/arzaroth.tailgauge/README.md`, committed
first as `[master] docs: ...`), bump, gate, `[master] chore(release): x.y.z`,
annotated `vx.y.z` tag, push, watch `release.yml`. A pushed tag is never moved:
a failure inside the tagged tree costs a patch release; one outside it re-runs
with `gh workflow run release.yml -f tag=vx.y.z`.

Every place the version lives:

| File | Field |
| --- | --- |
| `Cargo.toml` | `[workspace.package] version` |
| `Cargo.lock` | follows, via `cargo check --workspace` |
| `plasma/org.tailgauge.plasmoid/metadata.json` | `KPlugin.Version` |
| `gnome/tailgauge@arzaroth.github.io/metadata.json` | `version-name`, and the integer `version` +1 (EGO orders uploads by it) |
| `omarchy/arzaroth.tailgauge/manifest.json` | `version` |
| `README.md` | the `tailgauge --update` sample output |
| `.github/workflows/release.yml` | the `workflow_dispatch` tag example |

Then `git grep -n -F '<old>' -- ':!CHANGELOG.md'` must come back empty.
`release.yml` rebuilds and retests, refuses a tag that disagrees with any
manifest or with `tailgauge --version`, packages six assets and takes the
release notes from the `## [x.y.z]` CHANGELOG section (house format: no date).
The KDE Store and EGO uploads are manual and come after.

Feature branches go through `feature-treatment`: rebase, parallel finder
review, fixes as new layers, docs, gate, PR worked with `babysit-pr` to a
**rebase merge**, release, `wt remove`.

## Upstream watching

The `upstream-check` skill reviews basecamp/omarchy (the plugin TailGauge was
ported from, and the `shell/Ui` components the Omarchy widget imports) and
tailscale/tailscale (the CLI output the parsers read), against
`.claude/skills/upstream-check/BASELINES.md`, which each run rewrites (last:
2026-09-14, omarchy `b679363b`, Tailscale CLI 1.102.4). It points at the Rust
parsers in `crates/tailgauge-core/src/` (`status`, `accounts`, `exit_nodes`,
`peer`), the captures in `crates/tailgauge-core/tests/fixtures/`, and Taildrop
in `crates/tailgauge/src/send.rs` / `receive.rs`.

## Logs

| Where | How to read it |
| --- | --- |
| Plasma | `journalctl --user --identifier=plasmashell -f`. `console.log` is filtered out; probe with `console.warn` |
| GNOME | `journalctl --user -u org.gnome.Shell@wayland -f` |
| Omarchy | `qs log --pid "$(pgrep -f 'quickshell -n -p /usr/share/omarchy/shell')" -t 50` |
| Taildrop receiver | `journalctl --user -u tailgauge-receive` |
| The binary | stderr, prefixed `tailgauge:`; panels surface it as `lastError` |

Reloading: Plasma needs its shell restarted (`install.sh` prints the
`app-plasmashell@*.service` unit), GNOME on Wayland needs a new session,
Omarchy needs `omarchy-restart-shell` for QML edits.

## Sources

- `.github/workflows/ci.yml` (jobs `binary`, `frontends`, `helpers`), `.github/workflows/release.yml`
- `scripts/build.sh`, `scripts/install.sh`, `scripts/compat/tailgauge-update`, `systemd/tailgauge-receive.service`
- `.claude/skills/release/SKILL.md`, `.claude/skills/feature-treatment/SKILL.md`, `.claude/skills/upstream-check/SKILL.md`, `.claude/skills/upstream-check/BASELINES.md`
- `crates/tailgauge/src/project.rs` (distribution tests), `README.md` (Development)
