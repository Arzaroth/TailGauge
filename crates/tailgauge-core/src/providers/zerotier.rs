use super::{Authorization, Backend, Switch, argv};
use crate::network::Network;
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

    fn switch(&self) -> Switch {
        Switch::Networks
    }

    fn networks(&self) -> Option<Vec<String>> {
        Some(argv(&["zerotier-cli", "-j", "listnetworks"]))
    }

    fn parse_networks(&self, raw: &str) -> Vec<Network> {
        zt::parse_zerotier_networks(raw)
    }

    /// ZeroTier lists ids in lowercase hex and accepts them in any case.
    fn network_id(&self, id: &str) -> String {
        id.trim().to_ascii_lowercase()
    }

    fn select_network(&self, network_id: &str, selected: bool) -> Option<Vec<String>> {
        Some(argv(&[
            "zerotier-cli",
            if selected { "join" } else { "leave" },
            network_id,
        ]))
    }

    /// The service token is root's. A copy in the user's home is where the
    /// CLI looks next, so copying it there is the whole authorization. Only
    /// the read runs as root: a root write to a path in the user's home would
    /// follow whatever the user put there, a symlink into a root directory
    /// included.
    fn authorization(&self) -> Option<Authorization> {
        Some(Authorization {
            command: r#"t=$(mktemp "$HOME/.zeroTierOneAuthToken.XXXXXX") && { pkexec cat /var/lib/zerotier-one/authtoken.secret > "$t" && mv -fT "$t" "$HOME/.zeroTierOneAuthToken" || { rm -f "$t"; false; }; }"#,
            label: "Allow access to ZeroTier",
            sublabel: "Copy the ZeroTier service token to this user",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_read_of_the_token_runs_as_root() {
        let command = ZeroTier.authorization().expect("an authorization").command;
        let elevated: Vec<&str> = command
            .split("pkexec ")
            .skip(1)
            .map(|rest| rest.split(['>', '&', ';', '|']).next().unwrap_or("").trim())
            .collect();
        assert_eq!(
            elevated,
            ["cat /var/lib/zerotier-one/authtoken.secret"],
            "root never writes into the user's home, where a symlink could send it anywhere"
        );
        assert!(
            command.contains("mktemp"),
            "the copy is made private before it is filled"
        );
        assert!(
            command.contains("mv -fT"),
            "a planted link is replaced, not followed"
        );
    }
}
