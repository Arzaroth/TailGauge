//! `tailgauge watch` - block until tailscaled reports a state change.
//!
//! The panels chain this ahead of a refresh, so a change made anywhere - tsui,
//! the CLI, another desktop - shows up at once instead of waiting for the next
//! poll.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::launch;
use crate::tailscale;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Something moved. The caller refreshes on this and nothing else.
    Changed,
    /// The wait expired with nothing to report.
    Expired,
    /// No usable tailscale.
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

pub fn run(timeout: Duration) -> Outcome {
    if !tailscale::installed() {
        return Outcome::Unusable;
    }

    if supports_watch_ipn() {
        match watch_ipn(timeout) {
            // A bus that hangs up immediately is not a state change; fall back
            // rather than spinning on it.
            Outcome::Unusable => {}
            settled => return settled,
        }
    }

    poll_fallback(timeout)
}

fn supports_watch_ipn() -> bool {
    if launch::run_quiet("tailscale", ["debug", "watch-ipn", "--help"]) {
        return true;
    }
    launch::run("tailscale", ["debug", "--help"]).is_ok_and(|out| {
        let text = String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr);
        text.contains("watch-ipn")
    })
}

/// The IPN bus is the cheap path: tailscaled pushes a notification on every
/// state and prefs change, so this sits idle until something happens.
fn watch_ipn(timeout: Duration) -> Outcome {
    let Ok(mut child) = Command::new("tailscale")
        .args(["debug", "watch-ipn"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Outcome::Unusable;
    };

    let Some(stdout) = child.stdout.take() else {
        return reap(child, Outcome::Unusable);
    };

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if INTERESTING.iter().any(|key| line.contains(key)) {
                let _ = tx.send(Outcome::Changed);
                return;
            }
        }
        // The stream ended without saying anything: a bus we cannot use.
        let _ = tx.send(Outcome::Unusable);
    });

    let settled = match rx.recv_timeout(timeout) {
        Ok(outcome) => outcome,
        Err(mpsc::RecvTimeoutError::Timeout) => Outcome::Expired,
        Err(mpsc::RecvTimeoutError::Disconnected) => Outcome::Unusable,
    };
    reap(child, settled)
}

fn reap(mut child: Child, outcome: Outcome) -> Outcome {
    let _ = child.kill();
    let _ = child.wait();
    outcome
}

/// Older daemons have no watch-ipn, and a broken bus should not turn the panel
/// into a busy loop. Comparing only the fields the panel renders keeps this
/// from firing on netmap churn.
fn poll_fallback(timeout: Duration) -> Outcome {
    let before = fingerprint();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(2).min(deadline - Instant::now()));
        if fingerprint() != before {
            return Outcome::Changed;
        }
    }
    Outcome::Expired
}

fn fingerprint() -> Option<String> {
    tailscale::status().as_ref().map(fingerprint_of)
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
