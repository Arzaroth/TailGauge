# Frontends

The three desktop surfaces, plus the TUI. Everything a user reads comes from
[panel-spec.md](panel-spec.md); a frontend owns chrome, the search substring
test, and the state only it knows. A user-facing feature lands on all three
desktops (and the TUI) or it is not done, and `parity.rs` fails the build when a
desktop starts deciding something the panel already decided.

| Surface | Where | Toolkit | Calls the binary through |
| --- | --- | --- | --- |
| Plasma | `plasma/org.tailgauge.plasmoid` | QML, Plasma 6 (`X-Plasma-API-Minimum-Version` 6.0) | `Plasma5Support.DataSource`, `executable` engine, `sh -c` command lines |
| GNOME | `gnome/tailgauge@arzaroth.github.io` | TypeScript compiled to GJS, `shell-version` 45 to 50 | `Gio.Subprocess` on `sh -c '<PATH>; exec "$@"' sh argv...` |
| Omarchy | `omarchy/arzaroth.tailgauge` | Quickshell QML plugin (`kinds: ["bar-widget"]`) | `Quickshell.Io.Process` argv, `Quickshell.execDetached` |
| TUI | `crates/tailgauge/src/tui.rs` | ratatui + crossterm | in-process `gather::panel_state` + `panel_spec` |

## What every desktop does the same way

Each has one service file that owns the subprocesses (`ProviderService.qml`,
`provider.ts`, `Service.qml`) and a view that draws (`FullRep.qml` +
`PanelRowView.qml`, `extension.ts`, `Panel.qml`). The service:

- **Asks.** `refresh()` runs `tailgauge panel --json --ui <json>` where the JSON
  is `_ui()`: shown provider, `busy`, `updating`, `version`, `actionStatus`,
  `lastError`, the in-flight ids, `active` only while an optimistic toggle is
  pending, merged with the view's `ui` (`recentRegions`, `mullvadPickerOpen`,
  `expandedPeerId`, `phraseIndex`). Assigning `ui` re-asks at once. The answer
  is push-assigned to `panel`; output without a `header` is ignored and the last
  good panel stays.
- **Acts** only through the binary: `_ctl(kind, action, extra)` builds
  `tailgauge ctl [--provider <id>] <action>` (`up`, `down`, `switch-account`,
  `exit-node --peer <payload>`, `select-network <id> [--leave]`, `authorize`);
  copy is `tailgauge copy <text>`, send is `tailgauge send --peer <payload>`,
  URLs go to `xdg-open`, updates are `tailgauge --check-update [--force]` and
  `tailgauge --update`. One command per kind runs at a time.
- **Shows a click on the same frame.** `_showOptimistically` patches only the
  header (`toggleChecked`, `dimmed`, `crossed`), shallowly so `sections` stays
  the same array and nothing rebuilds; `_desired` (-1, 0, 1) is sent as `active`
  until the binary's header agrees.
- **Keeps fresh** with the three clocks in [overview.md](overview.md): `tailgauge
  watch 300` re-armed after 250 ms or backed off 30 s, a poll at the configured
  interval (3 s while `attentive`, i.e. open), a 600 ms settle refresh after a
  command, and `--check-update` every six hours.
- **Persists** the shown provider and the recent Mullvad regions (five, chosen
  one first) in the desktop's own settings.
- **Switching provider** blanks the panel to the empty one before re-asking, so
  one provider's machines are never listed under the other's name.
- **Reads its own version** from the manifest it shipped with and sends it, so
  the footer can show widget and binary skew.
- **Keys**, resolved against the focused row: `t` toggle, `r` refresh, `c` /
  `n` / `d` copy IP / name / DNS on a row with copy options, `s` send on a row
  carrying the `send` action. Plasma and Omarchy also move a cursor through
  `panel.navigation` with `j` / `k` and the arrows; GNOME leaves arrows and Enter
  to `PopupMenu`. See [../features/keyboard.md](../features/keyboard.md).

## The bar icon

It draws `panel.bar` (connected, crossed, warning badge) and its tooltip is
`bar.tooltip`, so it describes the machine, not the provider shown.

| | Left | Middle | Right |
| --- | --- | --- | --- |
| Plasma (`CompactRep.qml`) | open / close the popup | cycle provider (`root.cycleProvider`) | Plasma's applet menu: `toggleHint` entry and Refresh (`Plasmoid.contextualActions`) |
| GNOME (`extension.ts`) | open the menu (`PanelMenu.Button`) | `toggleTailscale()` on the active provider | opens the menu |
| Omarchy (`Panel.qml` `BarIconButton`) | open / close the panel | refresh | cycle provider |

Cycling walks the rows of the `providers` section and does nothing with fewer
than two. GNOME's middle click is the one place a bar click still turns a
connection on or off; it has done so since the first GNOME port, while Plasma
and Omarchy stopped in 0.4.0.

## Plasma

- `main.qml` (`PlasmoidItem`) wires `ProviderService` to
  `Plasmoid.configuration` (`activeProvider`, `refreshIntervalSec`,
  `recentMullvadRegions`, `showStatusInPanel` in `contents/config/main.xml`),
  sets `attentive: root.expanded`, `version: Plasmoid.metaData.version`, and
  builds the tooltip: `bar.tooltip`, then `lastError`, then `Exit node: %1`
  from the current `exitNode` row. `ConfigGeneral.qml` exposes the refresh
  interval (5 to 3600) and showing the machine name next to the icon.
- `ProviderService.qml` imports no plasmoid module, so `tests/qml/plasma.qml`
  can load it with `Plasma5Support` stubbed. The executable engine takes a
  command line with no stdin, so every argv goes out shell-quoted
  (`_quote`, `_command`), and each run gets a `# tailgauge-<seq>` suffix because
  the engine keys a source by its command line. `busy` counts only user kinds
  (`action`, `switch`, `exitNode`, `network`, `operator`, `applyUpdate`).
- `FullRep.qml` draws sections and rows through counted Repeaters
  (`model: full.panel.sections.length`) so a new panel object does not destroy
  the delegate holding a search field; `PanelRowView.qml` keeps row
  registration alive with `syncRegistration()`. Copy options open a popup menu.
- Global shortcut: none in the widget; bind `tailgauge-ctl toggle` as a custom
  command in System Settings.
- Logs: `journalctl --user --identifier=plasmashell -f`; `console.log` maps to
  `qDebug` and is filtered, so probe with `console.warn`.

## GNOME

- `panel.ts` is the hand-written contract (`Panel`, `PanelRow`, `Ui`,
  `Payload`), plus `SECTION_IDS`, `emptyPanel()` (status `Checking…`),
  `panelHasRow`, `mullvadRegionKey`, `pushRecentMullvad`. It is the one frontend
  where the panel shape is type-checked.
- `provider.ts` `ProviderService` is a `GObject` emitting `changed`; the clocks
  are `ATTENTIVE_INTERVAL_SEC` 3, `DELAYED_REFRESH_MS` 600, `ACTION_STATUS_MS`
  2200 (clears the progress line), `WATCH_TIMEOUT_SEC` 300, `WATCH_REARM_MS`
  250, `WATCH_BACKOFF_MS` 30000, `UPDATE_CHECK_MS` six hours.
- `extension.ts` `TailGaugeIndicator` (`PanelMenu.Button`): a
  `PopupSwitchMenuItem` header, a status line, one separator plus section per
  `SECTION_IDS` inside a scroll view sized to the work area, then Refresh
  (labelled from `header.actions`), Settings and the footer. Rows with
  children, copy options or actions become submenus; machines and the Mullvad
  picker are submenus. The menu rebuilds only when `_signatureOf(panel)`
  changes; typing filters by hiding rows (`_applySearch`), never by
  rebuilding. Opening the menu sets `attentive`, scrolls to the top, refreshes
  and starts the phrase tick (`PHRASE_INTERVAL_MS` 2800).
- `TailGaugeIcon` draws the mark natively on an `St.DrawingArea`; with
  `show-status-in-panel` the header title sits beside it.
- Settings: GSettings schema `org.gnome.shell.extensions.tailgauge`
  (`refresh-interval`, `show-status-in-panel`, `toggle-shortcut`,
  `active-provider`, `recent-mullvad-regions`), edited in `prefs.ts` (Adwaita:
  name in panel, refresh interval, shortcut capture, clear recent regions). The
  toggle shortcut is registered with `Main.wm.addKeybinding` and toggles
  in-process, so it works on an EGO install with nothing on PATH.
- Shell differences, probed at run time rather than by version number: `box()`
  sets `orientation` where GNOME 48 dropped `vertical`; `scrollView()` uses
  `add_actor` on GNOME 45, where `St.ScrollView` is an `St.Bin` without a
  `vadjustment`, and `child` from 46 on (setting `child` on 45 drew the section
  one pixel tall, the 0.6.1 fix). `tests/gnome/shell/run.sh --gnome 45` checks
  this in a real headless shell; it is local, not CI.
- Build: `tsc -p tsconfig.gnome.json` emits to `build/.ts/gnome`;
  `scripts/build.sh` copies the `.js` over the `.ts` into
  `build/tailgauge@arzaroth.github.io`, compiles the schemas with
  `glib-compile-schemas --strict` (refusing to package without them) and zips
  it. Edit the `.ts`, never `build/`. `metadata.json` carries an integer
  `version` for EGO and `version-name` for the footer.
- Logs: `journalctl --user -u org.gnome.Shell@wayland -f`.

## Omarchy

- `Panel.qml` is a `qs.Ui` `Panel` with `moduleName` and `ipcTarget`
  `arzaroth.tailgauge` (`manageIpc: false`). Its `IpcHandler` answers
  `omarchy-shell arzaroth.tailgauge <open|close|show|hide|toggle|refresh|up|down|toggleTailscale|status|diagnose>`;
  `diagnose` returns the shown provider, header, tooltip and each section's
  visibility and row count as JSON.
- `Service.qml` runs one `Process` per command kind (an `Action` component
  that clears its in-flight id on exit) and `execDetached` for copy, send and
  `xdg-open`. Its `busy` covers `actionProc`, `switchProc` and `exitNodeProc`.
  It reads its version from `manifest.json` with a `FileView` and refreshes
  once that loads.
- Settings live inline on the widget's entry in `~/.config/omarchy/shell.json`;
  `persistSetting` writes `activeProvider` and `recentMullvadRegions` back with
  `bar.shell.updateEntryInline`. The manifest's `barWidget.schema` exposes only
  `refreshIntervalSec`.
- Keys go through a `PanelKeyCatcher` that blocks while the copy menu is open or
  when it is not focused, so search fields receive `h`, `j`, `k`, `l`, `x` and
  letters intact. Tab switches between bar panels.
- It draws `glyph`, not `icon`, so account rows (which carry no glyph) show
  bare. Colours come from the bar (`bar.foreground`, `bar.urgent`); a status
  with tone `error` uses the urgent colour, everything dim is
  `Qt.darker(foreground, 1.55)`.
- Installed by copying, never symlinking (the plugin registry refuses symlinks);
  reload with `omarchy-shell -q shell rescanPlugins`. TailGauge began as a port
  of Omarchy's first-party `omarchy.tailscale` plugin; the MIT notice is in
  `omarchy/arzaroth.tailgauge/NOTICE`.
- Logs: `qs log --pid "$(pgrep -f 'quickshell -n -p /usr/share/omarchy/shell')" -t 50`.

## The TUI

`tailgauge tui` (or `tailgauge-tui`) draws the same `panel_spec` in a terminal,
gathering and acting on a worker thread, re-gathering every 3 s and 600 ms after
a command. It has no watcher and no settings file. See
[../features/tui.md](../features/tui.md).

## Sources

- `plasma/org.tailgauge.plasmoid/contents/ui/{main,CompactRep,FullRep,PanelRowView,ProviderService,ConfigGeneral}.qml`, `plasma/org.tailgauge.plasmoid/contents/config/main.xml`, `plasma/org.tailgauge.plasmoid/metadata.json`
- `gnome/tailgauge@arzaroth.github.io/panel.ts` (`SECTION_IDS`, `emptyPanel`), `provider.ts` (`ProviderService`, `_ui`, `_ctl`, `_showOptimistically`, `_watch`, `_armPoll`), `extension.ts` (`box`, `scrollView`, `TailGaugeIndicator`, `_signatureOf`, `_applySearch`, `_onMenuKey`, `_dispatch`), `prefs.ts`, `metadata.json`, `schemas/org.gnome.shell.extensions.tailgauge.gschema.xml`
- `omarchy/arzaroth.tailgauge/{Panel,Service}.qml` (`IpcHandler`, `diagnose`, `persistSetting`, `cycleProvider`), `omarchy/arzaroth.tailgauge/manifest.json`
- `crates/tailgauge/src/tui.rs`, `crates/tailgauge/src/copy.rs`
- `scripts/build.sh`, `tsconfig.gnome.json`, `tests/qml/run.sh`, `tests/gnome/shell/run.sh`
- `crates/tailgauge-core/tests/parity.rs`, `crates/tailgauge/tests/e2e.rs` (`every_command_the_frontends_invoke_exists`)
