//! Every provider TailGauge knows how to drive, in one table.
//!
//! Adding a provider is a row here, the binary to probe for and what it can
//! do, and a [`Backend`] beside it: the argv for each thing the panel asks of
//! it and the parser for what it answers. Nothing else in the crate names a
//! provider, and no frontend does at all.

use std::path::PathBuf;
use std::process::Output;

use serde::Serialize;

use crate::panel_state::PanelState;
use crate::status::StatusResult;

mod netbird;
mod tailscale;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub exit_nodes: bool,
    pub mullvad: bool,
    pub file_send: bool,
    pub accounts: bool,
    pub networks: bool,
    pub connection_quality: bool,
    /// The daemon pushes its changes, so `watch` waits on its event stream
    /// rather than polling it.
    pub live_watch: bool,
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
    LiveWatch,
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
            Capability::LiveWatch => self.live_watch,
        }
    }
}

/// What a provider does, one implementation per provider. Each optional
/// command is `None` on a provider without it, never an argv aimed at another
/// provider's binary.
pub trait Backend: Sync + std::fmt::Debug {
    fn status(&self) -> Vec<String>;

    /// What `parse_status` reads, gathered through `run`, which runs an argv
    /// and hands back what it printed. `None` is a daemon that did not answer.
    /// One call to `status()` unless the provider needs several.
    fn read_status(&self, run: &dyn Fn(&[String]) -> Option<Output>) -> Option<String> {
        let out = run(&self.status())?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    }

    /// The unix socket the CLI reaches its daemon on, when the binary should
    /// probe it before asking the CLI.
    fn daemon_socket(&self, _env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
        None
    }

    /// Where the CLI looks when the configured socket is missing.
    fn instance_socket_dir(&self) -> Option<&'static str> {
        None
    }

    /// What `status()` printed, through the parser for this provider's shape.
    fn parse_status(&self, raw: &str) -> StatusResult;

    fn up(&self) -> Vec<String>;

    /// What `up` is run with when TailGauge opens the login page itself, so
    /// the CLI does not open it too.
    fn unattended_up_env(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    fn down(&self) -> Vec<String>;

    fn exit_node_list(&self) -> Option<Vec<String>> {
        None
    }

    fn accounts(&self) -> Option<Vec<String>> {
        None
    }

    fn networks(&self) -> Option<Vec<String>> {
        None
    }

    /// A stream that prints a line per daemon notification, for a provider
    /// with `live_watch`. A provider without one is polled.
    fn events(&self) -> Option<Vec<String>> {
        None
    }

    /// Which notification lines are a change the panel draws. Everything else
    /// on the stream is churn, which would turn a refresh into a busy loop.
    fn event_markers(&self) -> &'static [&'static str] {
        &[]
    }

    fn switch_account(&self, _account_id: &str) -> Option<Vec<String>> {
        None
    }

    fn set_exit_node(&self, _target: &str) -> Option<Vec<String>> {
        None
    }

    fn select_network(&self, _network_id: &str, _selected: bool) -> Option<Vec<String>> {
        None
    }

    /// How a user the CLI refuses is let in, for a provider that can refuse
    /// one.
    fn authorization(&self) -> Option<Authorization> {
        None
    }
}

/// Letting this user drive a provider that refuses them: a shell line run as
/// the user, which elevates itself, and the row that offers it. The user is
/// resolved by that shell rather than here: a widget started without one in
/// its environment would otherwise authorize nobody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Authorization {
    pub command: &'static str,
    pub label: &'static str,
    pub sublabel: &'static str,
}

/// Serialized as a row payload, so a frontend can read the id off the row it
/// was handed. The commands live on the backend rather than in data: an argv
/// the panel never reads has no business crossing to it.
#[derive(Debug, Clone, Copy, Serialize)]
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
    #[serde(skip)]
    pub backend: &'static dyn Backend,
}

/// One row per id, so the id is the identity.
impl PartialEq for ProviderDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ProviderDescriptor {}

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
            live_watch: true,
        },
        backend: &tailscale::Tailscale,
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
            live_watch: false,
        },
        backend: &netbird::NetBird,
    },
];

const DEFAULT_PROVIDER_ID: &str = "tailscale";

/// The socket NetBird's CLI falls back to among `instance_socket_dir`'s
/// entries: the only `.sock` there, or none when there are several.
pub fn instance_socket(entries: &[PathBuf]) -> Option<PathBuf> {
    let mut sockets = entries
        .iter()
        .filter(|e| e.extension().is_some_and(|x| x == "sock"));
    match (sockets.next(), sockets.next()) {
        (Some(only), None) => Some(only.clone()),
        _ => None,
    }
}

pub(crate) fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|p| p.to_string()).collect()
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

/// What the provider can do at all, narrowed by what its daemon reports this
/// connection allows. A self-hosted control server (Headscale) runs the same
/// CLI with no Mullvad behind it, and an admin can switch Taildrop off: both
/// reach the panel here, the same way a provider without the feature does.
pub fn provider_supports(state: &PanelState, capability: Capability) -> bool {
    active_provider(state)
        .is_some_and(|p| p.capabilities.has(capability) && reported(state, capability))
}

fn reported(state: &PanelState, capability: Capability) -> bool {
    match capability {
        Capability::FileSend => state.file_sharing,
        Capability::Mullvad => !state.mullvad_regions.is_empty(),
        _ => true,
    }
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
    #[test]
    fn only_netbird_has_a_socket_to_check_and_the_cli_s_own_address_wins() {
        let netbird = provider_by_id("netbird").expect("netbird");
        let tailscale = provider_by_id("tailscale").expect("tailscale");
        assert_eq!(
            netbird.backend.daemon_socket(&|_| None),
            Some(PathBuf::from("/var/run/netbird.sock"))
        );
        assert_eq!(
            netbird
                .backend
                .daemon_socket(&|_| Some("unix:///tmp/nb.sock".into())),
            Some(PathBuf::from("/tmp/nb.sock"))
        );
        assert_eq!(
            netbird
                .backend
                .daemon_socket(&|_| Some("tcp://127.0.0.1:41731".into())),
            None,
            "a TCP daemon cannot be checked by a file"
        );
        assert_eq!(tailscale.backend.daemon_socket(&|_| None), None);
    }

    #[test]
    fn the_legacy_daemon_address_counts_but_the_current_one_wins() {
        let netbird = provider_by_id("netbird").expect("netbird");
        let legacy = |key: &str| (key == "WT_DAEMON_ADDR").then(|| "unix:///old.sock".to_string());
        assert_eq!(
            netbird.backend.daemon_socket(&legacy),
            Some(PathBuf::from("/old.sock"))
        );
        let both = |key: &str| {
            Some(
                if key == "NB_DAEMON_ADDR" {
                    "unix:///new.sock"
                } else {
                    "unix:///old.sock"
                }
                .to_string(),
            )
        };
        assert_eq!(
            netbird.backend.daemon_socket(&both),
            Some(PathBuf::from("/new.sock"))
        );
    }

    #[test]
    fn an_instance_socket_is_used_only_when_it_is_the_only_one() {
        let path = |p: &str| PathBuf::from(p);
        assert_eq!(
            instance_socket(&[
                path("/var/run/netbird/main.sock"),
                path("/var/run/netbird/main.pid")
            ]),
            Some(path("/var/run/netbird/main.sock"))
        );
        assert_eq!(
            instance_socket(&[
                path("/var/run/netbird/a.sock"),
                path("/var/run/netbird/b.sock")
            ]),
            None,
            "the CLI asks for --daemon-addr rather than guess"
        );
        assert_eq!(instance_socket(&[]), None);
    }

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
        let tailscale = PanelState {
            mullvad_regions: vec![crate::peer::Peer::default()],
            ..detected(&["tailscale"])
        };
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
    fn what_the_daemon_reports_narrows_what_the_provider_can_do() {
        let tailscale = detected(&["tailscale"]);
        assert!(!provider_supports(&tailscale, Capability::Mullvad));
        assert!(!provider_supports(&tailscale, Capability::FileSend));
        assert!(provider_supports(&tailscale, Capability::ExitNodes));

        let granted = PanelState {
            file_sharing: true,
            ..detected(&["tailscale"])
        };
        assert!(provider_supports(&granted, Capability::FileSend));

        let netbird = PanelState {
            active_provider_id: "netbird".into(),
            file_sharing: true,
            ..detected(&["netbird"])
        };
        assert!(
            !provider_supports(&netbird, Capability::FileSend),
            "a report never grants what the provider cannot do"
        );
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

        assert_eq!(
            tailscale.backend.status(),
            ["tailscale", "status", "--json"]
        );
        assert_eq!(netbird.backend.status(), ["netbird", "status", "--json"]);
        assert_eq!(
            tailscale
                .backend
                .set_exit_node("de-ber.mullvad.ts.net")
                .unwrap(),
            ["tailscale", "set", "--exit-node=de-ber.mullvad.ts.net"]
        );
        assert_eq!(
            tailscale.backend.set_exit_node("").unwrap(),
            ["tailscale", "set", "--exit-node="]
        );
        assert_eq!(
            netbird.backend.select_network("office", false).unwrap(),
            ["netbird", "networks", "deselect", "office"]
        );

        // A provider that cannot do a thing has no argv for it, rather than an
        // argv aimed at the other provider's binary.
        assert!(netbird.backend.exit_node_list().is_none());
        assert!(netbird.backend.accounts().is_none());
        assert!(netbird.backend.events().is_none());
        assert!(tailscale.backend.networks().is_none());
        assert!(tailscale.backend.select_network("x", true).is_none());
    }

    #[test]
    fn only_tailgauge_opens_the_login_page() {
        let netbird = provider_by_id("netbird").unwrap();
        assert_eq!(netbird.backend.up(), ["netbird", "up"]);
        assert_eq!(
            netbird.backend.unattended_up_env(),
            [("NB_NO_BROWSER", "true")]
        );
        assert!(
            provider_by_id("tailscale")
                .unwrap()
                .backend
                .unattended_up_env()
                .is_empty()
        );
    }
}

/// What turning a provider on means right now: a URL to open when the daemon
/// is waiting for an authorization, and the command to run otherwise.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct LoginPlan {
    #[serde(rename = "authUrl")]
    pub auth_url: String,
    pub command: Vec<String>,
}

pub fn login_plan(needs_login: bool, auth_url: &str, up_command: &[String]) -> LoginPlan {
    let url = auth_url.trim();
    if needs_login && (url.starts_with("https://") || url.starts_with("http://")) {
        return LoginPlan {
            auth_url: url.to_string(),
            command: Vec::new(),
        };
    }
    LoginPlan {
        auth_url: String::new(),
        command: up_command.to_vec(),
    }
}

#[cfg(test)]
mod login_tests {
    use super::*;

    #[test]
    fn a_pending_authorization_is_a_url_to_open_whoever_the_provider_is() {
        let up: Vec<String> = vec!["netbird".into(), "up".into()];
        assert_eq!(
            login_plan(false, "", &up),
            LoginPlan {
                auth_url: String::new(),
                command: up.clone()
            }
        );
        assert_eq!(
            login_plan(true, "https://login.example/x", &up),
            LoginPlan {
                auth_url: "https://login.example/x".into(),
                command: Vec::new()
            }
        );
        // A daemon that wants a login but named no URL still has to be told to
        // come up, or the button does nothing.
        assert_eq!(login_plan(true, "", &up).command, up);
        assert_eq!(login_plan(false, "", &[]), LoginPlan::default());
    }
}
