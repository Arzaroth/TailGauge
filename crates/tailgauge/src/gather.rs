//! Everything the panel draws, read off the machine in one pass.
//!
//! This is the half a frontend used to do: probe for a CLI, poll it, parse
//! what came back, and keep the pieces. It moves here so that the three
//! frontends stop each having their own copy of it, and so the panel they draw
//! is resolved once rather than three times.

use std::thread;

use serde::Deserialize;
use tailgauge_core as core;
use tailgauge_core::panel_state::{PanelState, ProviderState, UpdateInfo};
use tailgauge_core::providers::{self, Capability, ProviderDescriptor};

use crate::launch;
use crate::state;

/// What only the frontend knows: which provider the user is looking at, what
/// it is optimistically showing, and what it has in flight. Handed over as one
/// JSON blob rather than a flag each, because it is one object on that side
/// too.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Ui {
    #[serde(rename = "activeProviderId")]
    pub active_provider_id: String,
    /// The optimistic toggle: flipped locally on click, reconciled here on the
    /// next snapshot. `None` means "whatever the daemon says".
    pub active: Option<bool>,
    pub busy: bool,
    pub updating: bool,
    pub version: String,
    #[serde(rename = "actionStatus")]
    pub action_status: String,
    #[serde(rename = "lastError")]
    pub last_error: String,
    #[serde(rename = "switchingAccountId")]
    pub switching_account_id: String,
    #[serde(rename = "settingExitNodeId")]
    pub setting_exit_node_id: String,
    #[serde(rename = "selectingNetworkId")]
    pub selecting_network_id: String,
    #[serde(rename = "phraseIndex")]
    pub phrase_index: i64,
    #[serde(rename = "recentRegions")]
    pub recent_regions: Vec<String>,
    #[serde(rename = "mullvadPickerOpen")]
    pub mullvad_picker_open: bool,
    #[serde(rename = "expandedPeerId")]
    pub expanded_peer_id: String,
}

/// Which providers are on PATH. Answered in-process: `which` was a subprocess
/// spawned to do what resolving PATH does.
fn probe() -> Vec<ProviderState> {
    providers::PROVIDERS
        .iter()
        .map(|p| ProviderState {
            id: p.id.into(),
            installed: launch::has(p.cli),
        })
        .collect()
}

/// Every installed provider's status, gathered at once. The bar describes the
/// machine rather than the view, so the one being shown is not the only one
/// worth asking.
fn statuses(installed: &[&'static ProviderDescriptor]) -> Vec<(&'static str, core::StatusResult)> {
    let handles: Vec<_> = installed
        .iter()
        .map(|provider| {
            let id = provider.id;
            let argv = provider.status();
            thread::spawn(move || {
                let raw = launch::output(&argv[0], &argv[1..]).unwrap_or_default();
                let parsed = if id == "netbird" {
                    core::parse_netbird_status(&raw)
                } else {
                    core::parse_status(&raw)
                };
                (id, parsed)
            })
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// The active provider's extras, each one a call the panel needs only when the
/// provider has that capability. Run together, because they are independent
/// and a panel waiting on them one after another is a panel three round trips
/// behind.
struct Extras {
    mullvad_regions: Vec<core::Peer>,
    accounts: core::AccountsResult,
    accounts_access_denied: bool,
    networks: Vec<core::Network>,
}

fn extras(provider: &'static ProviderDescriptor) -> Extras {
    let exit_nodes = provider
        .capabilities
        .exit_nodes
        .then(|| provider.exit_node_list())
        .flatten();
    let accounts_argv = provider
        .capabilities
        .accounts
        .then(|| provider.accounts())
        .flatten();
    let networks_argv = provider
        .capabilities
        .networks
        .then(|| provider.networks())
        .flatten();

    let mullvad = thread::spawn(move || {
        exit_nodes.map(|argv| {
            let raw = launch::output(&argv[0], &argv[1..]).unwrap_or_default();
            core::mullvad_region_options(&core::parse_exit_node_list(&raw))
        })
    });
    let accounts = thread::spawn(move || {
        accounts_argv.map(|argv| {
            let out = launch::run(&argv[0], &argv[1..]).ok();
            let stdout = out
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default();
            let stderr = out
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stderr).into_owned())
                .unwrap_or_default();
            // The CLI refuses to list profiles for a user who does not operate
            // the daemon, which is a prompt to show rather than an error.
            let denied = core::fmt::is_profiles_access_denied(&stderr);
            (core::parse_accounts(&stdout), denied)
        })
    });
    let networks = thread::spawn(move || {
        networks_argv.map(|argv| {
            let raw = launch::output(&argv[0], &argv[1..]).unwrap_or_default();
            core::parse_netbird_networks(&raw).networks
        })
    });

    let (accounts, accounts_access_denied) = accounts
        .join()
        .ok()
        .flatten()
        .unwrap_or((core::AccountsResult::default(), false));
    Extras {
        mullvad_regions: mullvad.join().ok().flatten().unwrap_or_default(),
        accounts,
        accounts_access_denied,
        networks: networks.join().ok().flatten().unwrap_or_default(),
    }
}

/// What the last update check found. Read from the cache only: a panel drawing
/// itself must not wait on GitHub, and `--check-update` is what refreshes it.
fn update_info() -> UpdateInfo {
    state::read_update_status(&state::update_cache_file())
        .map(|cached| UpdateInfo {
            current: cached.current,
            latest: cached.latest.unwrap_or_default(),
            available: cached.available,
        })
        .unwrap_or_default()
}

pub fn panel_state(ui: &Ui) -> PanelState {
    let probed = probe();
    let mut state = PanelState {
        providers: Some(probed),
        active_provider_id: ui.active_provider_id.clone(),
        // A store-installed widget has no binary on PATH; this one plainly
        // does, since it is the one answering.
        helpers: true,
        busy: ui.busy,
        updating: ui.updating,
        version: ui.version.clone(),
        action_status: ui.action_status.clone(),
        last_error: ui.last_error.clone(),
        switching_account_id: ui.switching_account_id.clone(),
        setting_exit_node_id: ui.setting_exit_node_id.clone(),
        selecting_network_id: ui.selecting_network_id.clone(),
        update: Some(update_info()),
        ..PanelState::default()
    };

    let installed = providers::installed_providers(&state);
    let gathered = statuses(&installed);
    state.summaries = Some(
        gathered
            .iter()
            .map(|(id, status)| core::bar::summarize_provider(id, status))
            .collect(),
    );

    let Some(active) = providers::active_provider(&state) else {
        return state;
    };
    state.installed = true;

    if let Some((_, core::StatusResult::Ok(status))) =
        gathered.iter().find(|(id, _)| *id == active.id)
    {
        state.running = status.running;
        state.needs_login = status.needs_login;
        state.self_name = status.self_name.clone();
        state.self_ip = status.self_ip.clone();
        state.self_user_id = status.self_user_id.clone();
        state.self_peer = Some(status.self_peer.clone());
        state.file_sharing = status.file_sharing;
        state.peers = status.peers.clone();
        state.own_exit_nodes = status.exit_nodes.clone();
    }
    // The optimistic toggle wins until the daemon catches up: a click that
    // reads as nothing having happened is worse than one the next poll undoes.
    state.active = ui.active.unwrap_or(state.running);

    // Only what the panel will actually draw. A provider with no Mullvad has
    // no region list to fetch, and asking anyway is a round trip per poll.
    if state.running || providers::provider_supports(&state, Capability::Accounts) {
        let extras = extras(active);
        state.mullvad_regions = extras.mullvad_regions;
        state.selected_account_id = extras.accounts.selected_account_id.clone();
        state.accounts = extras.accounts.accounts;
        state.accounts_access_denied = extras.accounts_access_denied;
        state.networks = extras.networks;
    }

    state
}

pub fn resolve_options(ui: &Ui) -> core::panel::ResolveOptions {
    core::panel::ResolveOptions {
        phrase_index: ui.phrase_index,
        recent_regions: ui.recent_regions.clone(),
        mullvad_picker_open: ui.mullvad_picker_open,
        expanded_peer_id: ui.expanded_peer_id.clone(),
        now_ms: state::now_ms(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ui_blob_is_optional_field_by_field() {
        // A frontend that has not learned a field yet must not blank the panel.
        let ui: Ui = serde_json::from_str("{}").expect("an empty object is a valid UI state");
        assert_eq!(ui.active, None);
        assert!(!ui.busy);
        assert_eq!(ui.phrase_index, 0);
        assert!(ui.recent_regions.is_empty());
    }

    #[test]
    fn the_optimistic_toggle_is_absent_rather_than_false() {
        // `false` means "the user just turned it off"; absent means "ask the
        // daemon". Reading one as the other makes a panel that never connects.
        let clicked: Ui = serde_json::from_str(r#"{"active":false}"#).expect("valid");
        assert_eq!(clicked.active, Some(false));
        let quiet: Ui = serde_json::from_str(r#"{"busy":true}"#).expect("valid");
        assert_eq!(quiet.active, None);
    }

    /// The gather is the one part of the port with no differential
    /// counterpart: the TypeScript model never gathered, the frontends did. So
    /// what is checked here is that whatever this machine happens to have
    /// produces a panel that hangs together.
    #[test]
    fn what_this_machine_reports_resolves_into_a_coherent_panel() {
        let ui = Ui {
            version: "0.0.0-test".into(),
            ..Ui::default()
        };
        let state = panel_state(&ui);
        let panel = core::panel::panel_spec(&state, &resolve_options(&ui));

        if providers::active_provider(&state).is_none() {
            assert!(panel.status.text.contains("No supported VPN CLI"));
            assert_eq!(panel.navigation.len(), 1, "the header is the only stop");
            return;
        }

        // Every cursor stop resolves back to the row it points at, and every
        // row a frontend would draw is in a section marked visible.
        for i in 1..panel.navigation.len() {
            let row = core::panel::panel_row_at(&panel, i)
                .unwrap_or_else(|| panic!("stop {i} resolves to nothing"));
            assert_eq!(row.id, panel.navigation[i].row_id);
        }
        // Starts with, rather than equals: the footer adds the binary's own
        // version when it disagrees with the widget's, and on a machine that
        // has checked for an update once, it does.
        assert!(
            panel.footer.starts_with("TailGauge v0.0.0-test"),
            "the frontend's own version leads the footer: {}",
            panel.footer
        );
        assert_eq!(
            panel
                .sections
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            [
                "update",
                "providers",
                "self",
                "connections",
                "exitNodes",
                "networks",
                "machines"
            ]
        );
        assert!(!panel.bar.tooltip.is_empty());
    }

    #[test]
    fn every_installed_provider_is_probed_for() {
        let probed = probe();
        assert_eq!(probed.len(), providers::PROVIDERS.len());
        for (state, provider) in probed.iter().zip(providers::PROVIDERS) {
            assert_eq!(state.id, provider.id);
        }
    }
}
