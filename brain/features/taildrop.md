# Taildrop

Sending files to a tailnet machine from its row, and receiving files into the
downloads directory with a notification. Tailscale only: NetBird's descriptor
has no `FileSend` capability.

## Send

- **When the row offers it**: `panel.rs` `can_send_files` adds a `send` action
  ("Send files") to a machine row only when the active provider supports
  `Capability::FileSend`, the tailnet grants file sharing (`file_sharing`,
  read by `status.rs` `has_file_sharing` from `Self.CapMap` or the older
  capability list, key `https://tailscale.com/cap/file-sharing`), the daemon is
  running, the peer is online, `state.helpers` is set, and the peer is a
  Taildrop target. `is_taildrop_target` takes the daemon's `TaildropTarget`
  grading when it is non-zero (1 = yes), otherwise the older rule: a machine
  owned by the same user.
- `helpers` exists because the KDE Store and EGO cannot put the binary on
  `PATH`. `gather::panel_state` sets it to `true` unconditionally (the binary is
  the one answering), so in practice it only matters to a hand-built state. A
  store install without the binary has no panel at all, send included
  ([../architecture/distribution.md](../architecture/distribution.md)).
- **Clicking or `s`** (see [keyboard.md](keyboard.md)): every desktop detaches
  `tailgauge send --peer <row payload JSON>` and closes its popup, because the
  file chooser takes over. `--peer` lets the binary pick the address
  (`ctl::address_of`) rather than trusting the caller. The TUI does not offer
  sending.
- `send.rs` `run`: with no files named, opens the chooser titled `Send to
  <short name>`; cancelling is `NothingToSend`, not a failure. Then
  `tailscale file cp --update-interval=0 -- <files> <machine>:` (the `--` keeps
  a file named `-x` a file, the trailing colon marks the destination). A
  notification reports `Sent to <name>` or `Could not send to <name>` with
  whichever stream explained the failure. With no chooser installed it posts a
  critical `Could not send to <name>` / `The file chooser did not open` and
  exits 1.
- `file_select.rs`: `kdialog` first on a KDE/Plasma session, `zenity` first
  elsewhere, the other as fallback; both asked for one path per line
  (`--separate-output`, `--separator=\n`) so spaces in paths survive. Run as
  `tailgauge file-select`, no chooser is exit 2 (`EXIT_NO_CHOOSER`) and the
  message `install zenity or kdialog to pick files`.

## Receive

- `tailgauge receive [--once] [DIR]`, run by the systemd user unit
  `systemd/tailgauge-receive.service` (`Restart=always`, part of
  `graphical-session.target`). `scripts/install.sh` installs and enables it;
  `--no-taildrop` skips it. Release archives carry it under `systemd/`.
- `DIR` defaults to `$XDG_DOWNLOAD_DIR`, else `~/Downloads` (`default_dir`).
- Loop: `tailscale file get --wait --conflict=rename <DIR>/.tailgauge-taildrop`
  (a staging dir on the same filesystem), then deliver. A failed `get` retries
  after 10 s (or errors with `--once`). Anything left staged by an earlier run
  is delivered at start.
- `claim_path` moves each file in with `hard_link` (which refuses an existing
  name, so there is no check-then-rename race), numbering collisions
  `name-1.ext` with only the last extension kept off the counter
  (`notes.tar-1.gz`), then unlinks the staged copy.
- Each file is announced `Received <name>` / `Saved to ~/Downloads`, never with
  the file itself as the notification picture: a received file is untrusted, so
  nothing decodes it before someone opens it. Click-to-open:
  `notify.rs` re-execs itself detached as `notify --await-action --open
  <path>`, which runs `notify-send --wait --action=default=Open` and
  `xdg-open`s the file on the default action. Without `notify-send` action
  support it posts a plain notification; without `notify-send` it posts nothing.

## Sources

- `crates/tailgauge-core/src/panel.rs` (`can_send_files`, `is_taildrop_target`), `crates/tailgauge-core/src/status.rs` (`has_file_sharing`, `FILE_SHARING_CAP`), `crates/tailgauge-core/src/providers.rs` (`Capability::FileSend`)
- `crates/tailgauge/src/send.rs` (`run`, `taildrop_args`, `why_failed`), `crates/tailgauge/src/file_select.rs` (`order`, `kdialog_args`, `zenity_args`, `picked`)
- `crates/tailgauge/src/receive.rs` (`STAGING`, `claim_path`, `split_name`, `default_dir`), `crates/tailgauge/src/notify.rs` (`spawn_actionable`, `await_action`)
- `crates/tailgauge/src/main.rs` (`Command::Send`, `Command::Receive`, `Command::FileSelect`, `EXIT_NO_CHOOSER`), `systemd/tailgauge-receive.service`, `scripts/install.sh`
- Tests: `the_send_action_appears_only_for_a_taildrop_target`, `taildrop_targets_follow_the_daemon_grading`, `sending_files_is_a_provider_capability_not_just_a_helper_check` (`specification.rs`), `a_taken_name_is_numbered_rather_than_overwritten` (`receive.rs`), `a_peer_payload_can_still_name_the_files_to_send` (`main.rs`)
