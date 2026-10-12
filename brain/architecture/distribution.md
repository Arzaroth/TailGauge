# Distribution

How TailGauge reaches a machine and stays current: one binary, three desktop
payloads that install apart from it, two stores that can carry only the
payloads, and selvedge doing the fetching and replacing.

## The binary and its names

- One executable, `tailgauge`, in `~/.local/bin` (`scripts/install.sh`) or
  wherever the user put it; `--update` replaces it in place, next to itself.
- One symlink per subcommand, `tailgauge-{ctl,watch,notify,send,receive,file-select,copy,tui}`,
  pointing at `tailgauge`. `main::splice_alias` reads `argv[0]` and inserts the
  name after `tailgauge-` as the subcommand, so key bindings on the old shell
  helpers (`tailgauge-ctl toggle`) keep working with one parser.
- The list is written twice: the `for alias in` loop of `install.sh` and
  `ALIASES` in `crates/tailgauge/src/project.rs` (selvedge rewrites those on
  `--update`). `the_installer_and_the_updater_write_the_same_aliases` (in
  `main.rs`) and `every_alias_names_a_subcommand_the_binary_has` keep them equal
  and real. A new subcommand needs both.
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

## Release assets

`release.yml` publishes six, named `tailgauge-<tag>-<suffix>`:

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

`the_release_publishes_the_archive_this_binary_downloads` compares the
workflow's `dist/tailgauge-$safe-*` names with selvedge's own `arch_target()`,
not with a second copy of the string. The remaining tests are listed in
[testing.md](testing.md).

## Stores

The `.plasmoid` goes to store.kde.org and the extension zip to
extensions.gnome.org by hand after each release (`release` skill, step 7).
Neither store can install the binary or the systemd unit: the KDE Store ships a
kpackage, EGO an extension zip. Every frontend draws what `tailgauge panel`
prints, so a store-only install has a widget and no panel until the binary is
installed from a release archive. Store-installed payloads also update on the
store's schedule, apart from the binary, which is why the panel JSON must stay
readable one release either side and why skew is reported
([../features/updates.md](../features/updates.md)). Omarchy has no store: its
widget arrives through `install.sh`, the tarball, or `--update`.

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
- selvedge `v0.3.2`: `src/frontend.rs` (`ARCHIVE_ROOT`, `dest_dir`, `install_into`, `schemas_ready`), `src/update.rs` (`arch_target`, `ARCHIVE_SUFFIX`, `install_frontends`, `apply`)
- `.github/workflows/release.yml` (Package step), `scripts/install.sh`, `scripts/build.sh`, `scripts/compat/tailgauge-update`
- `NOTICE`, `LICENSE`, `LICENSE-MIT`, `omarchy/arzaroth.tailgauge/NOTICE`
