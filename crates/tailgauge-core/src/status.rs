//! `tailscale status --json`, turned into the shape the panel reads.

use serde::Serialize;
use serde_json::Value;

use crate::peer::{self, Peer};

/// What a status call produced. The unavailable case is not a failure: a
/// daemon that is not answering is a state the panel draws, not an error it
/// reports.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum StatusResult {
    Ok(Box<StatusOk>),
    Unavailable(StatusUnavailable),
    Error(StatusError),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusOk {
    pub ok: bool,
    pub unavailable: bool,
    #[serde(rename = "daemonState")]
    pub daemon_state: String,
    pub running: bool,
    #[serde(rename = "needsLogin")]
    pub needs_login: bool,
    #[serde(rename = "authUrl")]
    pub auth_url: String,
    #[serde(rename = "selfName")]
    pub self_name: String,
    #[serde(rename = "selfDnsName")]
    pub self_dns_name: String,
    #[serde(rename = "selfIp")]
    pub self_ip: String,
    #[serde(rename = "selfUserId")]
    pub self_user_id: String,
    #[serde(rename = "selfPeer")]
    pub self_peer: Peer,
    #[serde(rename = "fileSharing")]
    pub file_sharing: bool,
    pub peers: Vec<Peer>,
    #[serde(rename = "exitNodes")]
    pub exit_nodes: Vec<Peer>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusUnavailable {
    pub ok: bool,
    pub unavailable: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StatusError {
    pub ok: bool,
    pub unavailable: bool,
    pub message: String,
    pub error: String,
}

const FILE_SHARING_CAP: &str = "https://tailscale.com/cap/file-sharing";

pub fn parse_status(raw: &str) -> StatusResult {
    let text = raw.trim();
    if text.is_empty() {
        return StatusResult::Unavailable(StatusUnavailable {
            ok: true,
            unavailable: true,
            message: "Disconnected".into(),
        });
    }

    let Ok(data) = serde_json::from_str::<Value>(text) else {
        return StatusResult::Error(StatusError {
            ok: false,
            unavailable: true,
            message: "Status error".into(),
            error: "Failed to parse tailscale status".into(),
        });
    };

    let daemon_state = match peer::field(&data, "BackendState") {
        s if s.is_empty() => "Unknown".to_string(),
        s => s,
    };
    let own = data.get("Self").cloned().unwrap_or(Value::Null);
    // Normalized the same way as every peer, so the local machine can carry the
    // same copy options without a second shape to keep in step.
    let users = peer::users_by_id(data.get("User"));
    let mut self_peer = peer_from_status("self", &own, &users);
    if self_peer.ipv4.is_empty() && self_peer.ipv6.is_empty() {
        let ips = peer::strings(&data, "TailscaleIPs");
        self_peer.ipv4 = peer::filter_ipv4(&ips);
        self_peer.ipv6 = peer::filter_ipv6(&ips);
    }

    let mut peers: Vec<Peer> = Vec::new();
    let mut exit_nodes: Vec<Peer> = Vec::new();
    if let Some(raw_peers) = data.get("Peer").and_then(Value::as_object) {
        for (id, raw_peer) in raw_peers {
            let normalized = peer_from_status(id, raw_peer, &users);
            if normalized.mullvad {
                continue;
            }
            // An exit node that is not up is not a route anywhere.
            if normalized.online && normalized.exit_node_option {
                exit_nodes.push(normalized.clone());
            }
            peers.push(normalized);
        }
    }

    // Online first, each half alphabetical. A machine that is up is the one
    // being looked for; one that is asleep still has an address worth copying.
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| peer::collate(&a.host_name, &b.host_name))
    });
    exit_nodes.sort_by(|a, b| peer::collate(&a.host_name, &b.host_name));

    StatusResult::Ok(Box::new(StatusOk {
        ok: true,
        unavailable: false,
        running: daemon_state == "Running",
        needs_login: daemon_state == "NeedsLogin",
        daemon_state,
        auth_url: peer::field(&data, "AuthURL"),
        self_name: self_peer.display_name.clone(),
        self_dns_name: self_peer.dns_name.clone(),
        self_ip: self_peer.ipv4.first().cloned().unwrap_or_default(),
        self_user_id: peer::field(&own, "UserID"),
        file_sharing: has_file_sharing(&own),
        self_peer,
        peers,
        exit_nodes,
    }))
}

pub fn peer_from_status(
    id: &str,
    raw: &Value,
    users: &std::collections::BTreeMap<String, String>,
) -> Peer {
    let host_name = peer::field(raw, "HostName");
    let dns_name = peer::field(raw, "DNSName");
    let display = peer::display_host_name(&host_name, &dns_name);
    let ips = peer::strings(raw, "TailscaleIPs");
    let cur_addr = peer::field(raw, "CurAddr");
    let relay = peer::field(raw, "Relay");

    Peer {
        id: id.to_string(),
        host_name: display.clone(),
        user_id: Some(peer::field(raw, "UserID")),
        user_name: Some(peer::peer_owner(raw, users)),
        taildrop_target: Some(peer::number(raw, "TaildropTarget").unwrap_or(0)),
        dns_name: peer::clean_dns_name(&dns_name),
        display_name: display,
        ipv4: peer::filter_ipv4(&ips),
        ipv6: peer::filter_ipv6(&ips),
        online: raw.get("Online") == Some(&Value::Bool(true)),
        os: peer::field(raw, "OS"),
        tags: peer::strings(raw, "Tags"),
        exit_node_option: raw.get("ExitNodeOption") == Some(&Value::Bool(true)),
        exit_node: raw.get("ExitNode") == Some(&Value::Bool(true)),
        mullvad: peer::is_mullvad_peer(raw),
        // A direct address means the traffic is not going through a relay.
        connection_type: Some(if !cur_addr.is_empty() {
            "P2P".to_string()
        } else if !relay.is_empty() {
            "Relayed".to_string()
        } else {
            String::new()
        }),
        latency_ms: Some(-1.0),
        endpoint: Some(cur_addr),
        relay: Some(relay),
        rx_bytes: Some(peer::number(raw, "RxBytes").unwrap_or(0)),
        tx_bytes: Some(peer::number(raw, "TxBytes").unwrap_or(0)),
        last_handshake: Some(peer::non_zero_time(raw, "LastHandshake")),
        last_seen: Some(peer::non_zero_time(raw, "LastSeen")),
        created: Some(peer::non_zero_time(raw, "Created")),
        public_key: Some(peer::field(raw, "PublicKey")),
        routes: Some(peer::strings(raw, "PrimaryRoutes")),
        ..Peer::default()
    }
}

fn has_file_sharing(own: &Value) -> bool {
    if own
        .get("CapMap")
        .and_then(Value::as_object)
        .is_some_and(|caps| caps.contains_key(FILE_SHARING_CAP))
    {
        return true;
    }
    peer::strings(own, "Capabilities")
        .iter()
        .any(|cap| cap == FILE_SHARING_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(raw: &str) -> StatusOk {
        match parse_status(raw) {
            StatusResult::Ok(status) => *status,
            other => panic!("expected a running tailnet, got {other:?}"),
        }
    }

    #[test]
    fn nothing_on_stdout_is_a_daemon_that_is_not_answering() {
        assert_eq!(
            parse_status("   "),
            StatusResult::Unavailable(StatusUnavailable {
                ok: true,
                unavailable: true,
                message: "Disconnected".into(),
            })
        );
    }

    #[test]
    fn output_that_is_not_json_is_an_error_rather_than_a_state() {
        assert_eq!(
            parse_status("tailscale: command not found"),
            StatusResult::Error(StatusError {
                ok: false,
                unavailable: true,
                message: "Status error".into(),
                error: "Failed to parse tailscale status".into(),
            })
        );
    }

    #[test]
    fn a_self_with_no_peer_addresses_falls_back_to_the_document() {
        // tailscaled reports the machine's own addresses at the top level on
        // some versions and inside Self on others.
        let status = ok(
            r#"{"BackendState":"Running","TailscaleIPs":["100.64.0.1"],"Self":{"HostName":"box"}}"#,
        );
        assert_eq!(status.self_ip, "100.64.0.1");
        assert_eq!(status.self_peer.ipv4, ["100.64.0.1"]);
    }

    #[test]
    fn a_mullvad_peer_is_not_a_machine_on_the_tailnet() {
        let status = ok(r#"{"BackendState":"Running","Peer":{
                "a":{"HostName":"de-ber-wg-001.mullvad.ts.net","Online":true,"ExitNodeOption":true},
                "b":{"HostName":"laptop","Online":true}}}"#);
        assert_eq!(status.peers.len(), 1);
        assert_eq!(status.peers[0].host_name, "laptop");
        assert!(status.exit_nodes.is_empty(), "and not an exit node either");
    }

    #[test]
    fn an_offline_exit_node_is_not_a_route_anywhere() {
        let status = ok(r#"{"BackendState":"Running","Peer":{
                "a":{"HostName":"up","Online":true,"ExitNodeOption":true},
                "b":{"HostName":"down","Online":false,"ExitNodeOption":true}}}"#);
        assert_eq!(status.peers.len(), 2);
        assert_eq!(
            status
                .exit_nodes
                .iter()
                .map(|p| p.host_name.as_str())
                .collect::<Vec<_>>(),
            ["up"]
        );
    }

    #[test]
    fn online_machines_come_first_and_each_half_is_alphabetical() {
        let status = ok(r#"{"BackendState":"Running","Peer":{
                "1":{"HostName":"zulu","Online":true},
                "2":{"HostName":"alpha","Online":false},
                "3":{"HostName":"bravo","Online":true}}}"#);
        assert_eq!(
            status
                .peers
                .iter()
                .map(|p| p.host_name.as_str())
                .collect::<Vec<_>>(),
            ["bravo", "zulu", "alpha"]
        );
    }

    #[test]
    fn file_sharing_is_read_from_either_shape_the_daemon_writes() {
        let cap_map = ok(
            r#"{"BackendState":"Running","Self":{"CapMap":{"https://tailscale.com/cap/file-sharing":null}}}"#,
        );
        assert!(cap_map.file_sharing);

        let list = ok(
            r#"{"BackendState":"Running","Self":{"Capabilities":["https://tailscale.com/cap/file-sharing"]}}"#,
        );
        assert!(list.file_sharing);

        assert!(!ok(r#"{"BackendState":"Running","Self":{}}"#).file_sharing);
    }

    #[test]
    fn the_zero_time_is_never_rather_than_the_year_one() {
        let status = ok(r#"{"BackendState":"Running","Peer":{"a":{"HostName":"box",
                "LastHandshake":"0001-01-01T00:00:00Z","LastSeen":"2026-09-14T10:00:00Z"}}}"#);
        assert_eq!(status.peers[0].last_handshake.as_deref(), Some(""));
        assert_eq!(
            status.peers[0].last_seen.as_deref(),
            Some("2026-09-14T10:00:00Z")
        );
    }

    #[test]
    fn an_unknown_backend_state_is_named_rather_than_blank() {
        assert_eq!(ok("{}").daemon_state, "Unknown");
        assert!(!ok("{}").running);
        assert!(ok(r#"{"BackendState":"NeedsLogin"}"#).needs_login);
    }
}
