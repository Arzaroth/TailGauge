//! `tailgauge watch` - block until a VPN daemon reports a state change.
//!
//! The panels chain this ahead of a refresh, so a change made anywhere - tsui,
//! the CLI, another desktop - shows up at once instead of waiting for the next
//! poll. Tailscale pushes its changes on the IPN bus; every other installed
//! provider, and a Tailscale without the bus, is polled.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::tailscale;
use selvedge::proc;
use tailgauge_core::providers::{PROVIDERS, ProviderDescriptor};
use tailgauge_core::status::StatusResult;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Something moved. The caller refreshes on this and nothing else.
    Changed,
    /// The wait expired with nothing to report.
    Expired,
    /// No usable VPN CLI.
    Unusable,
}

impl Outcome {
    pub fn code(&self) -> u8 {
        match self {
            Outcome::Changed => 0,
            Outcome::Unusable => 1,
            Outcome::Expired => 2,
        }
    }
}

/// Notification fields worth waking a panel for. Everything else on the bus is
/// netmap churn, which would turn a refresh into a busy loop.
const INTERESTING: &[&str] = &[
    "\"State\"",
    "\"Prefs\"",
    "\"LoginFinished\"",
    "\"BackendState\"",
];

const TAILSCALE: &str = "tailscale";

pub fn run(timeout: Duration) -> Outcome {
    let installed: Vec<&'static ProviderDescriptor> = PROVIDERS
        .iter()
        .filter(|p| p.supported && proc::has(p.cli))
        .collect();
    if installed.is_empty() {
        return Outcome::Unusable;
    }

    // One deadline for the whole call. The bus can hang up at any point inside
    // its own wait, and starting the fallback on a fresh timeout would make a
    // frontend that asked for 300 seconds wait nearly 600.
    let deadline = Instant::now() + timeout;
    let (tx, rx) = mpsc::channel();

    let mut polled = installed.clone();
    let mut bus = None;
    if installed.iter().any(|p| p.id == TAILSCALE) && supports_watch_ipn() {
        bus = watch_ipn(tx.clone());
        if bus.is_some() {
            polled.retain(|p| p.id != TAILSCALE);
        }
    }
    poll(polled, deadline, tx.clone());

    loop {
        match rx.recv_timeout(remaining(deadline)) {
            // A bus that hangs up is not a state change; poll in its place
            // rather than spinning on it.
            Ok(Outcome::Unusable) => {
                if let Some(child) = bus.take() {
                    reap(child);
                }
                let tailscale = installed.iter().copied().filter(|p| p.id == TAILSCALE);
                poll(tailscale.collect(), deadline, tx.clone());
            }
            Ok(settled) => return settle(bus, settled),
            Err(_) => return settle(bus, Outcome::Expired),
        }
    }
}

fn settle(bus: Option<Child>, outcome: Outcome) -> Outcome {
    if let Some(child) = bus {
        reap(child);
    }
    outcome
}

/// What is left of the wait, never negative: `Instant - Instant` panics when
/// the deadline has already passed.
fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn supports_watch_ipn() -> bool {
    if proc::run_quiet(TAILSCALE, ["debug", "watch-ipn", "--help"]) {
        return true;
    }
    proc::run(TAILSCALE, ["debug", "--help"]).is_ok_and(|out| {
        let text = String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr);
        text.contains("watch-ipn")
    })
}

/// The IPN bus is the cheap path: tailscaled pushes a notification on every
/// state and prefs change, so this sits idle until something happens. It sends
/// `Changed` on one of those, `Unusable` when the stream ends without one.
fn watch_ipn(tx: Sender<Outcome>) -> Option<Child> {
    let mut child = Command::new(TAILSCALE)
        .args(["debug", "watch-ipn"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let Some(stdout) = child.stdout.take() else {
        reap(child);
        return None;
    };

    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if INTERESTING.iter().any(|key| line.contains(key)) {
                let _ = tx.send(Outcome::Changed);
                return;
            }
        }
        let _ = tx.send(Outcome::Unusable);
    });
    Some(child)
}

fn reap(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// For a provider with no bus, or a daemon too old to have one. Comparing only
/// the fields the panel renders keeps this from firing on netmap churn.
fn poll(providers: Vec<&'static ProviderDescriptor>, deadline: Instant, tx: Sender<Outcome>) {
    if providers.is_empty() {
        return;
    }
    thread::spawn(move || {
        let before = fingerprint(&providers);
        loop {
            let left = remaining(deadline);
            if left.is_zero() {
                return;
            }
            thread::sleep(Duration::from_secs(2).min(left));
            if fingerprint(&providers) != before {
                let _ = tx.send(Outcome::Changed);
                return;
            }
        }
    });
}

fn fingerprint(providers: &[&ProviderDescriptor]) -> Vec<Option<String>> {
    providers
        .iter()
        .map(|provider| {
            if provider.id == TAILSCALE {
                return tailscale::status().as_ref().map(fingerprint_of);
            }
            let argv = provider.status();
            let raw = proc::output(&argv[0], &argv[1..])?;
            Some(fingerprint_of_status(&provider.parse_status(&raw)))
        })
        .collect()
}

fn fingerprint_of(status: &serde_json::Value) -> String {
    format!(
        "{}|{}",
        tailscale::backend_state(status),
        status
            .pointer("/ExitNodeStatus/ID")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
    )
}

fn fingerprint_of_status(status: &StatusResult) -> String {
    match status {
        StatusResult::Ok(ok) => format!("{}|{}|{}", ok.daemon_state, ok.running, ok.needs_login),
        StatusResult::Unavailable(_) => "unavailable".into(),
        StatusResult::Error(_) => "error".into(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_exits_zero_and_an_expiry_does_not() {
        // The frontends refresh on 0 and only on 0, and back off on anything
        // that is not 0 or 2. The shell helper this replaced always reported
        // an expiry, so a change on the bus never reached a panel.
        assert_eq!(Outcome::Changed.code(), 0);
        assert_eq!(Outcome::Unusable.code(), 1);
        assert_eq!(Outcome::Expired.code(), 2);
    }

    #[test]
    fn only_the_fields_the_panel_renders_wake_it() {
        let wakes = |line: &str| INTERESTING.iter().any(|key| line.contains(key));
        assert!(wakes(r#"{"State":6}"#));
        assert!(wakes(r#"{"Prefs":{"WantRunning":true}}"#));
        assert!(wakes(r#"{"LoginFinished":{}}"#));
        assert!(!wakes(r#"{"NetMap":{"Peers":[]}}"#));
    }

    #[test]
    fn the_fingerprint_moves_with_the_connection_and_the_exit_node() {
        let state = |json: &str| fingerprint_of(&serde_json::from_str(json).expect("json"));

        let running = state(r#"{"BackendState":"Running"}"#);
        assert_ne!(running, state(r#"{"BackendState":"Stopped"}"#));

        // Routing through an exit node is a change the panel draws, and the
        // backend state does not move when one is picked.
        assert_ne!(
            running,
            state(r#"{"BackendState":"Running","ExitNodeStatus":{"ID":"n1"}}"#)
        );
        assert_ne!(
            state(r#"{"BackendState":"Running","ExitNodeStatus":{"ID":"n1"}}"#),
            state(r#"{"BackendState":"Running","ExitNodeStatus":{"ID":"n2"}}"#)
        );
    }

    #[test]
    fn a_netbird_that_connects_moves_its_fingerprint() {
        let fixture = |name: &str| {
            std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../tailgauge-core/tests/fixtures")
                    .join(name),
            )
            .expect("the capture")
        };
        let netbird = tailgauge_core::providers::provider_by_id("netbird").expect("netbird");
        assert_ne!(
            fingerprint_of_status(&netbird.parse_status(&fixture("netbird-status-idle.json"))),
            fingerprint_of_status(&netbird.parse_status(&fixture("netbird-status.json")))
        );
    }

    #[test]
    fn netmap_churn_is_not_a_change() {
        // The peers move constantly. Fingerprinting them would turn the
        // fallback into a busy loop that refreshes the panel every two seconds.
        let with_peers = r#"{"BackendState":"Running","Peer":{"a":{"Online":true}}}"#;
        let without = r#"{"BackendState":"Running"}"#;
        assert_eq!(
            fingerprint_of(&serde_json::from_str(with_peers).expect("json")),
            fingerprint_of(&serde_json::from_str(without).expect("json"))
        );
    }
}
