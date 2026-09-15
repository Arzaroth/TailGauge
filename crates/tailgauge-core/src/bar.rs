//! What the bar icon says, and what hovering it answers.
//!
//! Deliberately aggregate: switching which provider the panel *shows* must not
//! change an icon that describes the machine's connections.

use serde::Serialize;

use crate::panel_state::{PanelState, ProviderSummary};
use crate::providers::{self, ProviderDescriptor};
use crate::status::{StatusOk, StatusResult};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct BarState {
    pub connected: bool,
    pub warning: bool,
    pub crossed: bool,
    pub tooltip: Vec<String>,
}

pub fn summarize_provider(provider_id: &str, status: &StatusResult) -> ProviderSummary {
    let provider = providers::provider_by_id(provider_id);
    let mut summary = ProviderSummary {
        id: provider
            .map(|p| p.id.to_string())
            .unwrap_or_else(|| provider_id.to_string()),
        label: provider
            .map(|p| p.label.to_string())
            .unwrap_or_else(|| provider_id.to_string()),
        ..ProviderSummary::default()
    };
    let StatusResult::Ok(status) = status else {
        return summary;
    };
    let StatusOk {
        running,
        needs_login,
        self_name,
        self_ip,
        daemon_state,
        ..
    } = &**status;
    summary.running = *running;
    summary.needs_login = *needs_login;
    summary.self_name = self_name.clone();
    summary.self_ip = self_ip.clone();
    summary.state = daemon_state.clone();
    summary
}

fn summary_for<'a>(state: &'a PanelState, id: &str) -> Option<&'a ProviderSummary> {
    state.summaries.as_ref()?.iter().find(|s| s.id == id)
}

fn provider_state_word(summary: Option<&ProviderSummary>) -> String {
    let Some(summary) = summary else {
        return "checking\u{2026}".to_string();
    };
    if summary.needs_login {
        return "needs login".to_string();
    }
    if summary.running {
        return "connected".to_string();
    }
    if summary.state.is_empty() {
        "disconnected".to_string()
    } else {
        summary.state.clone()
    }
}

/// One line per installed provider, the active one first, so hovering the bar
/// answers "what is up" without opening the panel.
pub fn bar_tooltip(state: &PanelState) -> Vec<String> {
    let installed = providers::installed_providers(state);
    if installed.is_empty() {
        return vec![format!(
            "No supported VPN CLI on PATH. Looked for {}.",
            providers::provider_label_list()
        )];
    }

    let current = providers::active_provider(state);
    let mut ordered: Vec<&'static ProviderDescriptor> = Vec::new();
    if let Some(current) = current {
        ordered.push(current);
    }
    for provider in &installed {
        if current.is_none_or(|c| provider.id != c.id) {
            ordered.push(provider);
        }
    }

    // The names differ in length, so the state column would start in a
    // different place on every line. Padded, it reads as a column.
    let width = ordered
        .iter()
        .map(|p| p.label.chars().count())
        .max()
        .unwrap_or(0);

    let mut lines: Vec<String> = ordered
        .iter()
        .map(|provider| {
            let summary = summary_for(state, provider.id);
            let mut parts = vec![provider_state_word(summary)];
            if let Some(summary) = summary
                && summary.running
            {
                if !summary.self_name.is_empty() {
                    parts.push(summary.self_name.clone());
                }
                if !summary.self_ip.is_empty() {
                    parts.push(summary.self_ip.clone());
                }
            }
            let pad = width.saturating_sub(provider.label.chars().count());
            format!(
                "{}{}  {}",
                provider.label,
                " ".repeat(pad),
                parts.join(" \u{b7} ")
            )
        })
        .collect();

    // The bar centres each line of a tooltip on its own, so lines of different
    // lengths start at different places however their columns are padded.
    // Equal widths centre identically, which is the only way a plugin gets a
    // left edge. The pad is a no-break space because a trailing plain one is
    // not measured.
    let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    for line in &mut lines {
        let pad = longest.saturating_sub(line.chars().count());
        line.push_str(&"\u{a0}".repeat(pad));
    }
    lines
}

pub fn bar_state(state: &PanelState) -> BarState {
    let (connected, warning) = match state.summaries.as_ref() {
        Some(summaries) if !summaries.is_empty() => (
            summaries.iter().any(|s| s.running),
            summaries.iter().any(|s| s.needs_login),
        ),
        // Nothing has reported yet: fall back to the active provider's own
        // state.
        _ => (state.active, state.needs_login),
    };
    BarState {
        connected,
        warning,
        crossed: !connected && !warning,
        tooltip: bar_tooltip(state),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel_state::ProviderState;

    fn detected(ids: &[&str]) -> PanelState {
        PanelState {
            providers: Some(
                providers::PROVIDERS
                    .iter()
                    .map(|p| ProviderState {
                        id: p.id.into(),
                        installed: ids.contains(&p.id),
                    })
                    .collect(),
            ),
            ..PanelState::default()
        }
    }

    #[test]
    fn a_machine_with_no_vpn_cli_is_told_what_would_work() {
        let tooltip = bar_tooltip(&PanelState::default());
        assert_eq!(
            tooltip,
            ["No supported VPN CLI on PATH. Looked for Tailscale, NetBird."]
        );
    }

    #[test]
    fn every_tooltip_line_is_the_same_width() {
        let state = PanelState {
            summaries: Some(vec![
                ProviderSummary {
                    id: "tailscale".into(),
                    running: true,
                    self_name: "workstation".into(),
                    self_ip: "100.64.0.1".into(),
                    ..ProviderSummary::default()
                },
                ProviderSummary {
                    id: "netbird".into(),
                    ..ProviderSummary::default()
                },
            ]),
            ..detected(&["tailscale", "netbird"])
        };
        let tooltip = bar_tooltip(&state);
        assert_eq!(tooltip.len(), 2);
        let widths: Vec<usize> = tooltip.iter().map(|l| l.chars().count()).collect();
        assert_eq!(widths[0], widths[1], "the bar centres each line on its own");
        assert!(tooltip[0].starts_with("Tailscale  connected"));
        // NetBird is the shorter name, so its state column is padded out to
        // Tailscale's.
        assert!(tooltip[1].starts_with("NetBird    disconnected"));
    }

    #[test]
    fn the_provider_being_shown_leads_the_tooltip() {
        let state = PanelState {
            active_provider_id: "netbird".into(),
            ..detected(&["tailscale", "netbird"])
        };
        assert!(bar_tooltip(&state)[0].starts_with("NetBird"));
    }

    #[test]
    fn a_provider_that_has_not_reported_yet_says_so() {
        let state = detected(&["tailscale"]);
        assert!(bar_tooltip(&state)[0].contains("checking\u{2026}"));
    }

    #[test]
    fn the_icon_describes_the_machine_rather_than_the_view() {
        // NetBird is up and Tailscale is not; the panel is showing Tailscale.
        let state = PanelState {
            summaries: Some(vec![
                ProviderSummary {
                    id: "tailscale".into(),
                    ..ProviderSummary::default()
                },
                ProviderSummary {
                    id: "netbird".into(),
                    running: true,
                    ..ProviderSummary::default()
                },
            ]),
            ..detected(&["tailscale", "netbird"])
        };
        let bar = bar_state(&state);
        assert!(bar.connected, "one provider up is a connected machine");
        assert!(!bar.crossed);
    }

    #[test]
    fn a_login_that_is_wanted_is_a_warning_rather_than_a_disconnection() {
        let state = PanelState {
            summaries: Some(vec![ProviderSummary {
                id: "tailscale".into(),
                needs_login: true,
                ..ProviderSummary::default()
            }]),
            ..detected(&["tailscale"])
        };
        let bar = bar_state(&state);
        assert!(bar.warning && !bar.connected && !bar.crossed);
    }

    #[test]
    fn nothing_reported_yet_falls_back_to_the_active_provider() {
        let state = PanelState {
            active: true,
            ..detected(&["tailscale"])
        };
        assert!(bar_state(&state).connected);
        assert!(bar_state(&detected(&["tailscale"])).crossed);
    }

    #[test]
    fn a_summary_reads_only_a_status_that_parsed() {
        let running =
            crate::parse_status(r#"{"BackendState":"Running","Self":{"HostName":"box"}}"#);
        let summary = summarize_provider("tailscale", &running);
        assert_eq!(summary.label, "Tailscale");
        assert!(summary.running);
        assert_eq!(summary.self_name, "box");

        let broken = summarize_provider("tailscale", &crate::parse_status("nonsense"));
        assert!(!broken.running && broken.state.is_empty());

        let unknown = summarize_provider("wireguard", &crate::parse_status(""));
        assert_eq!(
            unknown.label, "wireguard",
            "a provider we do not know is named anyway"
        );
    }
}
