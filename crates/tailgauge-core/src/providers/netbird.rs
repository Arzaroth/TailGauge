use std::path::PathBuf;

use super::{Backend, argv};
use crate::status::StatusResult;

#[derive(Debug)]
pub struct NetBird;

impl Backend for NetBird {
    fn status(&self) -> Vec<String> {
        argv(&["netbird", "status", "--json"])
    }

    fn daemon_socket(&self, env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
        let addr = ["NB_DAEMON_ADDR", "WT_DAEMON_ADDR"]
            .into_iter()
            .find_map(|key| env(key).filter(|a| !a.is_empty()))
            .unwrap_or_else(|| "unix:///var/run/netbird.sock".to_string());
        addr.strip_prefix("unix://").map(PathBuf::from)
    }

    fn instance_socket_dir(&self) -> Option<&'static str> {
        Some("/var/run/netbird")
    }

    fn parse_status(&self, raw: &str) -> StatusResult {
        crate::netbird::parse_netbird_status(raw)
    }

    fn up(&self) -> Option<Vec<String>> {
        Some(argv(&["netbird", "up"]))
    }

    /// An environment variable rather than a flag: a NetBird older than
    /// `--no-browser` ignores it instead of failing.
    fn unattended_up_env(&self) -> &'static [(&'static str, &'static str)] {
        &[("NB_NO_BROWSER", "true")]
    }

    fn down(&self) -> Option<Vec<String>> {
        Some(argv(&["netbird", "down"]))
    }

    fn networks(&self) -> Option<Vec<String>> {
        Some(argv(&["netbird", "networks", "list"]))
    }

    fn parse_networks(&self, raw: &str) -> Vec<crate::netbird::Network> {
        crate::netbird::parse_netbird_networks(raw).networks
    }

    fn select_network(&self, network_id: &str, selected: bool) -> Option<Vec<String>> {
        Some(argv(&[
            "netbird",
            "networks",
            if selected { "select" } else { "deselect" },
            network_id,
        ]))
    }
}
