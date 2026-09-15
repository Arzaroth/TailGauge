//! One machine on the tailnet, however the provider spelled it.
//!
//! The field names are the ones the frontends already read, so a peer crossing
//! the process boundary needs no translation layer on the far side.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Every field a row can show. The optional ones are absent rather than empty
/// where a provider does not report them, because the panel distinguishes
/// "nothing to say" from "said nothing".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Peer {
    #[serde(rename = "id")]
    pub id: String,
    pub host_name: String,
    #[serde(rename = "UserID", skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taildrop_target: Option<i64>,
    #[serde(rename = "DNSName")]
    pub dns_name: String,
    pub display_name: String,
    #[serde(rename = "IPv4")]
    pub ipv4: Vec<String>,
    #[serde(rename = "IPv6")]
    pub ipv6: Vec<String>,
    pub online: bool,
    #[serde(rename = "OS")]
    pub os: String,
    pub tags: Vec<String>,
    pub exit_node_option: bool,
    pub exit_node: bool,
    pub mullvad: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mullvad_region: Option<bool>,
    /// How the tunnel is carried, and its round trip. -1 is "not measured"
    /// rather than "instant"; only NetBird reports a latency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<i64>,
    /// What the expanded row shows. Absent where the provider does not say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rx_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_handshake: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routes: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// reading whatever the CLI handed us
// ---------------------------------------------------------------------------

/// `String(value || "")`, which is what every field here was written against:
/// a missing key, a null and an empty string are the same answer.
pub fn text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(false)) => String::new(),
        Some(other) => other.to_string(),
    }
}

pub fn field(raw: &Value, key: &str) -> String {
    text(raw.get(key))
}

pub fn number(raw: &Value, key: &str) -> Option<i64> {
    raw.get(key).and_then(Value::as_i64)
}

pub fn strings(raw: &Value, key: &str) -> Vec<String> {
    raw.get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|v| text(Some(v))).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// names
// ---------------------------------------------------------------------------

pub fn clean_dns_name(name: &str) -> String {
    name.strip_suffix('.').unwrap_or(name).to_string()
}

pub fn short_dns_name(name: &str) -> String {
    let clean = clean_dns_name(name);
    if clean.is_empty() {
        return String::new();
    }
    let head = clean.split('.').next().unwrap_or("");
    if head.is_empty() {
        clean
    } else {
        head.to_string()
    }
}

/// A machine that calls itself `localhost` has told us nothing, so fall back to
/// the name the tailnet knows it by.
pub fn display_host_name(host_name: &str, dns_name: &str) -> String {
    if !host_name.is_empty() && !host_name.eq_ignore_ascii_case("localhost") {
        return host_name.to_string();
    }
    let short = short_dns_name(dns_name);
    if !short.is_empty() {
        short
    } else if !host_name.is_empty() {
        host_name.to_string()
    } else {
        "Unknown".to_string()
    }
}

const MULLVAD_SUFFIX: &str = ".mullvad.ts.net";

pub fn is_mullvad_host(name: &str) -> bool {
    let value = name.to_lowercase();
    value.len() > MULLVAD_SUFFIX.len() && value.ends_with(MULLVAD_SUFFIX)
}

pub fn is_mullvad_peer(raw: &Value) -> bool {
    is_mullvad_host(&clean_dns_name(&field(raw, "DNSName")))
        || is_mullvad_host(&field(raw, "HostName"))
}

// ---------------------------------------------------------------------------
// addresses
// ---------------------------------------------------------------------------

pub fn filter_ipv4(ips: &[String]) -> Vec<String> {
    ips.iter()
        .filter(|ip| ip.starts_with("100."))
        .cloned()
        .collect()
}

pub fn filter_ipv6(ips: &[String]) -> Vec<String> {
    ips.iter()
        .filter(|ip| ip.to_lowercase().starts_with("fd7a:115c:a1e0:"))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// owners
// ---------------------------------------------------------------------------

pub fn user_label(user: &Value) -> String {
    for key in ["DisplayName", "displayName", "LoginName", "loginName"] {
        let value = field(user, key);
        if !value.is_empty() {
            return value;
        }
    }
    let id = field(user, "ID");
    if !id.is_empty() {
        id
    } else {
        field(user, "id")
    }
}

pub fn users_by_id(raw: Option<&Value>) -> BTreeMap<String, String> {
    raw.and_then(Value::as_object)
        .map(|users| {
            users
                .iter()
                .map(|(id, user)| (id.clone(), user_label(user)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn peer_owner(raw: &Value, users: &BTreeMap<String, String>) -> String {
    let id = field(raw, "UserID");
    if id.is_empty() {
        return String::new();
    }
    users.get(&id).cloned().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// ordering
// ---------------------------------------------------------------------------

/// The order `String.localeCompare` gives for machine names, without depending
/// on which locale the process happens to run in - which is a property the
/// TypeScript never had, since `localeCompare` reads the runtime's default.
///
/// Case-insensitive first, so `Box` and `box` sort together rather than in two
/// blocks; lowercase wins a tie, which is what the root collation does and what
/// a byte comparison gets backwards.
pub fn collate(a: &str, b: &str) -> Ordering {
    let folded = a.to_lowercase().cmp(&b.to_lowercase());
    if folded != Ordering::Equal {
        return folded;
    }
    b.cmp(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_localhost_falls_back_to_the_name_the_tailnet_knows() {
        assert_eq!(
            display_host_name("laptop", "laptop.example.ts.net"),
            "laptop"
        );
        assert_eq!(
            display_host_name("localhost", "laptop.example.ts.net"),
            "laptop"
        );
        assert_eq!(
            display_host_name("LOCALHOST", "laptop.example.ts.net"),
            "laptop"
        );
        assert_eq!(display_host_name("", ""), "Unknown");
        assert_eq!(display_host_name("localhost", ""), "localhost");
    }

    #[test]
    fn a_trailing_dot_is_not_part_of_the_name() {
        assert_eq!(clean_dns_name("box.example.ts.net."), "box.example.ts.net");
        assert_eq!(clean_dns_name("box.example.ts.net"), "box.example.ts.net");
        assert_eq!(short_dns_name("box.example.ts.net."), "box");
        assert_eq!(short_dns_name(""), "");
    }

    #[test]
    fn only_a_mullvad_suffix_makes_a_mullvad_host() {
        assert!(is_mullvad_host("de-ber-wg-001.mullvad.ts.net"));
        assert!(is_mullvad_host("DE-BER-WG-001.MULLVAD.TS.NET"));
        assert!(
            !is_mullvad_host(".mullvad.ts.net"),
            "a suffix alone is not a host"
        );
        assert!(!is_mullvad_host("mullvad.ts.net"));
        assert!(!is_mullvad_host("laptop.example.ts.net"));
        assert!(is_mullvad_peer(&json!({"DNSName": "x.mullvad.ts.net."})));
        assert!(is_mullvad_peer(&json!({"HostName": "x.mullvad.ts.net"})));
        assert!(!is_mullvad_peer(&json!({"HostName": "laptop"})));
    }

    #[test]
    fn only_tailnet_addresses_are_kept() {
        let ips: Vec<String> = [
            "100.64.0.1",
            "192.168.1.5",
            "fd7a:115c:a1e0::1",
            "2001:db8::1",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(filter_ipv4(&ips), ["100.64.0.1"]);
        assert_eq!(filter_ipv6(&ips), ["fd7a:115c:a1e0::1"]);
    }

    #[test]
    fn an_owner_falls_through_display_name_to_login_to_id() {
        assert_eq!(
            user_label(&json!({"DisplayName": "Alice", "LoginName": "a@b"})),
            "Alice"
        );
        assert_eq!(user_label(&json!({"LoginName": "a@b"})), "a@b");
        assert_eq!(user_label(&json!({"ID": 7})), "7");
        assert_eq!(user_label(&json!({})), "");

        let users = users_by_id(Some(&json!({"1": {"DisplayName": "Alice"}})));
        assert_eq!(peer_owner(&json!({"UserID": "1"}), &users), "Alice");
        assert_eq!(peer_owner(&json!({"UserID": "2"}), &users), "");
        assert_eq!(peer_owner(&json!({}), &users), "");
    }

    #[test]
    fn machines_sort_the_way_a_reader_would_look_for_them() {
        let mut names = vec!["box-10", "Box-2", "apple", "Zebra", "box-1"];
        names.sort_by(|a, b| collate(a, b));
        assert_eq!(names, ["apple", "box-1", "box-10", "Box-2", "Zebra"]);

        // Lowercase wins a tie, which is the root collation's answer and the
        // opposite of what comparing bytes gives.
        assert_eq!(collate("a", "A"), Ordering::Less);
        assert_eq!(collate("a", "a"), Ordering::Equal);
    }

    #[test]
    fn a_missing_field_reads_as_an_empty_string() {
        let peer = json!({"HostName": "box", "Online": true, "RxBytes": 12});
        assert_eq!(field(&peer, "HostName"), "box");
        assert_eq!(field(&peer, "Nothing"), "");
        assert_eq!(number(&peer, "RxBytes"), Some(12));
        assert_eq!(number(&peer, "Nothing"), None);
        assert_eq!(strings(&peer, "Nothing"), Vec::<String>::new());
    }
}
