use super::{Backend, argv};
use crate::status::StatusResult;

#[derive(Debug)]
pub struct Tailscale;

impl Backend for Tailscale {
    fn status(&self) -> Vec<String> {
        argv(&["tailscale", "status", "--json"])
    }

    fn parse_status(&self, raw: &str) -> StatusResult {
        crate::status::parse_status(raw)
    }

    fn up(&self) -> Vec<String> {
        argv(&["tailscale", "up"])
    }

    fn down(&self) -> Vec<String> {
        argv(&["tailscale", "down"])
    }

    fn exit_node_list(&self) -> Option<Vec<String>> {
        Some(argv(&["tailscale", "exit-node", "list"]))
    }

    fn accounts(&self) -> Option<Vec<String>> {
        Some(argv(&["tailscale", "switch", "--list", "--json"]))
    }

    fn events(&self) -> Option<Vec<String>> {
        Some(argv(&["tailscale", "debug", "watch-ipn"]))
    }

    fn event_markers(&self) -> &'static [&'static str] {
        &[
            "\"State\"",
            "\"Prefs\"",
            "\"LoginFinished\"",
            "\"BackendState\"",
        ]
    }

    fn switch_account(&self, account_id: &str) -> Option<Vec<String>> {
        Some(argv(&["tailscale", "switch", account_id]))
    }

    fn set_exit_node(&self, target: &str) -> Option<Vec<String>> {
        Some(vec![
            "tailscale".into(),
            "set".into(),
            format!("--exit-node={target}"),
        ])
    }
}
