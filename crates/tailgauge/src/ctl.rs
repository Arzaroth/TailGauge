//! `tailgauge ctl` - driving tailscaled from a key binding or a script.
//!
//! Both panels watch the IPN bus, so whatever this changes shows up in them at
//! once. That is the whole mechanism: there is no socket to talk to, which is
//! why binding `tailgauge-ctl toggle` to a key is enough.

use std::io::{BufRead, BufReader, IsTerminal};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

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

pub fn status() -> Outcome {
    let Some(status) = tailscale::status() else {
        println!("Tailscale is not answering");
        return Outcome::Disconnected;
    };

    let state = tailscale::backend_state(&status);
    if state != "Running" {
        println!(
            "{}",
            match state {
                "NeedsLogin" => "Needs login",
                "" => "Disconnected",
                other => other,
            }
        );
        return Outcome::Disconnected;
    }

    println!(
        "Connected as {} ({})",
        tailscale::self_host_name(&status).unwrap_or("unknown"),
        tailscale::self_ip(&status).unwrap_or("no address")
    );
    if let Some(node) = tailscale::current_exit_node() {
        println!("Exit node: {node}");
    }
    Outcome::Ok
}

pub fn up() -> Outcome {
    if std::io::stdout().is_terminal() {
        return match Command::new("tailscale").arg("up").status() {
            Ok(s) if s.success() => Outcome::Ok,
            _ => Outcome::Failed("tailscale up failed".into()),
        };
    }

    // Bound to a key there is no terminal for `tailscale up` to print its
    // login URL on, so it is scraped out of the stream the way the panels do.
    if let Some(url) = first_login_url() {
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

    if tailscale::status().is_some_and(|s| tailscale::running(&s)) {
        return Outcome::Ok;
    }
    announce(
        "critical",
        "Could not turn Tailscale on",
        "Run tailgauge-ctl up for the reason",
    );
    Outcome::Failed("could not turn Tailscale on".into())
}

/// Run `tailscale up` to completion, returning the first login URL it printed
/// on either stream.
fn first_login_url() -> Option<String> {
    let mut child = Command::new("tailscale")
        .arg("up")
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

pub fn down() -> Outcome {
    if launch::run_quiet("tailscale", ["down"]) {
        return Outcome::Ok;
    }
    announce(
        "critical",
        "Could not turn Tailscale off",
        "Run tailgauge-ctl down for the reason",
    );
    Outcome::Failed("could not turn Tailscale off".into())
}

pub fn toggle() -> Outcome {
    let Some(status) = tailscale::status() else {
        return Outcome::Failed("tailscale is not answering".into());
    };
    // Not `on && down || up`: a failed `down` would fall through and turn it
    // back on again.
    if tailscale::running(&status) {
        down()
    } else {
        up()
    }
}

pub fn exit_node(target: Option<&str>) -> Outcome {
    let Some(target) = target else {
        println!(
            "{}",
            tailscale::current_exit_node().unwrap_or_else(|| "none".into())
        );
        return Outcome::Ok;
    };

    let resolved = match target.to_lowercase().as_str() {
        "off" | "none" | "clear" => "",
        _ => target,
    };

    if launch::run_quiet("tailscale", ["set", &format!("--exit-node={resolved}")]) {
        return Outcome::Ok;
    }
    announce("critical", "Could not set the exit node", target);
    Outcome::Failed(format!("could not set the exit node to {target}"))
}

pub fn exit_nodes() -> Outcome {
    match launch::run("tailscale", ["exit-node", "list"]) {
        Ok(out) => {
            print!("{}", String::from_utf8_lossy(&out.stdout));
            if out.status.success() {
                Outcome::Ok
            } else {
                Outcome::Failed("tailscale exit-node list failed".into())
            }
        }
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
