//! The networks of a provider switched by its networks, kept after they are
//! left. ZeroTier forgets a network the moment it is left, and the switch
//! that left them all has to know what to join again.
//!
//! State rather than cache: losing it strands the user with nothing to
//! rejoin, so it lives under `$XDG_STATE_HOME`, readable by the user alone
//! (it names every network joined, which the CLI shows only to holders of the
//! service token).

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
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

/// One change to the file at a time. Every panel on every monitor polls, and
/// `ctl` writes too: without the lock, a poll that read the file before a
/// leave could write its older list back over it.
fn update<T>(
    provider: &ProviderDescriptor,
    change: impl FnOnce(&mut Vec<Known>) -> T,
) -> io::Result<(Vec<Known>, T)> {
    let path = file(provider);
    let dir = path.parent().expect("a state directory");
    DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    let lock = File::create(path.with_extension("lock"))?;
    lock.lock()?;
    // Only a missing file is a fresh start. One that cannot be read or parsed
    // still holds networks ZeroTier has forgotten, so it is left alone.
    let before: Vec<Known> = match fs::read_to_string(&path) {
        Ok(raw) => {
            serde_json::from_str(&raw).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    let mut after = before.clone();
    let answer = change(&mut after);
    if after != before {
        let staged = path.with_extension(format!("tmp.{}", std::process::id()));
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&staged)?
            .write_all(&serde_json::to_vec(&after)?)?;
        fs::rename(&staged, &path)?;
    }
    Ok((after, answer))
}

/// What a poll saw joined, added to what is known: a network seen for the
/// first time is wanted, and a known one keeps its `wanted` and gains the
/// name the provider has learned. A poll never decides what is wanted: its
/// snapshot can be older than the leave it would undo.
fn observed(known: &mut Vec<Known>, joined: &[Network]) {
    for network in joined.iter().filter(|n| !n.id.is_empty()) {
        match known.iter_mut().find(|k| k.id == network.id) {
            Some(k) if !network.name.is_empty() => k.name = network.name.clone(),
            Some(_) => {}
            None => known.push(Known {
                id: network.id.clone(),
                name: network.name.clone(),
                wanted: true,
            }),
        }
    }
}

fn wanted(known: &mut Vec<Known>, id: &str, wanted: bool) {
    match known.iter_mut().find(|k| k.id == id) {
        Some(k) => k.wanted = wanted,
        None => known.push(Known {
            id: id.to_string(),
            name: String::new(),
            wanted,
        }),
    }
}

/// What a panel poll saw joined, remembered; the whole list, oldest first.
/// The list is still answered when the write fails: the poll only costs the
/// memory then.
pub fn observe(provider: &ProviderDescriptor, joined: &[Network]) -> Vec<Known> {
    update(provider, |known| observed(known, joined))
        .map(|(known, _)| known)
        .unwrap_or_else(|_| {
            let mut known = Vec::new();
            observed(&mut known, joined);
            known
        })
}

/// What the switch is about to leave, remembered as wanted. An error means
/// nothing may be left: the switch could not bring it back.
pub fn take(provider: &ProviderDescriptor, joined: &[Network]) -> io::Result<Vec<Known>> {
    update(provider, |known| {
        observed(known, joined);
        for network in joined {
            wanted(known, &network.id, true);
        }
    })
    .map(|(known, _)| known)
}

/// The user joined or left this network by hand: the switch brings it back
/// next time, or leaves it out.
pub fn want(provider: &ProviderDescriptor, network_id: &str, want: bool) -> io::Result<()> {
    update(provider, |known| wanted(known, network_id, want)).map(|_| ())
}

/// What turning the switch on joins: every wanted network not joined yet.
pub fn to_rejoin<'a>(known: &'a [Known], joined: &[Network]) -> Vec<&'a Known> {
    known
        .iter()
        .filter(|k| k.wanted && !joined.iter().any(|j| j.id == k.id))
        .collect()
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
    fn a_poll_adds_what_it_sees_and_never_wants_what_was_left() {
        let mut list = vec![known("a", false)];
        observed(&mut list, &[network("a", "lab"), network("b", "")]);
        assert_eq!(
            list,
            [
                Known {
                    id: "a".into(),
                    name: "lab".into(),
                    wanted: false
                },
                known("b", true)
            ],
            "a snapshot older than the leave does not undo it"
        );
        observed(&mut list, &[network("a", "")]);
        assert_eq!(list[0].name, "lab", "an unnamed answer keeps the name");
    }

    #[test]
    fn leaving_one_never_seen_still_remembers_it() {
        let mut list = Vec::new();
        wanted(&mut list, "c", false);
        assert_eq!(list, [known("c", false)]);
        wanted(&mut list, "c", true);
        assert_eq!(list, [known("c", true)]);
    }

    #[test]
    fn a_state_file_that_cannot_be_read_is_never_overwritten() {
        let dir = std::env::temp_dir().join(format!("tg-networks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        // SAFETY: this test alone reads XDG_STATE_HOME in this process's tests.
        unsafe { std::env::set_var("XDG_STATE_HOME", &dir) };
        let provider = tailgauge_core::providers::provider_by_id("zerotier").expect("zerotier");
        let path = file(provider);
        fs::create_dir_all(path.parent().expect("a dir")).expect("the dir");
        fs::write(&path, "{ not json").expect("a damaged file");

        assert!(take(provider, &[network("a", "lab")]).is_err());
        assert!(want(provider, "a", false).is_err());
        assert_eq!(observe(provider, &[network("a", "lab")]).len(), 1);
        assert_eq!(
            fs::read_to_string(&path).expect("still there"),
            "{ not json",
            "what ZeroTier forgot is still in it"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_switch_brings_back_what_it_took_and_not_what_was_left_by_hand() {
        let all = [known("a", true), known("b", false)];
        let ids = |v: Vec<&Known>| v.into_iter().map(|k| k.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(to_rejoin(&all, &[])), ["a"]);
        assert!(to_rejoin(&all, &[network("a", "")]).is_empty());
        assert!(
            to_rejoin(&[known("a", false)], &[]).is_empty(),
            "a network left by hand stays left"
        );
    }
}
