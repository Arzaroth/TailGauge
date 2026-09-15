//! `tailgauge ctl` - driving tailscaled from a key binding or a script.
//!
//! Both panels watch the IPN bus, so whatever this changes shows up in them at
//! once. That is the whole mechanism: there is no socket to talk to, which is
//! why binding `tailgauge-ctl toggle` to a key is enough.

use std::io::{BufRead, BufReader, IsTerminal};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use tailgauge_core as core;
use tailgauge_core::providers::{Capability, ProviderDescriptor};

use crate::launch;
use crate::notify::{self, Notification};
use crate::tailscale;

/// `status` answers a script, so it reports the connection rather than a fault.
pub const EXIT_DISCONNECTED: u8 = 3;

pub enum Outcome {
    Ok,
    Disconnected,
    Failed(String),
}

/// Nothing to report to on a terminal - the command already printed there.
/// Bound to a key there is no terminal at all, and a notification is the only
/// way a failure reaches anyone.
fn announce(urgency: &str, summary: &str, body: &str) {
    if std::io::stdout().is_terminal() {
        return;
    }
    let _ = notify::run(&Notification {
        summary,
        body,
        urgency,
        image: None,
        open: None,
    });
}

/// The provider's own status, through whichever parser it needs.
fn read(provider: &ProviderDescriptor) -> core::StatusResult {
    let argv = provider.status();
    let raw = launch::output(&argv[0], &argv[1..]).unwrap_or_default();
    if provider.id == "netbird" {
        core::parse_netbird_status(&raw)
    } else {
        core::parse_status(&raw)
    }
}

pub fn status(provider: &ProviderDescriptor) -> Outcome {
    let core::StatusResult::Ok(status) = read(provider) else {
        println!("{} is not answering", provider.label);
        return Outcome::Disconnected;
    };

    if !status.running {
        println!("{}", idle_line(&status.daemon_state));
        return Outcome::Disconnected;
    }

    println!("{}", connected_line(&status.self_name, &status.self_ip));
    if provider.capabilities.has(Capability::ExitNodes)
        && let Some(node) = tailscale::current_exit_node()
    {
        println!("Exit node: {node}");
    }
    Outcome::Ok
}

/// What `status` says when the daemon is not carrying traffic. A script reads
/// the exit code; a person reads this.
fn idle_line(daemon_state: &str) -> &str {
    match daemon_state {
        "NeedsLogin" => "Needs login",
        "" | "Unknown" => "Disconnected",
        other => other,
    }
}

fn connected_line(self_name: &str, self_ip: &str) -> String {
    let name = if self_name.is_empty() {
        "unknown"
    } else {
        self_name
    };
    let ip = if self_ip.is_empty() {
        "no address"
    } else {
        self_ip
    };
    format!("Connected as {name} ({ip})")
}

pub fn up(provider: &ProviderDescriptor) -> Outcome {
    let argv = provider.up();
    if std::io::stdout().is_terminal() {
        return match Command::new(&argv[0]).args(&argv[1..]).status() {
            Ok(s) if s.success() => Outcome::Ok,
            _ => Outcome::Failed(format!("{} failed", argv.join(" "))),
        };
    }

    // Bound to a key there is no terminal for `tailscale up` to print its
    // login URL on, so it is scraped out of the stream the way the panels do.
    if let Some(url) = first_login_url(&argv) {
        announce(
            "normal",
            "Authorize this device",
            "Opening the Tailscale login page",
        );
        let _ = Command::new("xdg-open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }

    if matches!(read(provider), core::StatusResult::Ok(status) if status.running) {
        return Outcome::Ok;
    }
    announce(
        "critical",
        &format!("Could not turn {} on", provider.label),
        "Run tailgauge ctl up for the reason",
    );
    Outcome::Failed(format!("could not turn {} on", provider.label))
}

/// Run `tailscale up` to completion, returning the first login URL it printed
/// on either stream.
fn first_login_url(argv: &[String]) -> Option<String> {
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let (tx, rx) = mpsc::channel();
    for stream in [
        child.stdout.take().map(readable),
        child.stderr.take().map(readable),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        thread::spawn(move || {
            for line in stream.lines().map_while(Result::ok) {
                if let Some(url) = login_url(&line) {
                    let _ = tx.send(url);
                }
            }
        });
    }
    drop(tx);

    let first = rx.iter().next();
    let _ = child.wait();
    first
}

fn readable(pipe: impl std::io::Read + Send + 'static) -> BufReader<Box<dyn std::io::Read + Send>> {
    BufReader::new(Box::new(pipe))
}

fn login_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let rest = &line[start..];
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

pub fn down(provider: &ProviderDescriptor) -> Outcome {
    let argv = provider.down();
    if launch::run_quiet(&argv[0], &argv[1..]) {
        return Outcome::Ok;
    }
    announce(
        "critical",
        &format!("Could not turn {} off", provider.label),
        "Run tailgauge ctl down for the reason",
    );
    Outcome::Failed(format!("could not turn {} off", provider.label))
}

pub fn toggle(provider: &ProviderDescriptor) -> Outcome {
    let core::StatusResult::Ok(status) = read(provider) else {
        return Outcome::Failed(format!("{} is not answering", provider.label));
    };
    // Not `on && down || up`: a failed `down` would fall through and turn it
    // back on again.
    if status.running {
        down(provider)
    } else {
        up(provider)
    }
}

/// The address a row's peer is reached at.
///
/// The frontend hands back the payload the model gave it rather than working
/// the address out itself: a Mullvad node is set by address and a tailnet one
/// by name, and that is the model's rule to keep.
pub fn target_of(peer_json: &str) -> Result<String, String> {
    let peer: core::Peer =
        serde_json::from_str(peer_json).map_err(|e| format!("--peer is not a peer: {e}"))?;
    Ok(if peer.exit_node {
        // Already the exit node, so the click is a disconnection.
        String::new()
    } else {
        core::panel::exit_node_target(&peer)
    })
}

pub fn address_of(peer_json: &str) -> Result<String, String> {
    let peer: core::Peer =
        serde_json::from_str(peer_json).map_err(|e| format!("--peer is not a peer: {e}"))?;
    Ok(core::panel::peer_address(&peer))
}

pub fn exit_node(provider: &ProviderDescriptor, target: Option<&str>) -> Outcome {
    if !provider.capabilities.has(Capability::ExitNodes) {
        return Outcome::Failed(format!("{} has no exit nodes", provider.label));
    }
    let Some(target) = target else {
        println!(
            "{}",
            tailscale::current_exit_node().unwrap_or_else(|| "none".into())
        );
        return Outcome::Ok;
    };

    let Some(argv) = provider.set_exit_node(requested_node(target)) else {
        return Outcome::Failed(format!("{} cannot set an exit node", provider.label));
    };
    if launch::run_quiet(&argv[0], &argv[1..]) {
        return Outcome::Ok;
    }
    announce("critical", "Could not set the exit node", target);
    Outcome::Failed(format!("could not set the exit node to {target}"))
}

/// The empty target is how every provider is told to stop using an exit node,
/// and the three words for it are what a person types at a prompt.
fn requested_node(target: &str) -> &str {
    match target.to_lowercase().as_str() {
        "off" | "none" | "clear" => "",
        _ => target,
    }
}

pub fn switch_account(provider: &ProviderDescriptor, account_id: &str) -> Outcome {
    let Some(argv) = provider.switch_account(account_id) else {
        return Outcome::Failed(format!("{} has no profiles to switch", provider.label));
    };
    if launch::run_quiet(&argv[0], &argv[1..]) {
        return Outcome::Ok;
    }
    announce("critical", "Could not switch profile", account_id);
    Outcome::Failed(format!("could not switch to {account_id}"))
}

pub fn select_network(provider: &ProviderDescriptor, network_id: &str, join: bool) -> Outcome {
    let Some(argv) = provider.select_network(network_id, join) else {
        return Outcome::Failed(format!("{} has no networks", provider.label));
    };
    if launch::run_quiet(&argv[0], &argv[1..]) {
        return Outcome::Ok;
    }
    let what = if join { "join" } else { "leave" };
    announce("critical", &format!("Could not {what} {network_id}"), "");
    Outcome::Failed(format!("could not {what} {network_id}"))
}

/// Let this user operate the daemon's profile.
///
/// `pkexec` because it needs root, and the user name is resolved by the shell
/// that runs it rather than read here: a widget started without one in its
/// environment would otherwise authorize nobody.
pub fn authorize(provider: &ProviderDescriptor) -> Outcome {
    if !provider.capabilities.has(Capability::Accounts) {
        return Outcome::Failed(format!("{} has no profiles to operate", provider.label));
    }
    match Command::new("sh")
        .arg("-c")
        .arg(authorize_command(provider.cli))
        .status()
    {
        Ok(status) if status.success() => Outcome::Ok,
        _ => {
            announce(
                "critical",
                &format!("Could not authorize the {} operator", provider.label),
                "Run tailgauge ctl authorize for the reason",
            );
            Outcome::Failed("could not authorize the operator".into())
        }
    }
}

fn authorize_command(cli: &str) -> String {
    format!(r#"pkexec {cli} set --operator="$(id -un)""#)
}

pub fn exit_nodes(provider: &ProviderDescriptor) -> Outcome {
    let Some(argv) = provider.exit_node_list() else {
        return Outcome::Failed(format!("{} has no exit nodes", provider.label));
    };
    match launch::run(&argv[0], &argv[1..]) {
        Ok(out) => {
            print!("{}", String::from_utf8_lossy(&out.stdout));
            if out.status.success() {
                Outcome::Ok
            } else {
                Outcome::Failed(format!("{} failed", argv.join(" ")))
            }
        }
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tailgauge_core::providers::provider_by_id;

    fn tailscale() -> &'static ProviderDescriptor {
        provider_by_id("tailscale").expect("tailscale is a provider")
    }

    fn netbird() -> &'static ProviderDescriptor {
        provider_by_id("netbird").expect("netbird is a provider")
    }

    fn why(outcome: Outcome) -> String {
        match outcome {
            Outcome::Failed(why) => why,
            Outcome::Ok => panic!("it went ahead"),
            Outcome::Disconnected => panic!("it reported a connection"),
        }
    }

    /// What a frontend hands back is what the panel gave it, so the cases are
    /// built the same way rather than written out by hand.
    fn payload(peer: &core::Peer) -> String {
        serde_json::to_string(peer).expect("a peer serializes")
    }

    fn tailnet_peer() -> core::Peer {
        core::Peer {
            id: "n1".into(),
            host_name: "box".into(),
            dns_name: "box.tail.ts.net.".into(),
            display_name: "box".into(),
            ipv4: vec!["100.64.0.1".into()],
            online: true,
            exit_node_option: true,
            ..core::Peer::default()
        }
    }

    fn mullvad_peer() -> core::Peer {
        core::Peer {
            id: "n2".into(),
            host_name: "de-ber-wg-001".into(),
            dns_name: "de-ber-wg-001.mullvad.ts.net.".into(),
            display_name: "de-ber-wg-001".into(),
            ipv4: vec!["100.64.0.9".into()],
            online: true,
            exit_node_option: true,
            mullvad: true,
            ..core::Peer::default()
        }
    }

    #[test]
    fn the_login_url_is_scraped_out_of_whatever_surrounds_it() {
        assert_eq!(login_url("To authenticate, visit:\n").as_deref(), None);
        assert_eq!(
            login_url("\thttps://login.tailscale.com/a/1234abcd").as_deref(),
            Some("https://login.tailscale.com/a/1234abcd")
        );
        assert_eq!(
            login_url("visit https://login.tailscale.com/a/1234abcd to log in").as_deref(),
            Some("https://login.tailscale.com/a/1234abcd"),
            "the URL must not swallow the rest of the sentence"
        );
    }

    #[test]
    fn a_daemon_that_is_not_carrying_traffic_says_why() {
        assert_eq!(idle_line("NeedsLogin"), "Needs login");
        assert_eq!(idle_line(""), "Disconnected");
        assert_eq!(idle_line("Unknown"), "Disconnected");
        // Anything else the daemon calls itself is worth printing as it stands
        // rather than flattening into "Disconnected".
        assert_eq!(idle_line("Starting"), "Starting");
    }

    #[test]
    fn a_connection_missing_its_name_or_address_still_reads_as_a_sentence() {
        assert_eq!(
            connected_line("box", "100.64.0.1"),
            "Connected as box (100.64.0.1)"
        );
        assert_eq!(connected_line("", ""), "Connected as unknown (no address)");
    }

    #[test]
    fn the_words_for_no_exit_node_all_mean_the_empty_target() {
        for word in ["off", "none", "clear", "OFF", "None"] {
            assert_eq!(requested_node(word), "", "{word}");
        }
        assert_eq!(requested_node("de-ber-wg-001"), "de-ber-wg-001");
    }

    #[test]
    fn the_operator_is_the_user_running_the_command_rather_than_the_one_here() {
        // Resolved by the shell that runs it: a widget started without a user
        // in its environment would otherwise authorize nobody.
        let command = authorize_command("tailscale");
        assert!(command.starts_with("pkexec tailscale set --operator="));
        assert!(command.contains("$(id -un)"));
    }

    #[test]
    fn a_peer_that_is_already_the_exit_node_resolves_to_a_disconnection() {
        let mut routing = tailnet_peer();
        routing.exit_node = true;
        assert_eq!(target_of(&payload(&routing)).as_deref(), Ok(""));
        assert_eq!(
            target_of(&payload(&tailnet_peer())).as_deref(),
            Ok("box.tail.ts.net")
        );
    }

    #[test]
    fn a_mullvad_node_is_routed_through_by_address_and_a_tailnet_one_by_name() {
        // A Mullvad node is not in the peer list under a name the CLI will
        // take back, so its address is the only handle on it.
        assert_eq!(
            target_of(&payload(&mullvad_peer())).as_deref(),
            Ok("100.64.0.9")
        );
        assert_eq!(
            address_of(&payload(&mullvad_peer())).as_deref(),
            Ok("de-ber-wg-001.mullvad.ts.net"),
            "the address it is shown and copied under is still the name"
        );
        assert_eq!(
            address_of(&payload(&tailnet_peer())).as_deref(),
            Ok("box.tail.ts.net")
        );
    }

    #[test]
    fn a_payload_that_is_not_a_peer_is_refused_rather_than_guessed_at() {
        for bad in ["not json", "[]", "{}", "null"] {
            assert!(target_of(bad).is_err(), "{bad}");
            assert!(address_of(bad).is_err(), "{bad}");
        }
    }

    /// Each of these returns before anything is spawned, which is what keeps
    /// one provider's command off another provider's daemon.
    #[test]
    fn a_provider_is_never_asked_for_what_it_does_not_have() {
        assert_eq!(
            why(exit_node(netbird(), Some("de-ber-wg-001"))),
            "NetBird has no exit nodes"
        );
        assert_eq!(why(exit_nodes(netbird())), "NetBird has no exit nodes");
        assert_eq!(
            why(switch_account(netbird(), "acct-1")),
            "NetBird has no profiles to switch"
        );
        assert_eq!(
            why(authorize(netbird())),
            "NetBird has no profiles to operate"
        );
        assert_eq!(
            why(select_network(tailscale(), "net-1", true)),
            "Tailscale has no networks"
        );
    }
}
