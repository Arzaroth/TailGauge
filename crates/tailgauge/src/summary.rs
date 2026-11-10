//! Plain `tailgauge`: which provider is up, whether it is connected, the exit
//! node, and the machines on the network, as a few lines of text.

use std::io::{self, Write};

use tailgauge_core::Peer;
use tailgauge_core::bar::provider_state_word;
use tailgauge_core::fmt::{compact_minutes, minutes_since};
use tailgauge_core::panel_state::PanelState;
use tailgauge_core::providers;

/// Colours for a terminal, or none.
pub struct Ink {
    dim: &'static str,
    good: &'static str,
    warn: &'static str,
    here: &'static str,
    reset: &'static str,
}

impl Ink {
    pub fn new(styled: bool) -> Self {
        if styled {
            Ink {
                dim: "\x1b[2m",
                good: "\x1b[32m",
                warn: "\x1b[33m",
                here: "\x1b[36m",
                reset: "\x1b[0m",
            }
        } else {
            Ink {
                dim: "",
                good: "",
                warn: "",
                here: "",
                reset: "",
            }
        }
    }
}

/// Machines unseen for longer are counted rather than listed: a tailnet keeps
/// every device it ever had.
const STALE_MINUTES: i64 = 30 * 24 * 60;

fn name_of(peer: &Peer) -> &str {
    [&peer.display_name, &peer.host_name, &peer.dns_name]
        .into_iter()
        .find(|n| !n.is_empty())
        .map_or("?", |n| n.trim_end_matches('.'))
}

pub fn render(state: &PanelState, now_ms: i64, ink: &Ink, out: &mut impl Write) -> io::Result<()> {
    let Ink {
        dim,
        good,
        warn,
        here,
        reset,
    } = ink;
    let Some(active) = providers::active_provider(state) else {
        return writeln!(
            out,
            "no VPN client found: tailgauge drives tailscale, netbird and zerotier"
        );
    };
    let summary = |id: &str| {
        state
            .summaries
            .as_ref()
            .and_then(|all| all.iter().find(|s| s.id == id))
    };
    let others: Vec<String> = providers::installed_providers(state)
        .into_iter()
        .filter(|p| p.id != active.id)
        .map(|p| {
            format!(
                "{} installed, {}",
                p.id,
                provider_state_word(summary(p.id)).to_lowercase()
            )
        })
        .collect();
    let others = if others.is_empty() {
        String::new()
    } else {
        format!(" {dim}({}){reset}", others.join("; "))
    };
    writeln!(out, "{dim}provider{reset}   {}{others}", active.id)?;

    let word = provider_state_word(summary(active.id));
    let tone = if state.running {
        good
    } else if state.needs_login {
        warn
    } else {
        dim
    };
    writeln!(out, "{dim}status{reset}     {tone}{word}{reset}")?;
    let exit = state
        .peers
        .iter()
        .find(|p| p.exit_node)
        .map_or("none", name_of);
    writeln!(out, "{dim}exit node{reset}  {exit}")?;

    let mut rows: Vec<(String, String, String, &str)> = Vec::new();
    if let Some(me) = state.self_peer.as_ref() {
        let name = if state.self_name.is_empty() {
            name_of(me).to_owned()
        } else {
            state.self_name.clone()
        };
        rows.push((name, state.self_ip.clone(), "this machine".into(), here));
    }
    let mut stale = 0;
    let mut peers: Vec<&Peer> = state.peers.iter().filter(|p| !p.mullvad).collect();
    peers.sort_by(|a, b| b.online.cmp(&a.online).then(name_of(a).cmp(name_of(b))));
    for peer in peers {
        let ip = peer.ipv4.first().cloned().unwrap_or_default();
        let (word, tone) = if peer.online {
            ("online".to_owned(), *good)
        } else {
            match peer
                .last_seen
                .as_deref()
                .and_then(|seen| minutes_since(seen, now_ms))
            {
                Some(m) if m < STALE_MINUTES => (format!("idle {}", compact_minutes(m)), *dim),
                _ => {
                    stale += 1;
                    continue;
                }
            }
        };
        rows.push((name_of(peer).to_owned(), ip, word, tone));
    }
    if rows.is_empty() {
        return Ok(());
    }
    writeln!(out)?;
    let wide = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(0);
    let wide_ip = rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(0);
    for (name, ip, word, tone) in rows {
        writeln!(out, "{name:wide$}  {ip:wide_ip$}  {tone}{word}{reset}")?;
    }
    if stale > 0 {
        let machines = if stale == 1 { "machine" } else { "machines" };
        writeln!(
            out,
            "{dim}and {stale} {machines} not seen for 30 days{reset}"
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tailgauge_core::panel_state::{PanelState, ProviderState};

    fn peer(name: &str, last_seen: &str) -> Peer {
        Peer {
            host_name: name.into(),
            ipv4: vec!["100.64.0.9".into()],
            last_seen: Some(last_seen.into()),
            ..Peer::default()
        }
    }

    #[test]
    fn the_cutoff_is_thirty_days_and_a_clock_ahead_is_now() {
        // 2026-11-02T12:00:00Z
        let now = 1_793_620_800_000;
        let state = PanelState {
            providers: Some(vec![ProviderState {
                id: "tailscale".into(),
                installed: true,
            }]),
            running: true,
            peers: vec![
                peer("ahead", "2026-11-02T12:00:05Z"),
                peer("edge", "2026-10-03T12:00:00Z"),
                peer("inside", "2026-10-03T12:01:00Z"),
            ],
            ..PanelState::default()
        };
        let mut out = Vec::new();
        render(&state, now, &Ink::new(false), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();

        assert!(text.contains("ahead   100.64.0.9  idle now"), "{text}");
        assert!(text.contains("inside  100.64.0.9  idle 29d"), "{text}");
        assert!(!text.contains("edge "), "{text}");
        assert!(
            text.ends_with("and 1 machine not seen for 30 days\n"),
            "{text}"
        );
    }
}
