# TUI

`tailgauge tui` (or the `tailgauge-tui` symlink): the panel in a terminal, laid
out after [tsui](https://github.com/neuralinkcorp/tsui), which it is meant to
replace. Added in 0.6.0. It draws what `panel_spec` resolves, in-process, with
no spawn of `tailgauge panel`; nothing in it decides which sections or rows
exist.

## What it shows ("tsui's shape")

- **Header**: the `TAILGAUGE` block wordmark and `by arzaroth` (dropped first on
  a narrow terminal), a status badge (`Loading...`, the header's warning meta,
  `Connected - <exit node>`, `Connected`, `Not Connected`) with the
  `(press . to connect)` hint, the account, and `tailgauge: <version>` /
  `provider: <label>` on the right.
- **Menu** down the left: every visible section that has a title
  (`App::menu`; the untitled update banner is not in it), each with a muted
  summary beside it (`section_value`: the current exit node or `None`, the
  provider, `N visible` machines, `N joined` networks, this device's name).
- **Pane** to the right of the selected section (`App::pane_rows`), where the
  TUI adds a view the widgets do not have, built only from the row's own
  payload: This device opens out into name, IPs and link details; exit nodes
  open with a `None` row (`clearExitNode`) so switching off is a row; machines
  are grouped under their owner (`Other devices` for none) with the OS as the
  second column.
- **Status bar**: the panel's status line toned warning/error, `Working…`
  while a command runs, `Error: ...` after a failed one, the search hint while
  typing, and `press q to quit`.
- Colours are tsui's palette by xterm index (`PRIMARY` 207 and so on in
  `render.rs`); values start at `VALUE_COLUMN` rather than right-aligning.

## Keys

| Key | Does |
| --- | --- |
| `j` `k` / arrows / `s` `w` | move in the menu, or in the open pane (captions, blanks and non-navigable rows are skipped) |
| `l` / Right / `d`, Enter, Space | open the section; inside a pane Enter and Space act on the row |
| `h` / Left / `a` | close the pane |
| `.` | connect or disconnect without opening anything (tsui's key) |
| `/` | type into the section's search field (`machineSearch`, `mullvadPicker`); Enter keeps, Esc clears |
| `r` | re-read now |
| Esc | close the pane, quit when none is open |
| `q`, Ctrl-C | quit |

These are not the desktop letters: `s` and `d` move rather than send or copy,
and `t`, `c`, `n` do nothing ([keyboard.md](keyboard.md)). Enter on a copy row
copies its first copy option's value.

## How it runs

- Gathering (`gather::panel_state`) and acting (`ctl::*`, `copy::run`,
  `selvedge::update::apply`) run on a worker thread; the draw loop polls keys
  every 120 ms and never blocks. Re-reads every `TICK` (3 s, the widgets'
  open-menu rate) and `SETTLE` (600 ms) after a command.
- Optimistic toggle via `Ui.active`, cleared once the daemon agrees. Commands
  go to the provider the panel shows, not the first one installed
  (`a_command_names_the_provider_the_panel_is_showing`).
- A fresh panel keeps the cursor on the same section id and row id, so a
  machine dropping off does not move it.
- Raw mode and the alternate screen are restored on exit and from a panic hook.
- Does not offer Taildrop sending, the Mullvad shortlist persistence, or a
  footer.

## Parity status

The TUI is the fourth renderer of `panel_spec` but is **not** in
`crates/tailgauge-core/tests/parity.rs`, whose `frontends()` lists Plasma,
GNOME and Omarchy only. Its search rule copies the widgets' (substring of the
pre-lowercased `searchKey`, trimmed and case-folded query), and `field_scope`
is a frontend-side mapping of row kind to scope that GNOME also keeps.

## Sources

- `crates/tailgauge/src/tui.rs` (`run`, `worker`, `perform`, `App::key`, `App::menu`, `App::pane_rows`, `device_rows`, `exit_node_rows`, `grouped_by_owner`, `section_value`, `field_scope`, `TICK`, `SETTLE`)
- `crates/tailgauge/src/tui/render.rs` (`draw`, `header`, `menu`, `pane`, `status_bar`, `WORDMARK`, `MENU_WIDTH`, `VALUE_COLUMN`)
- Tests in `tui.rs`: `machines_are_grouped_under_the_person_who_owns_them`, `the_exit_nodes_pane_opens_with_the_way_out`, `escape_closes_the_pane_before_it_quits`, `a_pane_with_nothing_to_act_on_is_walked_over`, `copying_a_row_copies_the_value_and_not_the_word_unknown`
- `CHANGELOG.md` (0.6.0)
