# Distribution

How TailGauge reaches a machine and stays current: one binary, three desktop
payloads that install apart from it, two stores that can carry only the
payloads, distribution packages that carry all but one, and selvedge doing the
fetching and replacing.

## The binary and its names

- One executable, `tailgauge`, static musl (no glibc requirement), in
  `~/.local/bin` (`scripts/install.sh`), `/usr/bin` (a package) or wherever the
  user put it; `--update` replaces it in place, next to itself, unless a
  package manager owns it.
- One symlink per subcommand, `tailgauge-{ctl,watch,notify,send,receive,file-select,copy,tui}`,
  pointing at `tailgauge`. `main::splice_alias` reads `argv[0]` and inserts the
  name after `tailgauge-` as the subcommand, so key bindings on the old shell
  helpers (`tailgauge-ctl toggle`) keep working with one parser.
- The list is written seven times: `ALIASES` in
  `crates/tailgauge/src/project.rs` (selvedge rewrites those on `--update`), the
  `for alias in` loops of `install.sh`, `package.sh`, `check-package.sh` and
  both PKGBUILDs, and `ALIASES=(...)` in `scripts/compat/tailgauge-update` (the
  bootstrap). `the_installer_the_packages_and_the_updater_write_the_same_aliases`
  (in `main.rs`) and `every_alias_names_a_subcommand_the_binary_has` keep the
  first six equal and real; the bootstrap is held only by a substring check in
  `the_bootstrap_downloads_an_archive_the_release_publishes`. A new subcommand
  needs all of them.
- `tailgauge-update` is a **legacy** name (`TAILGAUGE.legacy`): updating is a
  flag, so the binary answers that name with a usage error. Both `install.sh`
  and selvedge's update delete it. `a_legacy_name_is_not_also_an_alias`.

## The selvedge contract (`project.rs`)

`TAILGAUGE: selvedge::Project` is everything selvedge cannot know: binary
`tailgauge`, repo `Arzaroth/TailGauge` (overridable with `TAILGAUGE_REPO` so a
fork updates from its own releases), `version: env!("CARGO_PKG_VERSION")`
(evaluated here, not inside selvedge), `FRONTENDS`, `ALIASES`, `legacy`, no MSI.
`FRONTENDS` describes each payload:

| id | payload in archive | installs to | version read from | restart |
| --- | --- | --- | --- | --- |
| `plasma` | `frontends/plasma/org.tailgauge.plasmoid` | `$XDG_DATA_HOME/plasma/plasmoids/` | `KPlugin.Version` | cheap: `kquitapp6 plasmashell && kstart plasmashell` |
| `gnome` | `frontends/gnome/tailgauge@arzaroth.github.io` | `$XDG_DATA_HOME/gnome-shell/extensions/` | `version-name` | session: log out, then `gnome-extensions enable` |
| `omarchy` | `frontends/omarchy/arzaroth.tailgauge` | `$XDG_CONFIG_HOME/omarchy/plugins/` | `version` | cheap: `omarchy-shell -q shell rescanPlugins` |

GNOME is `compiled: true` (only the built payload installs; the TypeScript
sources would load as a broken extension) and `gsettings_schemas: true`
(schemas are compiled in staging, and a copy without `gschemas.compiled` is
reported as not ready).

selvedge installs a payload by copying it to a hidden `.<artifact>.tg-new`
beside the destination, compiling schemas there, renaming the live copy aside
to `.<artifact>.tg-old`, renaming the new one in, and deleting the old one only
after that succeeded; a failed rename puts the old copy back. Per-frontend
failures are collected, not fatal. The binary itself is swapped through a temp
`<binary>.old`. One kernel lock (`UpdateLock`) serialises updates.

## `install.sh`

One script, two sources. `checkout()` looks for `Cargo.toml` and
`crates/tailgauge` above the script's own path; `curl ... | bash` has no path
(`BASH_SOURCE` is empty), so it finds none.

- **In a clone**: `build.sh`, `cargo build --release --locked`, and the payloads
  from `build/`. GNOME prefers `gnome-extensions install` on the zip.
- **Piped, or with `--version`**: `fetch_release` resolves the tag (the
  `releases/latest` redirect unless `--version` names one), downloads
  `SHA256SUMS`, picks `tailgauge-<tag>-linux-<arch>.tar.gz` from it, refuses an
  archive whose sum differs, unpacks it to a temp directory and runs its
  `tailgauge --version` before installing anything. Payloads come from the
  archive's `frontends/<id>/<artifact>` and `systemd/`, the same paths selvedge
  reads, so a curl install and `--update` install the same bytes.
  `TAILGAUGE_REPO` picks a fork; `TAILGAUGE_DOWNLOAD_BASE` (a directory or URL)
  stands in for the release, which is how `tests/install.rs` runs it.
- Releases before 0.7.0 publish no `SHA256SUMS` and cannot be installed this
  way.
- The whole script is a `main` called on its last line, so a download cut short
  defines functions and runs nothing.
- `--no-frontend` installs the binary, aliases and unit only: the path for a
  widget that came from a store.

## Release assets

`release.yml` publishes these, named `tailgauge-<tag>-<suffix>` but for the
packages and `SHA256SUMS`:

- `plasmoid.plasmoid`: a zip of the package contents, for `kpackagetool6 -i`
  and the KDE Store.
- `gnome-shell-extension.zip`: `build.sh`'s zip, for `gnome-extensions install`
  and EGO.
- `omarchy-plugin.tar.gz`: the `arzaroth.tailgauge` folder, untarred into
  `~/.config/omarchy/plugins/`.
- `linux-x86_64.tar.gz`, `linux-aarch64.tar.gz`: `tailgauge`, `frontends/{plasma,gnome,omarchy}/<artifact>`,
  `systemd/tailgauge-receive.service`, and `LICENSE`, `LICENSE-MIT`,
  `LICENSE-WTFPL`, `NOTICE`. selvedge picks the asset containing
  `arch_target()` (`linux-x86_64` / `linux-aarch64`) and ending in
  `ARCHIVE_SUFFIX` (`.tar.gz`); this is what `--update` and
  `--install-frontend` download.
- `helpers.tar.gz`: `bin/tailgauge-update`, the 0.4 bootstrap (below).
- `tailgauge_<x.y.z>_{amd64,arm64}.deb`, `tailgauge-<x.y.z>-1.{x86_64,aarch64}.rpm`,
  `tailgauge-bin-<x.y.z>-1-{x86_64,aarch64}.pkg.tar.zst` and `pkgbuild.tar.gz`
  (below).
- `SHA256SUMS`: `sha256sum` of every asset above, taken in the `checksums` job
  once every package is built and signed
  (`the_release_publishes_the_checksums_of_every_asset`). With signing on, also
  a `.sig` per Arch package, `SHA256SUMS.asc` and `tailgauge-release-key.asc`.

Jobs: `binary` (per arch, static musl, refuses a `NEEDED` entry) -> `assets`
(frontends, tests, archives, `.deb`/`.rpm`) -> `arch` (archlinux container) ->
`checksums` (signing, `SHA256SUMS`) -> `verify` (per arch) -> `publish`. On a
pull request touching the release, the packaging or the installer, everything
but `publish` runs, with the tag taken from the workspace version.

`the_release_publishes_the_archive_this_binary_downloads` compares the
workflow's `dist/tailgauge-$safe-*` names with selvedge's own `arch_target()`,
not with a second copy of the string. The remaining tests are listed in
[testing.md](testing.md).

## Packages

Built from the release archive, never beside it, so a package and `--update`
install the same bytes (`the_packages_are_built_from_the_release_archive`).

| Installs | to |
| --- | --- |
| `tailgauge`, one symlink per alias | `/usr/bin/` |
| `tailgauge-receive.service`, `ExecStart` rewritten to `/usr/bin/tailgauge` | `/usr/lib/systemd/user/` (not enabled) |
| the plasmoid | `/usr/share/plasma/plasmoids/org.tailgauge.plasmoid` |
| the extension, schemas compiled | `/usr/share/gnome-shell/extensions/tailgauge@arzaroth.github.io` |
| licences, `NOTICE`, docs | `/usr/share/doc/tailgauge/`, `/usr/share/licenses/<pkgname>/` |

- `.deb` / `.rpm`: `scripts/package.sh <archive> <dist>` unpacks the archive,
  rewrites the unit, appends an nfpm symlink entry per alias to a copy of
  `packaging/nfpm.yaml`, and runs nfpm. Payload directories are `type: tree`
  (a plain directory source flattens them). No hard dependencies; recommends
  `ca-certificates`, suggests the VPN CLIs, `zenity`, `wl-clipboard`.
- Arch: `packaging/arch/tailgauge-bin` repackages the archives (built for both
  arches on one host, nothing in it being compiled);
  `packaging/arch/tailgauge` builds from the tag's source tarball (cargo,
  pnpm, `scripts/build.sh`, `cargo test -p tailgauge-core` in `check()`).
  `scripts/arch.sh` renders `pkgver` and the sums, writes `.SRCINFO`, and
  builds. Only the `-bin` packages ship; both PKGBUILDs ship in
  `pkgbuild.tar.gz`. Neither is on the AUR.
- **No Omarchy widget**: the shell loads third-party plugins from
  `~/.config/omarchy/plugins/` only. `tailgauge --install-frontend omarchy`
  puts it there from the binary's own release.
- `--update` refuses a packaged binary (selvedge `update::owner` finds it
  under `/usr`); the update banner says to upgrade through the package manager
  instead ([../features/updates.md](../features/updates.md)).
- `verify` installs each package on Debian 12, Ubuntu 22.04, Fedora and Arch
  (Arch on x86_64 only) and runs `scripts/check-package.sh`: version, every
  alias, the unit's `ExecStart`, both payloads with their subdirectories, and
  for `.deb`/`.rpm` that removal leaves no plasmoid directory behind.
- Signing (smith's scheme): off until `packaging/tailgauge.asc` is committed;
  then a tag release needs `TAILGAUGE_GPG_KEY` and `TAILGAUGE_GPG_PASSPHRASE`,
  signs the `.rpm`s, the Arch packages and `SHA256SUMS`, and `verify` checks
  each signature. A pull request never sees the secret.

## Stores

The `.plasmoid` goes to store.kde.org and the extension zip to
extensions.gnome.org by hand after each release (`release` skill, step 7).
Neither store can install the binary or the systemd unit: the KDE Store ships a
kpackage, EGO an extension zip. Every frontend draws what `tailgauge panel`
prints, so a store-only install has a widget and no panel until the binary is
installed (`install.sh --no-frontend`). Store-installed payloads also update on the
store's schedule, apart from the binary, which is why the panel JSON must stay
readable one release either side and why skew is reported
([../features/updates.md](../features/updates.md)). Omarchy has no store: its
widget arrives through `install.sh`, the tarball, `--install-frontend` or
`--update`.

## `--install-frontend`

`tailgauge --install-frontend plasma|gnome|omarchy|all` (case-insensitive)
downloads the release matching the running binary's own version
(`update::install_frontends`) and installs the named payloads whether or not
they are present: the "switched desktops" path. Never the latest release, so a
frontend cannot land out of step with the binary. Exits non-zero naming any
that failed.

## Upgrade from 0.4.0

0.4.0 and earlier were shell helpers updated by their own `tailgauge-update`,
which downloads `helpers.tar.gz` and installs its `bin/`. 0.5.0 stopped
publishing it and those installs upgraded their widgets, 404'd, and were left
calling a binary nobody had installed. 0.5.1 restored the asset with
`scripts/compat/tailgauge-update` inside; details in
[../operations.md](../operations.md).

## Licence and lineage

`MIT OR WTFPL` (`LICENSE`, `LICENSE-MIT`, `LICENSE-WTFPL`, `Cargo.toml`). TailGauge
began as a port of Omarchy 4's first-party `omarchy.tailscale` plugin
(basecamp/omarchy, MIT, David Heinemeier Hansson). No upstream file survives
the Rust rewrite; `NOTICE` records the behavioural lineage and why the MIT
notice travels in `LICENSE-MIT`. `omarchy/arzaroth.tailgauge/NOTICE` repeats it
inside the plugin, which is third-party and never `omarchy.*`. The release
archives copy all four licence files because an archive is a copy.

## Sources

- `crates/tailgauge/src/project.rs` (`TAILGAUGE`, `ALIASES`, `FRONTENDS`, `mod tests`)
- `crates/tailgauge/src/main.rs` (`splice_alias`, `handle_install_frontend`, `report_frontends`)
- selvedge `v0.8.0`: `src/frontend.rs` (`ARCHIVE_ROOT`, `dest_dir`, `install_into`, `schemas_ready`), `src/update.rs` (`arch_target`, `ARCHIVE_SUFFIX`, `install_frontends`, `apply`, `owner`, `Owner`)
- `.github/workflows/release.yml` (Package step), `scripts/install.sh` (`checkout`, `fetch_release`, `main`), `scripts/build.sh`, `scripts/compat/tailgauge-update`
- `crates/tailgauge/tests/install.rs`
- `packaging/nfpm.yaml`, `packaging/arch/tailgauge/PKGBUILD`, `packaging/arch/tailgauge-bin/PKGBUILD`, `scripts/package.sh`, `scripts/arch.sh`, `scripts/check-package.sh`
- `.github/workflows/release.yml` (`binary`, `arch`, `checksums`, `verify`, `publish`)
- `NOTICE`, `LICENSE`, `LICENSE-MIT`, `omarchy/arzaroth.tailgauge/NOTICE`
