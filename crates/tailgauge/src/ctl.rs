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
        println!(
            "{}",
            match status.daemon_state.as_str() {
                "NeedsLogin" => "Needs login",
                "" | "Unknown" => "Disconnected",
                other => other,
            }
        );
        return Outcome::Disconnected;
    }

    println!(
        "Connected as {} ({})",
        if status.self_name.is_empty() {
            "unknown"
        } else {
            &status.self_name
        },
        if status.self_ip.is_empty() {
            "no address"
        } else {
            &status.self_ip
        }
    );
    if provider.capabilities.has(Capability::ExitNodes)
        && let Some(node) = tailscale::current_exit_node()
    {
        println!("Exit node: {node}");
    }
    Outcome::Ok
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

    let resolved = match target.to_lowercase().as_str() {
        "off" | "none" | "clear" => "",
        _ => target,
    };

    let Some(argv) = provider.set_exit_node(resolved) else {
        return Outcome::Failed(format!("{} cannot set an exit node", provider.label));
    };
    if launch::run_quiet(&argv[0], &argv[1..]) {
        return Outcome::Ok;
    }
    announce("critical", "Could not set the exit node", target);
    Outcome::Failed(format!("could not set the exit node to {target}"))
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
