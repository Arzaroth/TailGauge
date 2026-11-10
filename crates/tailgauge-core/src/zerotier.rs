//! ZeroTier: `zerotier-cli -j info`, `-j listnetworks` and `-j peers`, read
//! together as one status.
//!
//! ZeroTier has no connection to bring up or down: its service always runs,
//! and each network is joined or left on its own. On means joined to at least
//! one network, which is what TailGauge's switch leaves and rejoins. Peers
//! carry no names on this side of the controller, so a machine is its
//! ZeroTier address.

use serde_json::{Value, json};

use crate::netbird::Network;
use crate::peer::{self, Peer};
use crate::status::{StatusError, StatusOk, StatusResult, StatusUnavailable};

/// What the binary hands the parser when the CLI refuses this user: the
/// service token is root's until it is copied to the user.
pub const ACCESS_DENIED: &str = r#"{"accessDenied":true}"#;

/// The three answers as one document, which is what `parse_zerotier_status`
/// reads. An answer that is not JSON is kept as null rather than failing the
/// other two.
pub fn envelope(info: &str, networks: &str, peers: &str) -> String {
    let read = |raw: &str| serde_json::from_str::<Value>(raw).unwrap_or(Value::Null);
    json!({
        "info": read(info),
        "networks": read(networks),
        "peers": read(peers),
    })
    .to_string()
}

/// Whether a failed call failed because this user may not ask: no readable
/// token (`authtoken.secret not found or readable`), or a token the service
/// does not accept (`401`).
pub fn is_access_denied(stdout: &str, stderr: &str) -> bool {
    let text = format!("{stdout}\n{stderr}");
    text.contains("authtoken.secret") || text.lines().any(|l| l.trim_start().starts_with("401 "))
}

/// A network's state in words. `None` for an unknown state, shown as the
/// service spells it.
fn network_state(status: &str) -> Option<&'static str> {
    Some(match status {
        "OK" => "Connected",
        "REQUESTING_CONFIGURATION" => "Requesting configuration",
        "ACCESS_DENIED" => "Waiting for authorization",
        "AUTHENTICATION_REQUIRED" => "Needs login",
        "NOT_FOUND" => "No such network",
        "PORT_ERROR" => "Could not create its interface",
        "CLIENT_TOO_OLD" => "ZeroTier is too old for it",
        _ => return None,
    })
}

fn zerotier_network(raw: &Value) -> Network {
    let status = peer::field(raw, "status");
    let up = status == "OK";
    let addresses = peer::strings(raw, "assignedAddresses");
    Network {
        id: peer::field(raw, "nwid"),
        name: peer::field(raw, "name"),
        // Addresses only once the network is up: a network still waiting
        // says what it waits for instead.
        range: if up {
            addresses.join(", ")
        } else {
            String::new()
        },
        domains: Vec::new(),
        selected: true,
        status: network_state(&status).map(str::to_string).unwrap_or(status),
    }
}

/// `zerotier-cli -j listnetworks`: every network this node has joined.
pub fn parse_zerotier_networks(raw: &str) -> Vec<Network> {
    serde_json::from_str::<Value>(raw.trim())
        .ok()
        .as_ref()
        .and_then(Value::as_array)
        .map(|networks| networks.iter().map(zerotier_network).collect())
        .unwrap_or_default()
}

/// What a network row list shows: the networks joined, then the ones this
/// user joined before and has since left, so the switch and the rows can
/// bring them back. ZeroTier forgets a network the moment it is left.
pub fn with_left_networks(joined: &[Network], remembered: &[Network]) -> Vec<Network> {
    let mut rows = joined.to_vec();
    for network in remembered {
        if !joined.iter().any(|j| j.id == network.id) {
            rows.push(Network {
                selected: false,
                range: String::new(),
                domains: Vec::new(),
                status: "Not joined".into(),
                ..network.clone()
            });
        }
    }
    rows
}

/// The path the CLI calls DIRECT: the one the node prefers, while it is
/// alive. Without one the peer is reached through a root.
fn preferred_path(raw: &Value) -> Option<&Value> {
    raw.get("paths")?.as_array()?.iter().find(|path| {
        path.get("preferred") == Some(&Value::Bool(true))
            && path.get("active") != Some(&Value::Bool(false))
            && path.get("expired") != Some(&Value::Bool(true))
    })
}

/// A peer the node can reach: it has a live path, or a latency measured
/// through a root. ZeroTier forgets a peer it cannot reach, and the ones it
/// lists without either are infrastructure it has heard of, not machines on
/// the user's networks.
fn reachable(raw: &Value) -> bool {
    let latency = raw.get("latency").and_then(Value::as_f64).unwrap_or(-1.0);
    latency >= 0.0
        || raw
            .get("paths")
            .and_then(Value::as_array)
            .is_some_and(|paths| {
                paths
                    .iter()
                    .any(|p| p.get("active") == Some(&Value::Bool(true)))
            })
}

fn zerotier_peer(raw: &Value) -> Peer {
    let address = peer::field(raw, "address");
    let latency = raw.get("latency").and_then(Value::as_f64).unwrap_or(-1.0);
    let path = preferred_path(raw);
    Peer {
        id: address.clone(),
        host_name: address.clone(),
        display_name: address,
        online: true,
        connection_type: Some(if path.is_some() { "P2P" } else { "Relayed" }.into()),
        latency_ms: Some(latency),
        endpoint: Some(path.map(|p| peer::field(p, "address")).unwrap_or_default()),
        relay: Some(String::new()),
        ..Peer::default()
    }
}

/// ZeroTier assigns from whatever range the network's owner picked, so an
/// address is told apart by its family rather than by a known prefix.
fn of_family(addresses: &[String], v4: bool) -> Vec<String> {
    addresses
        .iter()
        .filter(|a| {
            a.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_ipv4() == v4)
        })
        .cloned()
        .collect()
}

pub fn parse_zerotier_status(raw: &str) -> StatusResult {
    let text = raw.trim();
    if text.is_empty() {
        return StatusResult::Unavailable(StatusUnavailable {
            ok: true,
            unavailable: true,
            message: "Disconnected".into(),
            access_denied: false,
        });
    }

    let failed = || {
        StatusResult::Error(StatusError {
            ok: false,
            unavailable: true,
            message: "Status error".into(),
            error: "Failed to parse zerotier status".into(),
        })
    };
    let Ok(data) = serde_json::from_str::<Value>(text) else {
        return failed();
    };
    if data.get("accessDenied") == Some(&Value::Bool(true)) {
        return StatusResult::Unavailable(StatusUnavailable {
            ok: true,
            unavailable: true,
            message: "ZeroTier refuses this user".into(),
            access_denied: true,
        });
    }
    let Some(info) = data.get("info").filter(|i| i.is_object()) else {
        return failed();
    };

    let raw_networks: Vec<Value> = data
        .get("networks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let up = raw_networks
        .iter()
        .any(|n| peer::field(n, "status") == "OK");
    let running = !raw_networks.is_empty();
    let login = raw_networks
        .iter()
        .find(|n| peer::field(n, "status") == "AUTHENTICATION_REQUIRED");
    let online = info.get("online") == Some(&Value::Bool(true));
    let daemon_state = if !running {
        "Stopped"
    } else if up {
        "Running"
    } else if login.is_some() {
        "NeedsLogin"
    } else if !online {
        "Offline"
    } else {
        "Waiting"
    }
    .to_string();

    let addresses: Vec<String> = raw_networks
        .iter()
        .filter(|n| peer::field(n, "status") == "OK")
        .flat_map(|n| peer::strings(n, "assignedAddresses"))
        .map(|a| a.split('/').next().unwrap_or("").to_string())
        .collect();
    let address = peer::field(info, "address");
    let self_peer = Peer {
        id: address.clone(),
        host_name: address.clone(),
        display_name: address.clone(),
        ipv4: of_family(&addresses, true),
        ipv6: of_family(&addresses, false),
        online: up,
        ..Peer::default()
    };

    let mut peers: Vec<Peer> = data
        .get("peers")
        .and_then(Value::as_array)
        .map(|peers| {
            peers
                .iter()
                // Roots and moons carry the network rather than sit on it.
                .filter(|p| peer::field(p, "role") == "LEAF" && reachable(p))
                .map(zerotier_peer)
                .collect()
        })
        .unwrap_or_default();
    peers.sort_by(|a, b| peer::collate(&a.host_name, &b.host_name));

    StatusResult::Ok(Box::new(StatusOk {
        ok: true,
        unavailable: false,
        running,
        needs_login: login.is_some(),
        daemon_state,
        auth_url: login
            .map(|n| peer::field(n, "authenticationURL"))
            .unwrap_or_default(),
        self_name: address,
        self_dns_name: String::new(),
        self_ip: self_peer.ipv4.first().cloned().unwrap_or_default(),
        self_user_id: String::new(),
        self_peer,
        file_sharing: false,
        peers,
        exit_nodes: Vec::new(),
        exit_node_id: String::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(status: &str) -> String {
        format!(
            r#"[{{"nwid":"8056c2e21c000001","name":"lab","status":"{status}","assignedAddresses":["10.1.0.2/24","fd00::2/88"],"authenticationURL":"https://sso.example/zt"}}]"#
        )
    }

    fn status(networks: &str) -> StatusOk {
        match parse_zerotier_status(&envelope(
            r#"{"address":"a1b2c3d4e5","online":true}"#,
            networks,
            "[]",
        )) {
            StatusResult::Ok(ok) => *ok,
            other => panic!("expected a status, got {other:?}"),
        }
    }

    #[test]
    fn on_means_joined_to_a_network() {
        let up = status(&network("OK"));
        assert!(up.running);
        assert_eq!(up.self_ip, "10.1.0.2");
        assert_eq!(up.self_peer.ipv6, ["fd00::2"]);
        assert_eq!(up.self_name, "a1b2c3d4e5");

        let waiting = status(&network("ACCESS_DENIED"));
        assert!(
            waiting.running,
            "joined is on, so the network shows what it waits for"
        );
        assert_eq!(waiting.daemon_state, "Waiting");
        assert!(waiting.self_ip.is_empty(), "no address until it is up");

        let none = status("[]");
        assert!(!none.running);
        assert_eq!(none.daemon_state, "Stopped");
    }

    #[test]
    fn a_network_behind_sso_is_a_login_with_its_url() {
        let sso = status(&network("AUTHENTICATION_REQUIRED"));
        assert!(sso.needs_login);
        assert_eq!(sso.auth_url, "https://sso.example/zt");
        assert!(!status(&network("OK")).needs_login);
    }

    #[test]
    fn a_network_says_what_it_waits_for_until_it_is_up() {
        let up = &parse_zerotier_networks(&network("OK"))[0];
        assert_eq!(up.name, "lab");
        assert_eq!(up.range, "10.1.0.2/24, fd00::2/88");
        assert!(up.selected);

        let waiting = &parse_zerotier_networks(&network("ACCESS_DENIED"))[0];
        assert_eq!(waiting.range, "");
        assert_eq!(waiting.status, "Waiting for authorization");

        let odd = &parse_zerotier_networks(&network("SOMETHING_NEW"))[0];
        assert_eq!(odd.status, "SOMETHING_NEW");
        assert!(parse_zerotier_networks("not json").is_empty());
    }

    #[test]
    fn a_network_left_stays_listed_to_be_joined_again() {
        let joined = parse_zerotier_networks(&network("OK"));
        let office = Network {
            id: "8056c2e21c000002".into(),
            name: "office".into(),
            selected: true,
            ..Network::default()
        };
        let rows = with_left_networks(&joined, &[joined[0].clone(), office]);
        assert_eq!(
            rows.iter()
                .map(|n| (n.name.as_str(), n.selected))
                .collect::<Vec<_>>(),
            [("lab", true), ("office", false)]
        );
        assert_eq!(rows[1].status, "Not joined");
    }

    #[test]
    fn a_refused_user_and_a_stopped_service_are_not_errors() {
        assert!(matches!(
            parse_zerotier_status(ACCESS_DENIED),
            StatusResult::Unavailable(StatusUnavailable {
                access_denied: true,
                ..
            })
        ));
        assert!(matches!(
            parse_zerotier_status(""),
            StatusResult::Unavailable(StatusUnavailable {
                access_denied: false,
                ..
            })
        ));
        assert!(matches!(
            parse_zerotier_status("[]"),
            StatusResult::Error(_)
        ));
    }

    #[test]
    fn the_cli_s_refusals_are_told_apart_from_a_service_that_is_down() {
        assert!(is_access_denied(
            "",
            "zerotier-cli: authtoken.secret not found or readable in /var/lib/zerotier-one (try again as root)"
        ));
        assert!(is_access_denied("401 info {}", ""));
        assert!(!is_access_denied(
            "Error connecting to the ZeroTier service: connection failed",
            ""
        ));
    }

    #[test]
    fn only_reachable_leaves_are_machines_and_a_preferred_path_is_direct() {
        let peers = r#"[
            {"address":"cafe04eba9","role":"PLANET","latency":18,"paths":[]},
            {"address":"0654069b79","role":"LEAF","latency":3,"paths":[
                {"active":true,"expired":false,"preferred":false,"address":"172.23.0.4/9993"},
                {"active":true,"expired":false,"preferred":true,"address":"172.23.0.5/9993"}]},
            {"address":"1d798a3cf9","role":"LEAF","latency":-1,"paths":[]},
            {"address":"2a7c11ee90","role":"LEAF","latency":40,"paths":[
                {"active":true,"expired":false,"preferred":false,"address":"203.0.113.7/9993"}]}
        ]"#;
        let StatusResult::Ok(ok) = parse_zerotier_status(&envelope(
            r#"{"address":"a1b2c3d4e5","online":true}"#,
            &network("OK"),
            peers,
        )) else {
            panic!("expected a status");
        };
        assert_eq!(
            ok.peers.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["0654069b79", "2a7c11ee90"],
            "a peer with no path and no latency is not a machine"
        );
        let direct = &ok.peers[0];
        assert!(direct.online);
        assert_eq!(direct.connection_type.as_deref(), Some("P2P"));
        assert_eq!(direct.endpoint.as_deref(), Some("172.23.0.5/9993"));
        let relayed = &ok.peers[1];
        assert_eq!(relayed.connection_type.as_deref(), Some("Relayed"));
        assert_eq!(relayed.endpoint.as_deref(), Some(""));
    }
}
