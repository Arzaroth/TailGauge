# Stack

The technologies and versions TailGauge is built on. Bump these when a
`Cargo.toml`, `package.json`, `pnpm-lock.yaml` or a frontend manifest changes.

Sources of truth: `Cargo.toml` (workspace), `crates/*/Cargo.toml`, `Cargo.lock`,
`package.json`, `pnpm-lock.yaml`, `tsconfig.base.json`, `tsconfig.gnome.json`,
`gnome/*/metadata.json`, `plasma/*/metadata.json`, `omarchy/*/manifest.json`,
`.github/workflows/ci.yml`.

## Language and toolchain

- **Rust**, edition 2024, `rust-version = "1.89"` (MSRV, because selvedge locks
  with `File::try_lock`). No `rust-toolchain.toml` and no toolchain action: CI
  and the release build use the runner's preinstalled stable, always with
  `--locked`. Workspace `resolver = "2"`, one version for every crate
  (`[workspace.package] version`), licence `MIT OR WTFPL`, which
  `package.json` states too
  (`the_node_manifest_states_the_licence_the_crates_do` in `project.rs`).
- **Two crates**:
  - `tailgauge-core`: the provider registry, the CLI parsers and `panel_spec`.
    Depends on `chrono`, `serde`, `serde_json` and nothing else; it shells out to
    nothing. See [architecture/panel-spec.md](architecture/panel-spec.md).
  - `tailgauge`: the one binary (`[[bin]] name = "tailgauge"`, `src/main.rs`)
    every frontend spawns. See [architecture/binary.md](architecture/binary.md).
- **TypeScript** `~5.7.2` (lock: 5.7.3), `strict`, typed against
  `@girs/gnome-shell` pinned to `50.0.4`, the newest shell the extension lists.
  The extension still declares `shell-version` 45 to 50, so an API newer than 45
  typechecks and then fails on an old shell. Only the GNOME extension is
  compiled: `tsc -p tsconfig.gnome.json` (target ES2022, ESNext modules, output
  `build/.ts/gnome`), driven by `scripts/build.sh`.
- **QML** (Qt 6) for the Plasma 6 applet (`X-Plasma-API-Minimum-Version` 6.0)
  and the Omarchy (Quickshell) bar widget. Linted in CI by `qmllint` from
  `qt6-declarative-dev-tools`, where only a `[syntax]` diagnostic fails, and
  executed by the `qml` runtime in the QML harness
  ([architecture/testing.md](architecture/testing.md)).
- **pnpm** pinned by `packageManager: "pnpm@11.21.0"` plus `pnpm-lock.yaml`;
  **Node 22** in CI (`@types/node ^22`). Node exists for the extension's
  compiler and its tests and nothing else. `scripts/build.sh` falls back to
  `npm install --no-package-lock` when pnpm is absent, which re-resolves the
  ranges and may pick a different TypeScript patch.
- **glib** tools: `glib-compile-schemas` is a build requirement, not a test one
  (`scripts/build.sh` refuses to package the extension without it), and a
  runtime one for `--update` and `--install-frontend gnome`, where selvedge
  compiles the schemas in staging (`gsettings_schemas: true` in `project.rs`).

## Binary dependencies

- `clap` 4.5 with `derive` (lock: 4.6.7): the subcommands and the update flags.
- `anyhow`, `serde`, `serde_json`.
- `ratatui` 0.29 with `crossterm` 0.28: `tailgauge tui` only
  ([features/tui.md](features/tui.md)).
- `selvedge` from `https://github.com/Arzaroth/selvedge`, pinned by **tag**
  (`v0.3.2`, lock commit `9e4b8bd`), default features on, which means
  `self-update`: `self_update` 0.44 with `reqwest` + `rustls`, tar/flate2 and
  zip. It is the updater, the frontend installer, the update cache and the
  `proc` helpers. Shared with TokenGauge. See
  [architecture/distribution.md](architecture/distribution.md).
- No async runtime. HTTP happens only inside selvedge, on `--update`,
  `--check-update` and `--install-frontend`.

## Working on selvedge locally

Copy `.cargo/config.toml.example` to `.cargo/config.toml` (gitignored). It
`[patch]`es the selvedge git URL to `../selvedge`, but the patch only takes
effect when the local checkout's version matches what `Cargo.lock` resolves.
Otherwise cargo warns "patch ... was not used" and adds a `[[patch.unused]]`
entry to `Cargo.lock`. Making it apply takes `cargo update -p selvedge`, which
rewrites the lock's selvedge entry to the local path: never commit that lock.
Moving the pin is a `Cargo.toml` tag edit plus `cargo update -p selvedge` with
no `.cargo/config.toml` in place.

## External programs (runtime, never linked)

- `tailscale` and/or `netbird`: every status, action and Taildrop transfer goes
  through their CLIs ([architecture/providers.md](architecture/providers.md)).
- `notify-send`, `xdg-open`, `zenity` or `kdialog`, a clipboard tool:
  Taildrop and notifications ([features/taildrop.md](features/taildrop.md)).
- `kpackagetool6`, `gnome-extensions`, `omarchy-plugin-validate` /
  `omarchy-plugin-enable`: only `scripts/install.sh` calls these.
- `glib-compile-schemas`: `--update` and `--install-frontend gnome` (through
  selvedge).

## Packaging

GitHub Actions only: `ci.yml` on pull requests and pushes to `master`,
`release.yml` on `v*` tags (or `workflow_dispatch` with an existing tag). Linux
only, x86_64 and aarch64 on native runners. Six release assets; no Windows or
macOS build. See [operations.md](operations.md).

## Sources

- `Cargo.toml` (`[workspace.package]`, `[workspace.dependencies]`), `Cargo.lock` (`selvedge`, `self_update`, `ratatui`, `clap`)
- `crates/tailgauge/src/project.rs` (`gsettings_schemas`, `the_node_manifest_states_the_licence_the_crates_do`)
- `crates/tailgauge-core/Cargo.toml`, `crates/tailgauge/Cargo.toml`
- `.cargo/config.toml.example`, `.gitignore`
- `package.json` (`packageManager`, `scripts`, `devDependencies`), `pnpm-lock.yaml`, `tsconfig.base.json`, `tsconfig.gnome.json`
- `gnome/tailgauge@arzaroth.github.io/metadata.json` (`shell-version`), `plasma/org.tailgauge.plasmoid/metadata.json`
- `scripts/build.sh`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`
