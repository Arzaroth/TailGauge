use super::{Authorization, Backend, argv};
use crate::netbird::Network;
use crate::status::StatusResult;
use crate::zerotier::{self as zt, ACCESS_DENIED};

#[derive(Debug)]
pub struct ZeroTier;

impl Backend for ZeroTier {
    fn status(&self) -> Vec<String> {
        argv(&["zerotier-cli", "-j", "info"])
    }

    /// Three calls, read as one answer. A refusal from any of them is the
    /// user's to fix rather than a service that is down.
    fn read_status(
        &self,
        run: &dyn Fn(&[String]) -> Option<std::process::Output>,
    ) -> Option<String> {
        let mut answers = Vec::new();
        for command in ["info", "listnetworks", "peers"] {
            let out = run(&argv(&["zerotier-cli", "-j", command]))?;
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return zt::is_access_denied(&stdout, &stderr).then(|| ACCESS_DENIED.to_string());
            }
            answers.push(stdout);
        }
        Some(zt::envelope(&answers[0], &answers[1], &answers[2]))
    }

    fn parse_status(&self, raw: &str) -> StatusResult {
        zt::parse_zerotier_status(raw)
    }

    fn up(&self) -> Option<Vec<String>> {
        None
    }

    fn down(&self) -> Option<Vec<String>> {
        None
    }

    fn networks(&self) -> Option<Vec<String>> {
        Some(argv(&["zerotier-cli", "-j", "listnetworks"]))
    }

    fn parse_networks(&self, raw: &str) -> Vec<Network> {
        zt::parse_zerotier_networks(raw)
    }

    fn select_network(&self, network_id: &str, selected: bool) -> Option<Vec<String>> {
        Some(argv(&[
            "zerotier-cli",
            if selected { "join" } else { "leave" },
            network_id,
        ]))
    }

    /// The service token is root's. A copy in the user's home is where the
    /// CLI looks next, so copying it there is the whole authorization.
    fn authorization(&self) -> Option<Authorization> {
        Some(Authorization {
            command: r#"pkexec install -m 600 -o "$(id -un)" -g "$(id -gn)" /var/lib/zerotier-one/authtoken.secret "$HOME/.zeroTierOneAuthToken""#,
            label: "Allow access to ZeroTier",
            sublabel: "Copy the ZeroTier service token to this user",
        })
    }
}
