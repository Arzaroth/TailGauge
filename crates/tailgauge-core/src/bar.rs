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
    /// Whose mark the icon is drawn as: the provider connected first, or with
    /// nothing up, the one wanting a login, else the preferred installed one.
    #[serde(rename = "providerId")]
    pub provider_id: String,
    /// How many providers are up, once there is more than one.
    pub badge: String,
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

pub fn provider_state_word(summary: Option<&ProviderSummary>) -> String {
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

/// The running providers, oldest connection first: `previous` keeps its order
/// for those still up, and the ones that came up since follow in registry
/// order.
pub fn connection_order(previous: &[String], summaries: &[ProviderSummary]) -> Vec<String> {
    let running = |id: &str| summaries.iter().any(|s| s.running && s.id == id);
    let mut order: Vec<String> = previous.iter().filter(|id| running(id)).cloned().collect();
    for provider in providers::PROVIDERS {
        if running(provider.id) && !order.iter().any(|id| id == provider.id) {
            order.push(provider.id.to_string());
        }
    }
    for summary in summaries {
        if summary.running && !order.contains(&summary.id) {
            order.push(summary.id.clone());
        }
    }
    order
}

pub fn bar_state(state: &PanelState) -> BarState {
    let (connected, warning, provider_id, up) = match state.summaries.as_ref() {
        Some(summaries) if !summaries.is_empty() => {
            let order = connection_order(&state.connected_order, summaries);
            let installed = providers::installed_providers(state);
            let lead = order
                .first()
                .cloned()
                .or_else(|| {
                    installed
                        .iter()
                        .find(|p| summary_for(state, p.id).is_some_and(|s| s.needs_login))
                        .or(installed.first())
                        .map(|p| p.id.to_string())
                })
                .unwrap_or_default();
            (
                !order.is_empty(),
                summaries.iter().any(|s| s.needs_login),
                lead,
                order.len(),
            )
        }
        // Nothing has reported yet: fall back to the active provider's own
        // state.
        _ => (
            state.active,
            state.needs_login,
            providers::active_provider(state)
                .map(|p| p.id.to_string())
                .unwrap_or_default(),
            usize::from(state.active),
        ),
    };
    BarState {
        connected,
        warning,
        crossed: !connected && !warning,
        provider_id,
        badge: if up > 1 {
            up.to_string()
        } else {
            String::new()
        },
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
            ["No supported VPN CLI on PATH. Looked for Tailscale, NetBird, ZeroTier."]
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

    fn up(ids: &[&str]) -> Vec<ProviderSummary> {
        providers::PROVIDERS
            .iter()
            .map(|p| ProviderSummary {
                id: p.id.into(),
                running: ids.contains(&p.id),
                ..ProviderSummary::default()
            })
            .collect()
    }

    #[test]
    fn the_connection_order_keeps_who_came_first_and_forgets_who_left() {
        let previous = ["netbird".to_string()];
        assert_eq!(
            connection_order(&previous, &up(&["tailscale", "netbird"])),
            ["netbird", "tailscale"]
        );
        assert_eq!(
            connection_order(&previous, &up(&["tailscale"])),
            ["tailscale"]
        );
        assert!(connection_order(&previous, &up(&[])).is_empty());
    }

    #[test]
    fn nothing_installed_names_no_provider() {
        let bar = bar_state(&PanelState::default());
        assert!(bar.provider_id.is_empty() && bar.badge.is_empty());
    }

    #[test]
    fn nothing_up_draws_the_preferred_installed_provider() {
        let state = PanelState {
            summaries: Some(up(&[])),
            ..detected(&["tailscale", "netbird"])
        };
        assert_eq!(bar_state(&state).provider_id, "tailscale");
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
