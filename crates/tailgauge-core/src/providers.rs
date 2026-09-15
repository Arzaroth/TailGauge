//! Every provider TailGauge knows how to drive, in one table.
//!
//! Adding a provider is a row here: the binary to probe for, what it can do,
//! and the argv for each thing the panel asks of it. Nothing else in the crate
//! names a provider, and no frontend does at all.

use serde::Serialize;

use crate::panel_state::PanelState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub exit_nodes: bool,
    pub mullvad: bool,
    pub file_send: bool,
    pub accounts: bool,
    pub networks: bool,
    pub connection_quality: bool,
}

/// One capability, named rather than indexed, so a typo is a build error the
/// way `capabilities["exitNodes"]` never was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    ExitNodes,
    Mullvad,
    FileSend,
    Accounts,
    Networks,
    ConnectionQuality,
}

impl Capabilities {
    pub fn has(&self, capability: Capability) -> bool {
        match capability {
            Capability::ExitNodes => self.exit_nodes,
            Capability::Mullvad => self.mullvad,
            Capability::FileSend => self.file_send,
            Capability::Accounts => self.accounts,
            Capability::Networks => self.networks,
            Capability::ConnectionQuality => self.connection_quality,
        }
    }
}

/// Serialized as a row payload, so a frontend can read the id off the row it
/// was handed. The commands are methods rather than data: an argv the panel
/// never reads has no business crossing to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    /// Probed on PATH to decide whether the provider is installed at all.
    pub cli: &'static str,
    /// Whether this crate can actually parse the CLI yet. A provider can be
    /// installed and named by the panel before anything knows how to drive it,
    /// and a frontend must not fire one provider's commands at another's
    /// binary.
    pub supported: bool,
    pub icon: &'static str,
    pub glyph: &'static str,
    pub capabilities: Capabilities,
}

/// Order is the preference order: with several installed and no explicit
/// choice, the first drivable one wins.
pub const PROVIDERS: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        id: "tailscale",
        label: "Tailscale",
        cli: "tailscale",
        supported: true,
        icon: "network-vpn-symbolic",
        glyph: "\u{f0ea0}",
        capabilities: Capabilities {
            exit_nodes: true,
            mullvad: true,
            file_send: true,
            accounts: true,
            networks: false,
            // Empty CurAddr with a relay region is a relayed peer, so the
            // direct or relayed reading is there for the asking.
            connection_quality: true,
        },
    },
    ProviderDescriptor {
        id: "netbird",
        label: "NetBird",
        cli: "netbird",
        supported: true,
        icon: "network-workgroup-symbolic",
        glyph: "\u{f15c6}",
        capabilities: Capabilities {
            exit_nodes: false,
            mullvad: false,
            file_send: false,
            accounts: false,
            networks: true,
            connection_quality: true,
        },
    },
];

const DEFAULT_PROVIDER_ID: &str = "tailscale";

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| p.to_string()).collect()
}

impl ProviderDescriptor {
    pub fn status(&self) -> Vec<String> {
        argv(&[self.cli, "status", "--json"])
    }

    pub fn up(&self) -> Vec<String> {
        argv(&[self.cli, "up"])
    }

    pub fn down(&self) -> Vec<String> {
        argv(&[self.cli, "down"])
    }

    pub fn exit_node_list(&self) -> Option<Vec<String>> {
        (self.id == "tailscale").then(|| argv(&["tailscale", "exit-node", "list"]))
    }

    pub fn accounts(&self) -> Option<Vec<String>> {
        (self.id == "tailscale").then(|| argv(&["tailscale", "switch", "--list", "--json"]))
    }

    pub fn networks(&self) -> Option<Vec<String>> {
        (self.id == "netbird").then(|| argv(&["netbird", "networks", "list"]))
    }

    /// Blocks until the daemon reports a change, so the panel can ride events
    /// instead of the clock. A provider without one falls back to the timer.
    pub fn watch(&self, timeout_seconds: u64) -> Option<Vec<String>> {
        (self.id == "tailscale").then(|| {
            vec![
                "tailgauge".into(),
                "watch".into(),
                timeout_seconds.to_string(),
            ]
        })
    }

    pub fn switch_account(&self, account_id: &str) -> Option<Vec<String>> {
        (self.id == "tailscale").then(|| argv(&["tailscale", "switch", account_id]))
    }

    pub fn set_exit_node(&self, target: &str) -> Option<Vec<String>> {
        (self.id == "tailscale").then(|| {
            vec![
                "tailscale".into(),
                "set".into(),
                format!("--exit-node={target}"),
            ]
        })
    }

    pub fn select_network(&self, network_id: &str, selected: bool) -> Option<Vec<String>> {
        (self.id == "netbird").then(|| {
            argv(&[
                "netbird",
                "networks",
                if selected { "select" } else { "deselect" },
                network_id,
            ])
        })
    }
}

pub fn provider_by_id(id: &str) -> Option<&'static ProviderDescriptor> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// What each frontend probes on PATH. The detection loop reads this rather
/// than a list of its own, so teaching TailGauge a provider stays a one-file
/// change.
pub fn provider_cli_names() -> Vec<&'static str> {
    PROVIDERS.iter().map(|p| p.cli).collect()
}

/// `which a b` prints one absolute path per binary it resolved and reports the
/// misses on stderr, so the exit code is a miss count rather than an answer.
/// Only stdout decides, and only lines that are real paths.
pub fn parse_provider_probe(raw: &str) -> Vec<crate::panel_state::ProviderState> {
    let mut found: Vec<&str> = Vec::new();
    for line in raw.split('\n') {
        let line = line.trim();
        if !line.starts_with('/') {
            continue;
        }
        let base = &line[line.rfind('/').map(|i| i + 1).unwrap_or(0)..];
        if !base.is_empty() && !found.contains(&base) {
            found.push(base);
        }
    }
    PROVIDERS
        .iter()
        .map(|p| crate::panel_state::ProviderState {
            id: p.id.to_string(),
            installed: found.contains(&p.cli),
        })
        .collect()
}

pub fn installed_providers(state: &PanelState) -> Vec<&'static ProviderDescriptor> {
    let Some(reported) = state.providers.as_ref() else {
        // A frontend that has not been taught to probe every provider still
        // reports the one it always drove through `installed`.
        return if state.installed {
            provider_by_id(DEFAULT_PROVIDER_ID).into_iter().collect()
        } else {
            Vec::new()
        };
    };
    // Registry order, not report order, so the preferred provider stays first
    // however the frontend happened to enumerate them.
    PROVIDERS
        .iter()
        .filter(|p| reported.iter().any(|e| e.installed && e.id == p.id))
        .collect()
}

pub fn active_provider(state: &PanelState) -> Option<&'static ProviderDescriptor> {
    let available = installed_providers(state);
    if available.is_empty() {
        return None;
    }
    if let Some(wanted) = available.iter().find(|p| p.id == state.active_provider_id) {
        return Some(wanted);
    }
    // With no choice made, land on one this crate can actually drive rather
    // than on whichever the registry lists first. A selection naming a
    // provider that is gone falls back rather than blanking the panel:
    // uninstalling one must not strand the other.
    available
        .iter()
        .find(|p| p.supported)
        .or(available.first())
        .copied()
}

/// Installed and parseable: the ones a switcher could actually move between.
pub fn drivable_providers(state: &PanelState) -> Vec<&'static ProviderDescriptor> {
    installed_providers(state)
        .into_iter()
        .filter(|p| p.supported)
        .collect()
}

/// The provider after the active one, wrapping. None when there is nothing to
/// cycle to, so a gesture bound to this does nothing rather than something
/// surprising on a machine with one provider.
pub fn next_provider(state: &PanelState) -> Option<&'static ProviderDescriptor> {
    let drivable = drivable_providers(state);
    if drivable.len() < 2 {
        return None;
    }
    let current = active_provider(state);
    let at = current
        .and_then(|c| drivable.iter().rposition(|p| p.id == c.id))
        .unwrap_or(0);
    Some(drivable[(at + 1) % drivable.len()])
}

/// Installed and parseable. The frontends poll only when this holds, so one
/// provider's commands are never fired at another's binary.
pub fn provider_ready(state: &PanelState) -> bool {
    active_provider(state).is_some_and(|p| p.supported)
}

pub fn provider_supports(state: &PanelState, capability: Capability) -> bool {
    active_provider(state).is_some_and(|p| p.capabilities.has(capability))
}

/// The name the panel calls the thing it is driving. Falls back to the product
/// name so an empty panel still says what it is.
pub fn provider_label(state: &PanelState) -> &'static str {
    active_provider(state)
        .map(|p| p.label)
        .unwrap_or("TailGauge")
}

/// Every provider we know how to drive, for the line that tells a user with
/// none of them installed what would work.
pub fn provider_label_list() -> String {
    PROVIDERS
        .iter()
        .map(|p| p.label)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel_state::ProviderState;

    fn detected(ids: &[&str]) -> PanelState {
        PanelState {
            providers: Some(
                PROVIDERS
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
    fn only_absolute_paths_on_stdout_count_as_installed() {
        // `which` reports misses on stderr, and a shell that prints its own
        // diagnostic on stdout must not read as a provider.
        let probe = parse_provider_probe(
            "/usr/bin/tailscale\nwhich: no netbird in (/usr/bin)\n  /opt/bin/netbird  \n",
        );
        assert_eq!(probe.len(), PROVIDERS.len());
        assert!(probe.iter().all(|p| p.installed));

        let none = parse_provider_probe("which: no tailscale in (/usr/bin)\n");
        assert!(none.iter().all(|p| !p.installed));
        assert!(parse_provider_probe("").iter().all(|p| !p.installed));
    }

    #[test]
    fn the_registry_order_wins_however_a_frontend_enumerated_them() {
        let mut reversed = detected(&["tailscale", "netbird"]);
        reversed.providers.as_mut().unwrap().reverse();
        assert_eq!(
            installed_providers(&reversed)
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            ["tailscale", "netbird"]
        );
    }

    #[test]
    fn a_frontend_that_probes_nothing_still_reports_the_one_it_drove() {
        let old = PanelState {
            installed: true,
            ..PanelState::default()
        };
        assert_eq!(
            installed_providers(&old)
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            ["tailscale"]
        );
        assert!(installed_providers(&PanelState::default()).is_empty());
    }

    #[test]
    fn a_selection_naming_a_provider_that_is_gone_falls_back() {
        let state = PanelState {
            active_provider_id: "netbird".into(),
            ..detected(&["tailscale"])
        };
        assert_eq!(active_provider(&state).unwrap().id, "tailscale");
        assert!(provider_ready(&state));
    }

    #[test]
    fn cycling_needs_somewhere_to_go() {
        assert!(next_provider(&detected(&["tailscale"])).is_none());
        assert!(next_provider(&PanelState::default()).is_none());

        let both = detected(&["tailscale", "netbird"]);
        assert_eq!(next_provider(&both).unwrap().id, "netbird");
        let on_netbird = PanelState {
            active_provider_id: "netbird".into(),
            ..both
        };
        assert_eq!(
            next_provider(&on_netbird).unwrap().id,
            "tailscale",
            "and wraps"
        );
    }

    #[test]
    fn capabilities_follow_the_provider_being_driven() {
        let tailscale = detected(&["tailscale"]);
        assert!(provider_supports(&tailscale, Capability::Mullvad));
        assert!(!provider_supports(&tailscale, Capability::Networks));

        let netbird = PanelState {
            active_provider_id: "netbird".into(),
            ..detected(&["netbird"])
        };
        assert!(provider_supports(&netbird, Capability::Networks));
        assert!(!provider_supports(&netbird, Capability::FileSend));
        assert!(!provider_supports(
            &PanelState::default(),
            Capability::Mullvad
        ));
    }

    #[test]
    fn an_empty_panel_still_says_what_it_is() {
        assert_eq!(provider_label(&PanelState::default()), "TailGauge");
        assert_eq!(provider_label(&detected(&["netbird"])), "NetBird");
        assert_eq!(provider_label_list(), "Tailscale, NetBird");
    }

    #[test]
    fn a_command_belongs_to_the_provider_that_has_it() {
        let tailscale = provider_by_id("tailscale").unwrap();
        let netbird = provider_by_id("netbird").unwrap();

        assert_eq!(tailscale.status(), ["tailscale", "status", "--json"]);
        assert_eq!(netbird.status(), ["netbird", "status", "--json"]);
        assert_eq!(
            tailscale.set_exit_node("de-ber.mullvad.ts.net").unwrap(),
            ["tailscale", "set", "--exit-node=de-ber.mullvad.ts.net"]
        );
        assert_eq!(
            tailscale.set_exit_node("").unwrap(),
            ["tailscale", "set", "--exit-node="]
        );
        assert_eq!(
            netbird.select_network("office", false).unwrap(),
            ["netbird", "networks", "deselect", "office"]
        );

        // A provider that cannot do a thing has no argv for it, rather than an
        // argv aimed at the other provider's binary.
        assert!(netbird.exit_node_list().is_none());
        assert!(netbird.accounts().is_none());
        assert!(netbird.watch(300).is_none());
        assert!(tailscale.networks().is_none());
        assert!(tailscale.select_network("x", true).is_none());
    }
}
