//! The networks of a provider switched by its networks, kept after they are
//! left. ZeroTier forgets a network the moment it is left, and the switch
//! that left them all has to know what to join again.
//!
//! State rather than cache: losing it strands the user with nothing to
//! rejoin, so it lives under `$XDG_STATE_HOME`.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tailgauge_core::Network;
use tailgauge_core::providers::ProviderDescriptor;

/// A network this user has joined. `wanted` holds until the user leaves it on
/// its own, so turning the switch on brings back what turning it off took
/// away, and not what the user left by hand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Known {
    pub id: String,
    pub name: String,
    pub wanted: bool,
}

impl Default for Known {
    fn default() -> Self {
        Known {
            id: String::new(),
            name: String::new(),
            wanted: true,
        }
    }
}

impl Known {
    pub fn network(&self) -> Network {
        Network {
            id: self.id.clone(),
            name: self.name.clone(),
            ..Network::default()
        }
    }
}

fn state_dir() -> PathBuf {
    match std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
        Some(v) => PathBuf::from(v),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join(".local")
            .join("state"),
    }
    .join("tailgauge")
}

fn file(provider: &ProviderDescriptor) -> PathBuf {
    state_dir().join(format!("networks-{}.json", provider.id))
}

fn read(provider: &ProviderDescriptor) -> Vec<Known> {
    fs::read_to_string(file(provider))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Losing the write only costs the memory, so a failure to write is not one.
fn write(provider: &ProviderDescriptor, known: &[Known]) {
    let path = file(provider);
    let staged = path.with_extension(format!("tmp.{}", std::process::id()));
    let written = path
        .parent()
        .is_some_and(|dir| fs::create_dir_all(dir).is_ok())
        && serde_json::to_vec(known)
            .ok()
            .is_some_and(|bytes| fs::write(&staged, bytes).is_ok());
    if written {
        let _ = fs::rename(&staged, &path);
    }
}

/// What is joined now, added to what was known: a joined network is wanted,
/// and keeps the name the provider has learned for it since.
fn merged(known: &[Known], joined: &[Network]) -> Vec<Known> {
    let mut after = known.to_vec();
    for network in joined.iter().filter(|n| !n.id.is_empty()) {
        match after.iter_mut().find(|k| k.id == network.id) {
            Some(k) => {
                k.wanted = true;
                if !network.name.is_empty() {
                    k.name = network.name.clone();
                }
            }
            None => after.push(Known {
                id: network.id.clone(),
                name: network.name.clone(),
                wanted: true,
            }),
        }
    }
    after
}

/// Add what is joined now to what is remembered, and return the whole list,
/// oldest first.
pub fn remember(provider: &ProviderDescriptor, joined: &[Network]) -> Vec<Known> {
    let before = read(provider);
    let after = merged(&before, joined);
    if after != before {
        write(provider, &after);
    }
    after
}

/// The user left this network on its own: keep it listed, but leave it out
/// when the switch comes back on.
pub fn unwant(provider: &ProviderDescriptor, network_id: &str) {
    let mut known = read(provider);
    if let Some(k) = known.iter_mut().find(|k| k.id == network_id && k.wanted) {
        k.wanted = false;
        write(provider, &known);
    }
}

/// What turning the switch on joins: every wanted network not joined yet.
/// With none wanted, every network known, so the switch is never a click
/// that does nothing while there is something it could join.
pub fn to_rejoin<'a>(known: &'a [Known], joined: &[Network]) -> Vec<&'a Known> {
    let absent = |k: &&Known| !joined.iter().any(|j| j.id == k.id);
    let wanted: Vec<&Known> = known.iter().filter(|k| k.wanted).collect();
    if wanted.is_empty() {
        known.iter().filter(absent).collect()
    } else {
        wanted.into_iter().filter(absent).collect()
    }
}

/// The networks joined right now, or `None` when the provider did not
/// answer.
pub fn joined(provider: &ProviderDescriptor) -> Option<Vec<Network>> {
    let argv = provider.backend.networks()?;
    let out = crate::gather::run_bounded(&argv, crate::gather::READ_LIMIT)?;
    out.status.success().then(|| {
        provider
            .backend
            .parse_networks(&String::from_utf8_lossy(&out.stdout))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(id: &str, name: &str) -> Network {
        Network {
            id: id.into(),
            name: name.into(),
            ..Network::default()
        }
    }

    fn known(id: &str, wanted: bool) -> Known {
        Known {
            id: id.into(),
            name: String::new(),
            wanted,
        }
    }

    #[test]
    fn a_joined_network_is_remembered_wanted_and_keeps_its_latest_name() {
        let after = merged(
            &[known("a", false)],
            &[network("a", "lab"), network("b", "")],
        );
        assert_eq!(
            after,
            [
                Known {
                    id: "a".into(),
                    name: "lab".into(),
                    wanted: true
                },
                known("b", true)
            ]
        );
        assert_eq!(merged(&after, &[network("a", "")])[0].name, "lab");
    }

    #[test]
    fn the_switch_brings_back_what_it_took_and_not_what_was_left_by_hand() {
        let all = [known("a", true), known("b", false)];
        let ids = |v: Vec<&Known>| v.into_iter().map(|k| k.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(to_rejoin(&all, &[])), ["a"]);
        assert!(to_rejoin(&all, &[network("a", "")]).is_empty());

        let none_wanted = [known("a", false), known("b", false)];
        assert_eq!(
            ids(to_rejoin(&none_wanted, &[])),
            ["a", "b"],
            "a switch with nothing wanted still joins what it knows"
        );
        assert!(to_rejoin(&[], &[]).is_empty());
    }
}
