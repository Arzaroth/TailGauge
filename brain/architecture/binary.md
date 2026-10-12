# The `tailgauge` binary

One executable, `crates/tailgauge`, that every frontend shells out to: it
gathers the machine and prints the resolved panel, drives the VPN daemon, and
carries the small desktop helpers the frontends would otherwise each write.
The decisions live in `tailgauge-core`; this crate spawns processes and wires
argv to them. How the panel is built is in [panel-spec.md](panel-spec.md),
what it asks each CLI in [providers.md](providers.md), updating and packaging
in [distribution.md](distribution.md).

## Command line

`Cli` (clap, `propagate_version`) is an optional subcommand plus four
top-level flags. `resolve()` turns them into one `Action` and refuses
combinations: two update flags at once, or an update flag with a subcommand,
exit 2 with a message (`the_update_flags_do_one_thing_each`). No subcommand and
no flag prints help and exits 2 (`Action::Nothing`).

| Flag | Does |
| --- | --- |
| `--check-update [--force]` | cached release check, printed as JSON (`handle_check_update`) |
| `--update` | download the latest release, swap the binary, refresh installed frontends, report skew (`handle_update`, `report_frontend_skew`) |
| `--install-frontend <plasma\|gnome\|omarchy\|all>` | install a frontend payload from this binary's release (`handle_install_frontend`) |
| `-V`, `--version` | clap's version |

| Subcommand | Does | Exit |
| --- | --- | --- |
| `panel [--json] [--ui '<json>']` | gather every installed provider and print `panel_spec` as JSON (`run_panel`); `--ui` defaults to `{}`, `--json` is accepted and ignored | 0, 1 on bad `--ui` |
| `ctl [--provider ID] <action>` | drive the daemon, see below | per action |
| `watch [TIMEOUT=300]` | block until tailscaled reports a change ([providers.md](providers.md)) | 0 changed, 2 expired, 1 unusable |
| `send [--peer JSON] MACHINE\|FILES...` | Taildrop send, opening a chooser when no file is named | 0 sent or nothing picked, 1 failed |
| `receive [--once] [DIR]` | Taildrop receive loop, the systemd unit's `ExecStart` | 0 / 1 |
| `tui` | the panel in a terminal ([../features/tui.md](../features/tui.md)) | 0 / 1 |
| `copy [TEXT]` | clipboard write | 0, 1 when no tool worked |
| `notify [--image] [--open PATH] [-u URGENCY] [SUMMARY] [BODY]` | desktop notification; `--await-action` is hidden and internal | 0 |
| `file-select [--title] [--multiple]` | desktop file chooser, one path per line | 0 picked, 1 cancelled, 2 no chooser (`EXIT_NO_CHOOSER`) |

`send` and `receive` refuse with exit 1 when `tailscale` is not on PATH.
Failures print `tailgauge: <why>` on stderr (`report`, `settle`).

## argv[0] dispatch

The installer writes one symlink per subcommand beside the binary
(`tailgauge-ctl`, `-watch`, `-notify`, `-send`, `-receive`, `-file-select`,
`-copy`, `-tui`), keeping the names of the shell helpers the binary replaced,
so a key binding on `tailgauge-ctl toggle` and the Taildrop unit keep working.
`splice_alias` strips the `tailgauge-` prefix off argv[0] and **inserts** the
rest as argv[1], so the symlink and the subcommand share one parser. A bare
`tailgauge-` is left alone. There is no `tailgauge-panel` symlink, and the old
`tailgauge-update` is in `project::TAILGAUGE.legacy` because updating is a flag.

- `ALIASES` in `project.rs` (what selvedge rewrites on `--update`) and the
  `for alias in` loop in `scripts/install.sh` must match:
  `the_installer_and_the_updater_write_the_same_aliases`,
  `every_alias_the_installer_writes_is_a_subcommand`.

## `ctl`

`resolve_provider` picks `--provider ID` (unknown id or CLI not on PATH is
exit 1), else the first `supported` provider whose CLI is on PATH. It does not
know which provider a panel shows, so frontends always pass `--provider`.
`ctl version` answers before that check (`tailgauge-ctl <version>`).

| Action | Behaviour |
| --- | --- |
| `status` | "Connected as NAME (IP)" plus "Exit node: HOST" when the provider has exit nodes; exit 0 connected, **3** (`EXIT_DISCONNECTED`) not connected or not answering |
| `toggle` | reads status, then `down` or `up` (never `down \|\| up`, which would turn a failed down back on) |
| `up` / `down` | see [../features/connection.md](../features/connection.md) |
| `exit-node [NAME \| --peer JSON]` | no arg prints the current one (`tailscale::current_exit_node`); `off`/`none`/`clear` clear it (`requested_node`); `--peer` resolves through `target_of` |
| `exit-nodes` | prints `tailscale exit-node list` |
| `switch-account ID` | `tailscale switch ID` |
| `select-network ID [--leave]` | `netbird networks select\|deselect ID` |
| `authorize` | `sh -c 'pkexec <cli> set --operator="$(id -un)"'` (`authorize_command`) |

Each action asks the descriptor for its argv first, so a provider without the
capability fails before anything spawns
(`a_provider_is_never_asked_for_what_it_does_not_have`). `ctl` exits 0, 1
(`Outcome::Failed`) or 3 (`Outcome::Disconnected`).

**Attended or not.** `attended()` is stdout being a terminal and
`take_over_terminal()` not having been called (the TUI calls it so a daemon's
chatter does not land on its frame). Unattended, failures become
notifications through `announce`, `up` scrapes the login URL, and `authorize`
detaches pkexec's stdio.

## Helpers

- **`notify.rs`**: `notify-send --app-name=TailGauge --icon=network-vpn-symbolic`,
  always with `--` before summary and body (`the_message_is_never_read_as_options`).
  No `notify-send` is not a failure. With `--open PATH` and a `notify-send`
  that knows `--action`, it re-execs itself detached (`spawn_actionable`,
  process group 0) with `--await-action`; that copy runs `--wait
  --action=default=Open` and `xdg-open`s the path on click.
- **`copy.rs`**: empty text is a no-op. Tries `xclip -selection clipboard`,
  `xsel --clipboard --input`, `wl-copy`, with `wl-copy` rotated first only when
  `WAYLAND_DISPLAY` is set (`wl_copy_is_tried_first_only_under_wayland`). A
  tool that fails moves on to the next.
- **`file_select.rs`**: `kdialog` first when `XDG_CURRENT_DESKTOP` contains
  kde/plasma, `zenity` first otherwise; both asked for one path per line
  (`--separate-output`, `--separator=\n`). Non-zero exit or empty output is
  `Picked::Cancelled`.
- **`send.rs`, `receive.rs`**: Taildrop ([../features/taildrop.md](../features/taildrop.md)).

## Modules

`main.rs` (CLI, dispatch, update handlers), `gather.rs`, `watch.rs`,
`tailscale.rs` ([providers.md](providers.md)), `ctl.rs`, `notify.rs`,
`copy.rs`, `file_select.rs`, `send.rs`, `receive.rs`, `tui.rs` + `tui/render.rs`,
`project.rs` (the selvedge `Project`: repo, `ALIASES`, `FRONTENDS`).
`tests/e2e.rs` runs the built binary against fake CLIs, including
`every_command_the_frontends_invoke_exists` ([testing.md](testing.md)).

## Sources

- `crates/tailgauge/src/main.rs` (`Cli`, `Action`, `resolve`, `Command`, `CtlAction`, `EXIT_NO_CHOOSER`, `resolve_provider`, `run_panel`, `splice_alias`, `settle`, `report`)
- `crates/tailgauge/src/ctl.rs` (`EXIT_DISCONNECTED`, `Outcome`, `attended`, `take_over_terminal`, `announce`, `target_of`, `address_of`, `requested_node`, `authorize_command`)
- `crates/tailgauge/src/notify.rs` (`Notification`, `run`, `spawn_actionable`, `await_action`), `copy.rs` (`candidates`), `file_select.rs` (`order`, `Picked`)
- `crates/tailgauge/src/project.rs` (`TAILGAUGE`, `ALIASES`), `scripts/install.sh` (alias loop), `systemd/tailgauge-receive.service`
- `crates/tailgauge/tests/e2e.rs`
