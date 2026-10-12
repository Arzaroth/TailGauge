# Updates

Knowing a newer release exists, installing it, and noticing when the binary
and a widget have drifted apart. Three flags on the binary, one banner on every
panel. The flags are mutually exclusive and refuse a subcommand
(`main::resolve`, tested by `the_update_flags_do_one_thing_each`).

## `--check-update [--force]`

- `selvedge::update::check_cached` against
  `$XDG_CACHE_HOME/tailgauge/update.json` (`state::update_cache_file`): within
  `CACHE_TTL_MS` (6 h) of the last check it serves the cache, otherwise it asks
  GitHub's latest release. `--force` skips the cache.
- Prints the `UpdateStatus` as JSON: `current`, `latest`, `available`,
  `checked_ms`, `notified`. Written through a temp file and renamed, so a panel
  never reads half of it.
- The panels poll it every 6 h from start (`Timer` in Plasma's
  `ProviderService.qml` and Omarchy's `Service.qml`, `_armUpdateCheck` in GNOME's
  `provider.ts`), mostly reading the file. After an apply they re-check with
  `--force`.

## The banner

- `tailgauge panel` never waits on GitHub: `gather::update_info` reads the cache
  only (`the_panel_never_waits_on_github` in `e2e.rs`). A cold cache means no
  banner.
- `panel.rs` `update_section`: section `update`, first in the panel, untitled,
  visible only when `available`. One row, kind and action `update`, label
  `TailGauge <latest> is available`, sublabel `Install it now`, `busy` while the
  frontend says it is `updating`. Always offered: no store owns any part of
  TailGauge, so there is never a copy the binary may not replace.
- Plasma, GNOME and Omarchy dispatch `update` to `applyUpdate`, which runs
  `tailgauge --update`, shows `Updating TailGauge…`, and puts stderr in
  `lastError` on failure. The TUI runs `selvedge::update::apply` in-process from
  its worker ([tui.md](tui.md)).
- Tests: `the_update_section_is_hidden_until_there_is_an_update`,
  `an_available_update_offers_to_install_itself`,
  `an_update_in_flight_marks_the_row_busy` (`specification.rs`);
  `the update check is cached unless forced...` and `applying an update says
  so...` (`tests/gnome/provider.test.js`).

## `--update`

`selvedge::update::apply`: downloads the `linux-<arch>.tar.gz` of the latest
release, swaps the binary next to itself, rewrites the alias symlinks, removes
legacy names, and reinstalls whichever frontends are already present from the
same archive. It prints `Current version`, then either `Updated to x.` plus one
line per frontend (version read back off disk, or `NOT updated - why`) and its
restart hint, marked `required` when it needs a new session (GNOME) and `to
load it` otherwise. Mechanics in
[../architecture/distribution.md](../architecture/distribution.md).

## Version skew

The binary and the widgets install separately (stores, a half-applied update),
so the skew is reported in two places:

- **Footer**: `panel_footer` is `TailGauge v<widget>`, plus ` · binary
  v<current>` when the cached `current` differs from the version the frontend
  passed in `--ui`. Tests:
  `a_binary_left_behind_by_a_half_applied_update_shows_next_to_the_widget`,
  `a_binary_on_the_widget_version_is_not_worth_a_second_number`.
- **`--update` when already current**: `report_frontend_skew` names each
  installed frontend whose version differs or is unreadable (`tailgauge
  --install-frontend <id>`), and a GNOME install with no compiled schemas.

## `--install-frontend NAME|all`

Installs payloads from the release matching the binary's own version, for a
desktop switched to after install. See
[../architecture/distribution.md](../architecture/distribution.md).

## Sources

- `crates/tailgauge/src/main.rs` (`resolve`, `handle_check_update`, `handle_update`, `handle_install_frontend`, `report_frontends`, `report_frontend_skew`)
- `crates/tailgauge/src/gather.rs` (`update_info`), `crates/tailgauge-core/src/panel_state.rs` (`UpdateInfo`)
- `crates/tailgauge-core/src/panel.rs` (`update_section`, `panel_footer`, `binary_version`)
- selvedge `v0.3.2` `src/update.rs` (`check_cached`, `CACHE_TTL_MS`, `apply`), `src/state.rs` (`UpdateStatus`, `update_cache_file`)
- `plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml`, `gnome/tailgauge@arzaroth.github.io/provider.ts`, `omarchy/arzaroth.tailgauge/Service.qml` (`checkUpdate`, `applyUpdate`)
