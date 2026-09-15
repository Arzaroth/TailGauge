//! `tailscale switch --list --json`, and the label a switcher row shows.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::peer::text;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Account {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tailnet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountsResult {
    pub accounts: Vec<Account>,
    #[serde(rename = "selectedAccountId")]
    pub selected_account_id: String,
    #[serde(rename = "selectedAccountLabel")]
    pub selected_account_label: String,
}

/// The first key of each group that carries a value, because the CLI has
/// spelled these differently across versions and both spellings are still out
/// there on machines that have not updated.
fn first_of(raw: &Value, keys: &[&str]) -> String {
    for key in keys {
        let value = text(raw.get(*key));
        if !value.is_empty() {
            return value;
        }
    }
    String::new()
}

pub fn parse_accounts(raw: &str) -> AccountsResult {
    let text = raw.trim();
    if text.is_empty() {
        return AccountsResult::default();
    }
    let Ok(parsed) = serde_json::from_str::<Value>(text) else {
        return AccountsResult::default();
    };
    let Some(entries) = parsed.as_array() else {
        return AccountsResult::default();
    };

    let mut accounts = Vec::new();
    let mut selected: Option<Account> = None;
    for entry in entries {
        let account = Account {
            id: first_of(entry, &["id", "ID"]),
            nickname: Some(first_of(entry, &["nickname", "Nickname", "name", "Name"])),
            tailnet: Some(first_of(entry, &["tailnet", "Tailnet"])),
            account: Some(first_of(
                entry,
                &[
                    "account",
                    "Account",
                    "loginName",
                    "LoginName",
                    "user",
                    "User",
                ],
            )),
            selected: Some(
                entry.get("selected") == Some(&Value::Bool(true))
                    || entry.get("Selected") == Some(&Value::Bool(true)),
            ),
        };
        if account.selected == Some(true) {
            selected = Some(account.clone());
        }
        accounts.push(account);
    }

    AccountsResult {
        selected_account_id: selected.as_ref().map(|a| a.id.clone()).unwrap_or_default(),
        selected_account_label: selected.as_ref().map(account_label).unwrap_or_default(),
        accounts,
    }
}

/// What to call an account, in the order a person would recognise it.
pub fn account_label(account: &Account) -> String {
    for value in [&account.nickname, &account.tailnet, &account.account] {
        if let Some(value) = value
            && !value.is_empty()
        {
            return value.clone();
        }
    }
    if account.id.is_empty() {
        "Unknown account".to_string()
    } else {
        account.id.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_and_nonsense_both_read_as_no_accounts() {
        assert_eq!(parse_accounts(""), AccountsResult::default());
        assert_eq!(parse_accounts("not json"), AccountsResult::default());
        assert_eq!(
            parse_accounts("{}"),
            AccountsResult::default(),
            "an object is not a list"
        );
    }

    #[test]
    fn the_selected_account_is_reported_by_id_and_by_label() {
        let parsed = parse_accounts(
            r#"[{"id":"1","nickname":"work"},{"id":"2","tailnet":"home.ts.net","selected":true}]"#,
        );
        assert_eq!(parsed.accounts.len(), 2);
        assert_eq!(parsed.selected_account_id, "2");
        assert_eq!(parsed.selected_account_label, "home.ts.net");
    }

    #[test]
    fn either_spelling_the_cli_has_used_is_read() {
        let parsed = parse_accounts(r#"[{"ID":"7","Name":"Work","Selected":true}]"#);
        assert_eq!(parsed.selected_account_id, "7");
        assert_eq!(parsed.selected_account_label, "Work");
    }

    #[test]
    fn a_label_falls_through_to_whatever_is_left() {
        let by_id = Account {
            id: "9".into(),
            ..Account::default()
        };
        assert_eq!(account_label(&by_id), "9");
        assert_eq!(account_label(&Account::default()), "Unknown account");
    }
}
