# Accounts ("Connections")

Switching between Tailscale profiles (several logins or tailnets on one
daemon). The section is titled **Connections**, its rows are accounts; only
providers with the `accounts` capability have it, which today is Tailscale.

- **Reading.** `gather::extras` runs `tailscale switch --list --json` (the
  descriptor's `accounts()`) alongside the other extras. `parse_accounts` reads
  each entry through `first_of`, accepting every key spelling the CLI has used
  (`id`/`ID`, `nickname`/`Nickname`/`name`/`Name`, `tailnet`/`Tailnet`,
  `account`/`loginName`/`user`...; `either_spelling_the_cli_has_used_is_read`).
  Empty or non-array output is no accounts, not an error.
- **Label.** `account_label`: nickname, else tailnet, else account, else the id,
  else "Unknown account".
- **Section.** `connections_section` emits one `account:<id>` row per profile
  (`action: switchAccount`, `current`/`bold` on the selected one, `busy` while
  `switchingAccountId` matches). Visible when the provider has accounts
  **and** there is a choice: more than one profile, or the operator prompt
  (`connections_appear_only_with_a_choice_to_make`); or, for any provider,
  when its CLI refuses this user outright (`accessDenied`, below). Account rows carry no
  glyph, so Omarchy draws them bare; the code keeps that on purpose.
- **Switching.** The frontend runs `tailgauge ctl --provider tailscale
  switch-account <id>`, which runs `tailscale switch <id>`; failure is a
  critical notification "Could not switch profile" when unattended.

## The operator offer

Without operator rights, `tailscale switch --list` fails with "profiles access
denied" on stderr. `fmt::is_profiles_access_denied` (case-insensitive) turns
that into `accounts_access_denied`, and the section leads with an `auth` row,
`action: authorize` (`the_operator_prompt_leads_the_connections_section`).

The row's words and command are the backend's `authorization()`, so the same
row lets a user into any provider whose CLI refuses them. A status that comes
back access denied (ZeroTier without its token) sets `accessDenied`, which
shows the row with no accounts at all
(`a_cli_that_refuses_this_user_asks_for_it_whether_or_not_there_are_accounts`):

| Provider | Row | `ctl authorize` runs |
| --- | --- | --- |
| Tailscale | "Authorize Tailscale operator" / "Allow this user to operate this Tailscale profile" | `pkexec tailscale set --operator="$(id -un)"` |
| ZeroTier | "Allow access to ZeroTier" / "Copy the ZeroTier service token to this user" | `pkexec install -m 600 -o "$(id -un)" ... authtoken.secret "$HOME/.zeroTierOneAuthToken"` |

`ctl authorize` runs the line with `sh -c`. The user is resolved by that
shell, because a widget started without `$USER` in its environment would
otherwise authorize nobody
(`the_operator_is_the_user_running_the_command_rather_than_the_one_here`).
Unattended, pkexec's stdio is nulled and a failure is a notification. A
provider with no `authorization()` (NetBird) is refused before anything
spawns. The frontends call it `authorizeOperator`.

## Sources

- `crates/tailgauge-core/src/accounts.rs` (`parse_accounts`, `first_of`, `account_label`, `AccountsResult`)
- `crates/tailgauge-core/src/panel.rs` (`connections_section`), `crates/tailgauge-core/src/fmt.rs` (`is_profiles_access_denied`)
- `crates/tailgauge/src/gather.rs` (`extras`, `connect`), `crates/tailgauge/src/ctl.rs` (`switch_account`, `authorize`), `crates/tailgauge-core/src/providers/mod.rs` (`Authorization`), `providers/tailscale.rs` and `providers/zerotier.rs` (`authorization`)
- `crates/tailgauge-core/tests/fixtures/accounts.json`, `crates/tailgauge-core/tests/specification.rs` (`parse_accounts_finds_the_selected_profile`, `the_selected_account_is_the_current_row`)
