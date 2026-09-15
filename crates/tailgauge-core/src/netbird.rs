//! NetBird: `netbird status --json`, and `netbird networks list`.

use serde::Serialize;
use serde_json::Value;

use crate::peer::{self, Peer};
use crate::status::{StatusError, StatusOk, StatusResult, StatusUnavailable};

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

fn netbird_online(status: &str) -> bool {
    status.eq_ignore_ascii_case("connected")
}

/// NetBird names a peer by its FQDN; the panel shows the first label of it,
/// falling back to the address when there is no name at all.
fn netbird_host_name(fqdn: &str, fallback: &str) -> String {
    let name = fqdn.trim();
    match name.find('.') {
        Some(dot) if dot > 0 => name[..dot].to_string(),
        _ if !name.is_empty() => name.to_string(),
        _ => fallback.to_string(),
    }
}

fn netbird_peer(raw: &Value) -> Peer {
    let fqdn = peer::clean_dns_name(&peer::field(raw, "fqdn"));
    let ip = peer::strip_cidr(raw.get("netbirdIp"));
    let host = netbird_host_name(&fqdn, &ip);
    let public_key = peer::field(raw, "publicKey");
    let latency_ns = raw.get("latency").and_then(Value::as_f64).unwrap_or(0.0);

    Peer {
        id: if !fqdn.is_empty() {
            fqdn.clone()
        } else if !ip.is_empty() {
            ip.clone()
        } else {
            public_key.clone()
        },
        host_name: host.clone(),
        dns_name: fqdn,
        display_name: host,
        ipv4: if ip.is_empty() { Vec::new() } else { vec![ip] },
        ipv6: Vec::new(),
        online: netbird_online(&peer::field(raw, "status")),
        // NetBird's status carries no OS, so every row falls back to the
        // generic machine glyph rather than guessing one.
        os: String::new(),
        tags: Vec::new(),
        exit_node_option: false,
        exit_node: false,
        mullvad: false,
        status: Some(peer::field(raw, "status")),
        connection_type: Some(peer::field(raw, "connectionType")),
        latency_ms: Some(if latency_ns > 0.0 {
            latency_ns / 1_000_000.0
        } else {
            -1.0
        }),
        endpoint: Some(peer::text(
            raw.get("iceCandidateEndpoint")
                .and_then(|e| e.get("remote")),
        )),
        relay: Some(peer::field(raw, "relayAddress")),
        rx_bytes: Some(
            raw.get("transferReceived")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        ),
        tx_bytes: Some(raw.get("transferSent").and_then(Value::as_i64).unwrap_or(0)),
        last_handshake: Some(peer::non_zero_time(raw, "lastWireguardHandshake")),
        public_key: Some(public_key),
        routes: Some(peer::strings(raw, "networks")),
        ..Peer::default()
    }
}

pub fn parse_netbird_status(raw: &str) -> StatusResult {
    let text = raw.trim();
    if text.is_empty() {
        return StatusResult::Unavailable(StatusUnavailable {
            ok: true,
            unavailable: true,
            message: "Disconnected".into(),
        });
    }

    let failed = || {
        StatusResult::Error(StatusError {
            ok: false,
            unavailable: true,
            message: "Status error".into(),
            error: "Failed to parse netbird status".into(),
        })
    };

    let Ok(data) = serde_json::from_str::<Value>(text) else {
        return failed();
    };
    if !data.is_object() {
        return failed();
    }

    let state = peer::field(&data, "daemonStatus");
    let lowered = state.to_lowercase();
    let running = lowered == "connected";
    let needs_login =
        lowered == "needslogin" || lowered == "sessionexpired" || lowered == "loginfailed";

    let mut peers: Vec<Peer> = data
        .get("peers")
        .and_then(|p| p.get("details"))
        .and_then(Value::as_array)
        .map(|details| details.iter().map(netbird_peer).collect())
        .unwrap_or_default();
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| peer::collate(&a.host_name, &b.host_name))
    });

    let self_fqdn = peer::clean_dns_name(&peer::field(&data, "fqdn"));
    let self_ip = peer::strip_cidr(data.get("netbirdIp"));
    let self_name = netbird_host_name(&self_fqdn, &self_ip);
    let self_peer = Peer {
        id: if !self_fqdn.is_empty() {
            self_fqdn.clone()
        } else if !self_ip.is_empty() {
            self_ip.clone()
        } else {
            "self".to_string()
        },
        host_name: self_name.clone(),
        dns_name: self_fqdn.clone(),
        display_name: self_name.clone(),
        ipv4: if self_ip.is_empty() {
            Vec::new()
        } else {
            vec![self_ip.clone()]
        },
        ipv6: Vec::new(),
        online: running,
        os: String::new(),
        tags: Vec::new(),
        exit_node_option: false,
        exit_node: false,
        mullvad: false,
        ..Peer::default()
    };

    StatusResult::Ok(Box::new(StatusOk {
        ok: true,
        unavailable: false,
        daemon_state: state,
        running,
        needs_login,
        // NetBird prints its authorization URL on the `up` stream rather than
        // into status, so there is never one to read here.
        auth_url: String::new(),
        self_name,
        self_dns_name: self_fqdn,
        self_ip,
        self_user_id: String::new(),
        self_peer,
        file_sharing: false,
        peers,
        exit_nodes: Vec::new(),
    }))
}

// ---------------------------------------------------------------------------
// networks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Network {
    pub id: String,
    pub range: String,
    pub domains: Vec<String>,
    pub selected: bool,
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NetworksResult {
    pub ok: bool,
    pub networks: Vec<Network>,
    pub message: String,
}

/// A `-` is how the CLI spells an absent value, in the range as in the domain
/// list. Neither is something to show.
fn split_network_list(value: &str) -> Vec<String> {
    let text = value.trim();
    if text.is_empty() || text == "-" {
        return Vec::new();
    }
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

/// `netbird networks list` prints a block per network, `- ID:` opening each.
pub fn parse_netbird_networks(raw: &str) -> NetworksResult {
    let text = raw.trim();
    if text.is_empty() || text.contains("No networks available.") {
        return NetworksResult {
            ok: true,
            ..NetworksResult::default()
        };
    }
    if !text.contains("Available Networks:") {
        return NetworksResult {
            ok: false,
            networks: Vec::new(),
            message: text
                .split('\n')
                .next()
                .unwrap_or("")
                .trim_end_matches('\r')
                .to_string(),
        };
    }

    let mut networks: Vec<Network> = Vec::new();
    let mut current: Option<Network> = None;

    for line in text.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "Available Networks:" {
            continue;
        }

        if let Some(id) = trimmed.strip_prefix('-')
            && let Some(id) = id.trim_start().strip_prefix("ID:")
        {
            if let Some(done) = current.take() {
                networks.push(done);
            }
            current = Some(Network {
                id: id.trim().to_string(),
                ..Network::default()
            });
            continue;
        }
        let Some(network) = current.as_mut() else {
            continue;
        };
        // A resolved-address line, which carries no field we show.
        if trimmed.starts_with('[') && trimmed[1..].contains("]:") {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        if !key.chars().all(|c| c.is_ascii_alphabetic() || c == ' ') {
            continue;
        }
        let value = value.trim();
        match key.trim().to_lowercase().as_str() {
            "network" => {
                network.range = if value == "-" {
                    String::new()
                } else {
                    value.to_string()
                }
            }
            "domains" => network.domains = split_network_list(value),
            "status" => {
                network.status = value.to_string();
                network.selected = value.eq_ignore_ascii_case("selected");
            }
            _ => {}
        }
    }
    if let Some(done) = current {
        networks.push(done);
    }

    NetworksResult {
        ok: true,
        networks,
        message: String::new(),
    }
}

/// What a network row shows under its name: the range if it routes one, the
/// domains if it resolves them instead.
pub fn network_subtitle(network: &Network) -> String {
    if !network.range.is_empty() {
        return network.range.clone();
    }
    network.domains.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(raw: &str) -> StatusOk {
        match parse_netbird_status(raw) {
            StatusResult::Ok(status) => *status,
            other => panic!("expected a connected daemon, got {other:?}"),
        }
    }

    #[test]
    fn a_list_is_not_a_status_document() {
        assert!(matches!(parse_netbird_status("[]"), StatusResult::Error(_)));
        assert!(matches!(
            parse_netbird_status("nope"),
            StatusResult::Error(_)
        ));
        assert!(matches!(
            parse_netbird_status("  "),
            StatusResult::Unavailable(_)
        ));
    }

    #[test]
    fn every_state_that_means_go_and_log_in() {
        for state in ["NeedsLogin", "SessionExpired", "LoginFailed"] {
            let status = ok(&format!(r#"{{"daemonStatus":"{state}"}}"#));
            assert!(status.needs_login, "{state}");
            assert!(!status.running, "{state}");
        }
        assert!(ok(r#"{"daemonStatus":"Connected"}"#).running);
    }

    #[test]
    fn a_latency_in_nanoseconds_is_shown_in_milliseconds() {
        let status = ok(r#"{"daemonStatus":"Connected","peers":{"details":[
                {"fqdn":"box.netbird.cloud","status":"Connected","latency":25500000}]}}"#);
        assert_eq!(status.peers[0].latency_ms, Some(25.5));
    }

    #[test]
    fn an_unmeasured_latency_is_not_an_instant_one() {
        let status = ok(r#"{"daemonStatus":"Connected","peers":{"details":[
                {"fqdn":"box.netbird.cloud","status":"Connected"}]}}"#);
        assert_eq!(status.peers[0].latency_ms, Some(-1.0));
    }

    #[test]
    fn a_prefix_length_is_not_part_of_the_address() {
        let status = ok(
            r#"{"daemonStatus":"Connected","netbirdIp":"100.92.0.3/16","fqdn":"me.netbird.cloud"}"#,
        );
        assert_eq!(status.self_ip, "100.92.0.3");
        assert_eq!(status.self_name, "me");
        assert_eq!(status.self_peer.ipv4, ["100.92.0.3"]);
    }

    #[test]
    fn no_networks_is_an_answer_rather_than_a_failure() {
        let none = parse_netbird_networks("No networks available.");
        assert!(none.ok && none.networks.is_empty() && none.message.is_empty());
        assert!(parse_netbird_networks("").ok);

        let broken = parse_netbird_networks("Error: daemon not running\nsecond line");
        assert!(!broken.ok);
        assert_eq!(broken.message, "Error: daemon not running");
    }

    #[test]
    fn a_network_block_carries_its_range_domains_and_status() {
        let parsed = parse_netbird_networks(
            "Available Networks:\n\
             \n\
             - ID: office\n\
               Network: 10.0.0.0/24\n\
               Domains: -\n\
               Status: Selected\n\
             - ID: lab\n\
               Network: -\n\
               Domains: lab.example.com, other.example.com\n\
               [10.0.0.5]: resolved\n\
               Status: Not selected\n",
        );
        assert!(parsed.ok);
        assert_eq!(parsed.networks.len(), 2);

        let office = &parsed.networks[0];
        assert_eq!(office.id, "office");
        assert_eq!(office.range, "10.0.0.0/24");
        assert!(office.domains.is_empty(), "a dash is not a domain");
        assert!(office.selected);
        assert_eq!(network_subtitle(office), "10.0.0.0/24");

        let lab = &parsed.networks[1];
        assert_eq!(lab.range, "", "a dash is not a range");
        assert_eq!(lab.domains, ["lab.example.com", "other.example.com"]);
        assert!(!lab.selected);
        assert_eq!(network_subtitle(lab), "lab.example.com, other.example.com");
    }
}
