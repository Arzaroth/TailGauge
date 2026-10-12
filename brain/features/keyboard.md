# Keyboard

Driving the panel without a pointer: one traversal order from the binary, six
single-letter actions shared by the three desktops, and an optional global
connection-toggle key on GNOME. Plasma has none: its widget shortcut only opens
the popup, and a custom command on `tailgauge ctl toggle` stands in. The TUI has its own keys ([tui.md](tui.md)).

## The letters

| Key | Does | Applies to |
| --- | --- | --- |
| `t` | toggle the connection | anywhere in the open panel |
| `r` | refresh now | anywhere |
| `c` | copy the IP (`ip` copy option) | a row with `copyOptions` |
| `n` | copy the name | a row with `copyOptions` |
| `d` | copy the MagicDNS name | a row with `copyOptions` |
| `s` | send files ([taildrop.md](taildrop.md)) | a row whose `actions` include `send` |

Upper and lower case both work. Each frontend reads only what the row carries
(`copyOptions`, `actions`), so whether a letter applies is the binary's
decision. A letter on a row without that option does nothing.

## Cursor stops: `panel.navigation`

`panel.rs` `panel_navigation` emits the traversal: the header first (its action
is `toggle`), then every `navigable` row of every visible section in draw
order, with an expanded row's navigable children right after it. Each
`NavEntry` carries `sectionId`, `rowId`, `action`, `searchScope`, `searchKey`,
so a frontend filters stops with the same predicate it filters rows with.
Search fields and their empty-result rows are not stops. Tests:
`navigation_visits_the_header_then_every_visible_row_in_draw_order`,
`hidden_sections_contribute_no_cursor_stops`,
`every_cursor_stop_resolves_back_to_its_row`,
`an_expanded_picker_puts_its_regions_in_the_traversal_a_closed_one_does_not`.

## Per desktop

- **Plasma** (`FullRep.qml`, `Keys.onPressed`): Down/`j` and Up/`k` move
  `cursorIndex` through `panel.navigation`, skipping stops a live query hides;
  the first press only shows the cursor. Enter, Return or Space activates: stop
  0 toggles, any other dispatches the row's action. Esc closes the Mullvad
  picker first, then the popup. The cursor follows a row's id across polls
  (`_pinnedRowId`), and resets to the top each time the popup opens.
- **Omarchy** (`Panel.qml`): the shell's `KeyboardPanel` and `PanelKeyCatcher`
  carry the same cursor over `panel.navigation`; `j`/`k` or arrows move, Enter
  activates, Esc closes the picker then the panel, Tab moves to the
  neighbouring bar panel. Letters arrive through `onTextKey`. The catcher is
  `blocked` while the copy menu is open or a search field has focus, so typing
  `c` or `t` into a query no longer fires an action. Inside a search field
  Up/Down still move the cursor and Enter activates.
- **GNOME** (`extension.ts`): native `PopupMenu` rows, so arrows, Enter and
  type-ahead are the shell's own and `panel.navigation` is unused.
  `_onMenuKey` on the menu box handles only the letters, passes keys through
  when an editable `Clutter.Text` has focus, and resolves the row from the
  actor with key focus (`_focusedRow`). `c`/`n`/`d` close the menu after
  copying; `s` closes it via the file chooser path.
- **Machine copy menus**: Plasma's is a native `QQC2.Menu`
  (`PanelRowView.qml`); Omarchy's popup takes `j`/`k`/arrows and Enter/Space
  itself; GNOME puts copy actions in the machine's submenu.

## Global toggle

- **GNOME**: the `toggle-shortcut` key (`as`, default `[]`, so it collides with
  nothing) in `org.gnome.shell.extensions.tailgauge`, set from the **Shortcut**
  row in `prefs.ts` (`captureShortcut`: needs a modifier, Backspace clears, Esc
  cancels). `enable()` registers it with `Main.wm.addKeybinding` in `NORMAL` and
  `OVERVIEW` modes, calling the indicator's `toggle()`. The binding is
  in-process but the toggle is not: `toggle()` calls `toggleTailscale()`, which
  returns early unless the binary has answered a panel and otherwise spawns
  `tailgauge ctl` through `_ctl`, so without the binary on `PATH` the key does
  nothing.
- **Plasma**: the applet's standard *Configure Keyboard Shortcuts* opens the
  popup, with no code here. To toggle without opening it, bind `tailgauge ctl
  toggle` as a custom command in System Settings.
- **Omarchy**: the widget's `IpcHandler` answers `omarchy-shell
  arzaroth.tailgauge toggleTailscale` (the connection) and `toggle` (opens or
  closes the panel), plus `open`, `close`, `refresh`, `up`, `down`, `status`,
  `diagnose`; bind either in Hyprland.
- Anywhere: `tailgauge ctl toggle` (or `tailgauge-ctl toggle`) drives the daemon
  directly; it does not talk to a running widget.

## Sources

- `crates/tailgauge-core/src/panel.rs` (`panel_navigation`, `NavEntry`, `panel_row_at`)
- `plasma/org.tailgauge.plasmoid/contents/ui/FullRep.qml` (`moveCursor`, `activateCursor`, `dispatch`, `Keys.onPressed`)
- `omarchy/arzaroth.tailgauge/Panel.qml` (`KeyboardPanel`, `PanelKeyCatcher`, `onTextKey`, `moveCursor`)
- `gnome/tailgauge@arzaroth.github.io/extension.ts` (`_onMenuKey`, `_focusedRow`, `toggle`, `TailGaugeExtension.enable`), `provider.ts` (`toggleTailscale`, `_ctl`), `prefs.ts` (`captureShortcut`), `schemas/org.gnome.shell.extensions.tailgauge.gschema.xml` (`toggle-shortcut`)
- `README.md` (Features, Outside the panel), `omarchy/arzaroth.tailgauge/README.md`
