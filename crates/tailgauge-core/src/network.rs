//! A network a provider joins or leaves on its own, shared by every provider
//! with networks (NetBird, ZeroTier).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Network {
    pub id: String,
    /// What the network is called, where the provider names it; the id
    /// otherwise.
    pub name: String,
    pub range: String,
    pub domains: Vec<String>,
    pub selected: bool,
    pub status: String,
}

/// What a network row shows under its name: the range if it routes one, the
/// domains if it resolves them instead, and its state when it has neither yet.
pub fn network_subtitle(network: &Network) -> String {
    if !network.range.is_empty() {
        return network.range.clone();
    }
    if !network.domains.is_empty() {
        return network.domains.join(", ");
    }
    network.status.clone()
}

/// A network's row label: its name, or its id where it has none.
pub fn network_label(network: &Network) -> String {
    if network.name.is_empty() {
        network.id.clone()
    } else {
        network.name.clone()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_network_left_stays_listed_to_be_joined_again() {
        let joined = crate::zerotier::parse_zerotier_networks(&format!(
            r#"[{{"nwid":"8056c2e21c000001","name":"lab","status":"{}","assignedAddresses":["10.1.0.2/24"]}}]"#,
            "OK"
        ));
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
}
