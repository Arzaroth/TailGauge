//! Everything the panel draws, read off the machine in one pass.
//!
//! This is the half a frontend used to do: probe for a CLI, poll it, parse
//! what came back, and keep the pieces. It moves here so that the three
//! frontends stop each having their own copy of it, and so the panel they draw
//! is resolved once rather than three times.

use std::fs;
use std::path::PathBuf;
use std::thread;

use serde::{Deserialize, Serialize};
use socket2::{Domain, SockAddr, Socket, Type};
use tailgauge_core as core;
use tailgauge_core::panel_state::{PanelState, ProviderState, UpdateInfo};
use tailgauge_core::providers::{self, Capability, ProviderDescriptor};

use selvedge::state;
use selvedge::update::Owner;

use crate::project::TAILGAUGE;
use selvedge::proc;

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
            installed: proc::has(p.cli),
        })
        .collect()
}

/// What the provider's `status` printed, or `None` as for a failed call.
pub fn status_output(provider: &ProviderDescriptor) -> Option<String> {
    if daemon_is_down(provider) {
        return None;
    }
    let argv = provider.backend.status();
    proc::output(&argv[0], &argv[1..])
}

/// A socket that is missing, or that refuses the connection, has no daemon
/// behind it, and the CLI would spend its timeout finding that out. Anything
/// else, a permission error included, is left for the CLI to report.
fn daemon_is_down(provider: &ProviderDescriptor) -> bool {
    daemon_is_down_in(provider, |key| std::env::var(key).ok())
}

fn daemon_is_down_in(provider: &ProviderDescriptor, env: impl Fn(&str) -> Option<String>) -> bool {
    let Some(configured) = provider.backend.daemon_socket(&env) else {
        return false;
    };
    // The CLI reads the address as a URL; one with escapes is the CLI's to
    // decode, and its raw path is not the socket.
    if configured.as_os_str().as_encoded_bytes().contains(&b'%') {
        return false;
    }
    let socket = if configured.exists() {
        configured
    } else {
        provider
            .backend
            .instance_socket_dir()
            .and_then(|dir| fs::read_dir(dir).ok())
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .collect::<Vec<_>>()
            })
            .and_then(|entries| providers::instance_socket(&entries))
            .unwrap_or(configured)
    };
    refuses(&socket)
}

/// Whether nothing listens on `socket`. Non-blocking, so a listener whose
/// backlog is full answers at once rather than holding the poll: that is a
/// daemon, busy, and the CLI is asked.
fn refuses(socket: &std::path::Path) -> bool {
    let Ok(probe) = Socket::new(Domain::UNIX, Type::STREAM, None) else {
        return false;
    };
    // Escaped addresses never get here, so a path that does not fit as a
    // socket address cannot reach a daemon, the CLI's attempt included.
    let Ok(addr) = SockAddr::unix(socket) else {
        return true;
    };
    if probe.set_nonblocking(true).is_err() {
        return false;
    }
    match probe.connect(&addr) {
        Ok(()) => false,
        Err(e) => matches!(
            e.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
        ),
    }
}

/// Every installed provider's status, gathered at once. The bar describes the
/// machine rather than the view, so the one being shown is not the only one
/// worth asking.
fn statuses(installed: &[&'static ProviderDescriptor]) -> Vec<(&'static str, core::StatusResult)> {
    let handles: Vec<_> = installed
        .iter()
        .map(|&provider| {
            thread::spawn(move || {
                let raw = status_output(provider).unwrap_or_default();
                (provider.id, provider.backend.parse_status(&raw))
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
        .then(|| provider.backend.exit_node_list())
        .flatten();
    let accounts_argv = provider
        .capabilities
        .accounts
        .then(|| provider.backend.accounts())
        .flatten();
    let networks_argv = provider
        .capabilities
        .networks
        .then(|| provider.backend.networks())
        .flatten();

    let mullvad = thread::spawn(move || {
        exit_nodes.map(|argv| {
            let raw = proc::output(&argv[0], &argv[1..]).unwrap_or_default();
            core::mullvad_region_options(&core::parse_exit_node_list(&raw))
        })
    });
    let accounts = thread::spawn(move || {
        accounts_argv.map(|argv| {
            let out = proc::run(&argv[0], &argv[1..]).ok();
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
            let raw = proc::output(&argv[0], &argv[1..]).unwrap_or_default();
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
    state::read_update_status(&state::update_cache_file(&TAILGAUGE))
        .map(|cached| UpdateInfo {
            current: cached.current,
            latest: cached.latest.unwrap_or_default(),
            upgraded_by: if cached.available {
                upgraded_by()
            } else {
                String::new()
            },
            available: cached.available,
        })
        .unwrap_or_default()
}

/// Who upgrades this binary when `--update` would refuse to, by where it runs
/// from.
fn upgraded_by() -> String {
    match selvedge::update::owner(&TAILGAUGE) {
        Ok(Some(Owner::Homebrew)) => "Homebrew",
        Ok(Some(Owner::Nix)) => "Nix",
        Ok(Some(Owner::Snap)) => "snap",
        Ok(Some(_)) => "your package manager",
        Ok(None) | Err(_) => return String::new(),
    }
    .into()
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
    let observed = snapshot_stamp();
    let gathered = statuses(&installed);
    let summaries: Vec<_> = gathered
        .iter()
        .map(|(id, status)| core::bar::summarize_provider(id, status))
        .collect();
    state.connected_order = remember_connection_order(&summaries, observed.as_ref());
    state.summaries = Some(summaries);

    let Some(active) = connect(&mut state, &gathered) else {
        return state;
    };
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

fn connection_order_file() -> PathBuf {
    state::cache_dir(&TAILGAUGE).join("connected.json")
}

/// Which provider came up first is only known by having watched it happen, and
/// nothing runs between two polls, so the order outlives the call in the cache.
/// Losing the file only costs the order, so a failure to write it is not one.
fn remember_connection_order(
    summaries: &[core::panel_state::ProviderSummary],
    observed: Option<&Stamp>,
) -> Vec<String> {
    let path = connection_order_file();
    // Panels on several monitors poll at once; one read-modify-write at a time,
    // and a snapshot older than the one that last changed the order never
    // writes, so a slow poll cannot put back an order that has moved on.
    let _lock = path.parent().and_then(|parent| {
        fs::create_dir_all(parent).ok()?;
        let lock = fs::File::create(path.with_extension("lock")).ok()?;
        lock.lock().ok()?;
        Some(lock)
    });
    let previous: RememberedOrder = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    let (order, write) = next_order(&previous, summaries, observed);
    // Without the lock, writing is what lets an older snapshot win.
    if let Some(next) = write.filter(|_| _lock.is_some()) {
        let staged = path.with_extension(format!("tmp.{}", std::process::id()));
        if serde_json::to_vec(&next)
            .ok()
            .is_some_and(|bytes| fs::write(&staged, bytes).is_ok())
        {
            let _ = fs::rename(&staged, &path);
        }
    }
    order
}

/// When a snapshot was taken, on the clock since boot: it never goes back,
/// unlike the wall clock, and the boot id says when it started again. `None`
/// where `/proc` does not answer, and such a snapshot never writes.
#[derive(Debug, Clone, PartialEq)]
struct Stamp {
    boot: String,
    ms: i64,
}

fn snapshot_stamp() -> Option<Stamp> {
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    let uptime = fs::read_to_string("/proc/uptime").ok()?;
    let seconds: f64 = uptime.split_whitespace().next()?.parse().ok()?;
    Some(Stamp {
        boot: boot.trim().to_string(),
        ms: (seconds * 1000.0) as i64,
    })
}

/// The order this snapshot draws, and what to store when it is the newest
/// snapshot to change it. A snapshot no later than the stored one, a tie
/// included, draws without writing.
fn next_order(
    previous: &RememberedOrder,
    summaries: &[core::panel_state::ProviderSummary],
    observed: Option<&Stamp>,
) -> (Vec<String>, Option<RememberedOrder>) {
    let order = core::bar::connection_order(&previous.order, summaries);
    let newer = observed.is_some_and(|s| s.boot != previous.boot || s.ms > previous.observed_ms);
    let write = observed
        .filter(|_| order != previous.order && newer)
        .map(|s| RememberedOrder {
            boot: s.boot.clone(),
            observed_ms: s.ms,
            order: order.clone(),
        });
    (order, write)
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct RememberedOrder {
    boot: String,
    #[serde(rename = "observedMs")]
    observed_ms: i64,
    order: Vec<String>,
}

/// Fills in the active provider's connection from the statuses gathered, and
/// returns that provider.
fn connect(
    state: &mut PanelState,
    gathered: &[(&'static str, core::StatusResult)],
) -> Option<&'static ProviderDescriptor> {
    let active = providers::active_provider(state)?;
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
    Some(active)
}

/// What plain `tailgauge` shows: every installed provider's status and the
/// active one's machines, without the calls only the panel needs.
pub fn connection_state() -> PanelState {
    let mut state = PanelState {
        providers: Some(probe()),
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
    connect(&mut state, &gathered);
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

    fn up(ids: &[&str]) -> Vec<core::panel_state::ProviderSummary> {
        ["tailscale", "netbird"]
            .iter()
            .map(|id| core::panel_state::ProviderSummary {
                id: (*id).into(),
                running: ids.contains(id),
                ..Default::default()
            })
            .collect()
    }

    fn at(boot: &str, ms: i64) -> Stamp {
        Stamp {
            boot: boot.into(),
            ms,
        }
    }

    fn stored(ms: i64) -> RememberedOrder {
        RememberedOrder {
            boot: "b1".into(),
            observed_ms: ms,
            order: vec!["netbird".into(), "tailscale".into()],
        }
    }

    #[test]
    fn a_snapshot_older_than_the_last_change_does_not_write_its_order() {
        let (drawn, write) = next_order(&stored(10), &up(&["netbird"]), Some(&at("b1", 7)));
        assert_eq!(drawn, ["netbird"], "it still draws what it saw");
        assert_eq!(write, None, "but leaves the newer order alone");

        let (_, write) = next_order(&stored(10), &up(&["netbird"]), Some(&at("b1", 12)));
        assert_eq!(
            write,
            Some(RememberedOrder {
                boot: "b1".into(),
                observed_ms: 12,
                order: vec!["netbird".into()],
            }),
            "a newer one that sees Tailscale gone records it"
        );
        assert_eq!(
            next_order(
                &stored(10),
                &up(&["netbird", "tailscale"]),
                Some(&at("b1", 12))
            )
            .1,
            None,
            "nothing changed, nothing written"
        );
    }

    #[test]
    fn a_socket_nothing_can_reach_reads_as_down() {
        assert!(refuses(std::path::Path::new("/nonexistent/netbird.sock")));
        let overlong = format!("/tmp/{}.sock", "n".repeat(200));
        assert!(
            refuses(std::path::Path::new(&overlong)),
            "no socket address is that long, so the CLI could not reach it either"
        );
    }

    #[test]
    fn an_escaped_daemon_address_is_left_to_the_cli() {
        let netbird = providers::provider_by_id("netbird").expect("netbird");
        let escaped = |key: &str| {
            (key == "NB_DAEMON_ADDR").then(|| "unix:///nonexistent/netbird%2Esock".to_string())
        };
        assert!(
            !daemon_is_down_in(netbird, escaped),
            "its raw path is not the socket, so the CLI decodes it"
        );
    }

    #[test]
    fn a_snapshot_without_a_stamp_draws_but_never_writes() {
        let (drawn, write) = next_order(&stored(10), &up(&["netbird"]), None);
        assert_eq!(drawn, ["netbird"]);
        assert_eq!(write, None, "it cannot say it is the newer one");
    }

    #[test]
    fn a_tie_does_not_overwrite_the_order() {
        let (_, write) = next_order(&stored(10), &up(&["netbird"]), Some(&at("b1", 10)));
        assert_eq!(write, None, "neither snapshot is the newer one");
    }

    #[test]
    fn a_new_boot_starts_the_clock_again() {
        let (_, write) = next_order(&stored(10_000), &up(&["netbird"]), Some(&at("b2", 5)));
        assert_eq!(
            write.map(|w| (w.boot, w.order)),
            Some(("b2".to_string(), vec!["netbird".to_string()])),
            "a stamp from the last boot does not outrank this one"
        );
    }

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
