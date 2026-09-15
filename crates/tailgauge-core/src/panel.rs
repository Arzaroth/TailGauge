//! The panel every frontend draws, resolved once.
//!
//! Which sections exist, their order, their rows, every label, and the
//! cursor's traversal order are decided here. A frontend decides only what a
//! row looks like.

use serde::Serialize;
use serde_json::Value;

use crate::fmt;
use crate::panel_state::PanelState;
use crate::peer::Peer;
use crate::providers::{self, Capability};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CopyOption {
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RowAction {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub glyph: String,
}

impl RowAction {
    fn new(id: &str, label: &str, icon: &str, glyph: &str) -> Self {
        RowAction {
            id: id.into(),
            label: label.into(),
            icon: icon.into(),
            glyph: glyph.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PanelRow {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub sublabel: String,
    pub icon: String,
    pub glyph: String,
    pub action: String,
    pub current: bool,
    pub busy: bool,
    pub bold: bool,
    pub navigable: bool,
    pub hint: String,
    pub actions: Vec<RowAction>,
    #[serde(rename = "copyOptions")]
    pub copy_options: Vec<CopyOption>,
    pub children: Vec<PanelRow>,
    pub expanded: bool,
    #[serde(rename = "searchPlaceholder")]
    pub search_placeholder: String,
    /// A row with a scope is filtered live by that search field's query: it is
    /// drawn when the query is a substring of `search_key`. The one row per
    /// scope with an empty key is the message drawn when nothing matched.
    #[serde(rename = "searchScope")]
    pub search_scope: String,
    #[serde(rename = "searchKey")]
    pub search_key: String,
    pub payload: Value,
}

impl Default for PanelRow {
    /// Everything a caller does not set, with `navigable` true: a row is a
    /// cursor stop unless it says otherwise, which is the way the far more
    /// numerous rows want it.
    fn default() -> Self {
        PanelRow {
            id: String::new(),
            kind: String::new(),
            label: String::new(),
            sublabel: String::new(),
            icon: String::new(),
            glyph: String::new(),
            action: String::new(),
            current: false,
            busy: false,
            bold: false,
            navigable: true,
            hint: String::new(),
            actions: Vec::new(),
            copy_options: Vec::new(),
            children: Vec::new(),
            expanded: false,
            search_placeholder: String::new(),
            search_scope: String::new(),
            search_key: String::new(),
            payload: Value::Null,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PanelStatus {
    pub text: String,
    pub tone: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PanelHeader {
    pub id: String,
    pub title: String,
    #[serde(rename = "providerId")]
    pub provider_id: String,
    pub icon: String,
    pub glyph: String,
    pub meta: String,
    pub action: String,
    #[serde(rename = "toggleVisible")]
    pub toggle_visible: bool,
    #[serde(rename = "toggleEnabled")]
    pub toggle_enabled: bool,
    #[serde(rename = "toggleChecked")]
    pub toggle_checked: bool,
    pub busy: bool,
    #[serde(rename = "toggleHint")]
    pub toggle_hint: String,
    pub crossed: bool,
    pub warning: bool,
    pub dimmed: bool,
    pub actions: Vec<RowAction>,
}

// ---------------------------------------------------------------------------
// what a machine row shows
// ---------------------------------------------------------------------------

pub fn os_icon(os: &str) -> &'static str {
    match os.to_lowercase().as_str() {
        "linux" => "\u{f033d}",
        "macos" | "ios" => "\u{f0035}",
        "windows" => "\u{f0372}",
        "android" => "\u{f0032}",
        "mullvad" => "\u{f0582}",
        _ => "\u{f07c0}",
    }
}

pub fn os_icon_name(os: &str) -> &'static str {
    match os.to_lowercase().as_str() {
        "linux" | "macos" | "windows" => "computer-symbolic",
        "ios" | "android" => "phone-symbolic",
        "mullvad" => "network-vpn-symbolic",
        _ => "network-server-symbolic",
    }
}

/// The name to address a machine by, in the order the CLI will accept.
pub fn peer_address(peer: &Peer) -> String {
    if !peer.dns_name.is_empty() {
        return crate::peer::clean_dns_name(&peer.dns_name);
    }
    if !peer.host_name.is_empty() {
        return peer.host_name.clone();
    }
    crate::peer::filter_ipv4(&peer.ipv4)
        .first()
        .cloned()
        .unwrap_or_default()
}

/// A Mullvad node is set by address: its name is not a tailnet name, and the
/// CLI will not resolve it.
pub fn exit_node_target(peer: &Peer) -> String {
    if peer.mullvad
        && let Some(ip) = crate::peer::filter_ipv4(&peer.ipv4).first()
    {
        return ip.clone();
    }
    peer_address(peer)
}

/// Taildrop's own grading, when the daemon reports one; otherwise the older
/// rule, which is that you can send to your own machines.
pub fn is_taildrop_target(peer: &Peer, self_user_id: &str) -> bool {
    if let Some(target) = peer.taildrop_target
        && target != 0
    {
        return target == 1;
    }
    let owner = peer.user_id.as_deref().unwrap_or("");
    !owner.is_empty() && owner == self_user_id
}

pub fn can_send_files(state: &PanelState, peer: &Peer) -> bool {
    if !providers::provider_supports(state, Capability::FileSend) {
        return false;
    }
    if !state.file_sharing || !state.running || !peer.online {
        return false;
    }
    // The KDE Store ships a kpackage and EGO ships an extension zip; neither
    // can put the tailgauge binary on PATH. Without it the button would do
    // nothing at all.
    if !state.helpers {
        return false;
    }
    is_taildrop_target(peer, &state.self_user_id)
}

pub fn peer_copy_options(peer: &Peer) -> Vec<CopyOption> {
    let mut options = Vec::new();
    let mut add = |kind: &str, label: &str| {
        if !label.is_empty() {
            options.push(CopyOption {
                kind: kind.into(),
                label: label.into(),
            });
        }
    };
    let name = if !peer.display_name.is_empty() {
        peer.display_name.clone()
    } else {
        peer.host_name.clone()
    };
    add("name", &name);
    add("dns", &peer.dns_name);
    add("ipv6", peer.ipv6.first().map(String::as_str).unwrap_or(""));
    add("ip", peer.ipv4.first().map(String::as_str).unwrap_or(""));
    add("key", peer.public_key.as_deref().unwrap_or(""));
    options
}

/// The owner takes the DNS name's place rather than sitting after it: the row
/// already names the machine, the full name is one click away in the copy
/// menu, and a third part would only elide on Plasma and widen the menu on
/// GNOME.
pub fn peer_subtitle(peer: &Peer) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(ip) = peer.ipv4.first() {
        parts.push(ip.clone());
    }
    match peer.user_name.as_deref() {
        Some(owner) if !owner.is_empty() => parts.push(owner.to_string()),
        _ if !peer.dns_name.is_empty() => parts.push(peer.dns_name.clone()),
        _ => {}
    }
    parts.join(" \u{b7} ")
}

/// An offline machine keeps its copy actions - a sleeping laptop's address is
/// exactly what someone needs to wake it - so the row has to say why it reads
/// differently from the ones above it.
pub fn peer_row_subtitle(peer: &Peer) -> String {
    let subtitle = peer_subtitle(peer);
    if peer.online {
        return subtitle;
    }
    if subtitle.is_empty() {
        "Offline".to_string()
    } else {
        format!("Offline \u{b7} {subtitle}")
    }
}

/// How the tunnel is carried, in words rather than a provider's shorthand.
pub fn connection_summary(peer: &Peer) -> String {
    let kind = peer.connection_type.as_deref().unwrap_or("");
    let relay = peer.relay.as_deref().unwrap_or("");
    if kind == "P2P" || kind == "Direct" {
        return "Direct peer-to-peer".to_string();
    }
    if kind == "Relayed" || !relay.is_empty() {
        return if relay.is_empty() {
            "Relayed".to_string()
        } else {
            format!("Relayed via {relay}")
        };
    }
    String::new()
}

/// The rows behind a machine's disclosure arrow. Only what the provider
/// actually reported: an absent reading is a row that is not there.
pub fn peer_detail_rows(peer: &Peer, now_ms: i64) -> Vec<PanelRow> {
    fn detail(rows: &mut Vec<PanelRow>, id: &str, label: &str, value: String) {
        if value.is_empty() {
            return;
        }
        rows.push(PanelRow {
            id: format!("detail:{id}"),
            kind: "detail".into(),
            label: label.into(),
            sublabel: value,
            navigable: false,
            ..PanelRow::default()
        });
    }

    let mut rows: Vec<PanelRow> = Vec::new();

    let latency = match peer.latency_ms {
        Some(ms) if ms >= 0.0 => format!("{} ms", ms.round() as i64),
        _ => String::new(),
    };
    let summary = connection_summary(peer);
    let connection = match (summary.is_empty(), latency.is_empty()) {
        (false, false) => format!("{summary} \u{b7} {latency}"),
        (false, true) => summary,
        (true, _) => latency,
    };

    detail(&mut rows, "connection", "Connection", connection);
    detail(
        &mut rows,
        "endpoint",
        "Endpoint",
        peer.endpoint.clone().unwrap_or_default(),
    );

    let handshake = fmt::format_since(peer.last_handshake.as_deref().unwrap_or(""), now_ms);
    let had_handshake = !handshake.is_empty();
    detail(&mut rows, "handshake", "Last handshake", handshake);
    // Tailscale fills the handshake and byte counters only once a session is
    // up. For an idle peer, when the control plane last saw it is all there is.
    if !had_handshake {
        detail(
            &mut rows,
            "seen",
            "Last seen",
            fmt::format_since(peer.last_seen.as_deref().unwrap_or(""), now_ms),
        );
    }

    let rx = peer.rx_bytes.unwrap_or(0);
    let tx = peer.tx_bytes.unwrap_or(0);
    if rx > 0 || tx > 0 {
        detail(
            &mut rows,
            "transfer",
            "Transfer",
            format!(
                "\u{2193} {}   \u{2191} {}",
                fmt::format_bytes(rx),
                fmt::format_bytes(tx)
            ),
        );
    }

    let routes = peer.routes.clone().unwrap_or_default();
    if !routes.is_empty() {
        detail(&mut rows, "routes", "Routes", routes.join(", "));
    }

    // An idle peer reports nothing about a session it does not have, so the
    // one thing always known about it keeps the row from opening onto almost
    // nothing.
    if rows.len() < 3 {
        detail(
            &mut rows,
            "added",
            "Added",
            fmt::format_since(peer.created.as_deref().unwrap_or(""), now_ms),
        );
    }

    rows
}

// ---------------------------------------------------------------------------
// the header, and what it says underneath
// ---------------------------------------------------------------------------

const ACTIVE_PHRASES: [&str; 10] = [
    "Encrypting connections",
    "Sending secrets",
    "Guarding wires",
    "Braiding packets",
    "Polishing tunnels",
    "Hiding routes",
    "Sealing ports",
    "Sorting peers",
    "Shuffling keys",
    "Watching machines",
];

/// What the switch will do, named after the provider it will do it to. Shared
/// so a desktop's own menu cannot drift from the panel's switch.
pub fn toggle_hint(state: &PanelState) -> String {
    let label = providers::provider_label(state);
    if state.active {
        return format!("Turn {label} off");
    }
    if state.needs_login {
        return "Authorize this device".to_string();
    }
    format!("Turn {label} on")
}

pub fn panel_header(state: &PanelState, phrase_index: i64) -> PanelHeader {
    let label = providers::provider_label(state);
    let provider = providers::active_provider(state);
    // A provider we cannot drive yet is named, but its switch would do nothing.
    let present = providers::provider_ready(state);
    let phrases = ACTIVE_PHRASES.len() as i64;
    let meta = if state.active {
        ACTIVE_PHRASES[(phrase_index.rem_euclid(phrases)) as usize].to_string()
    } else {
        format!("{label} is disconnected")
    };

    PanelHeader {
        id: "header".into(),
        title: if present && !state.self_name.is_empty() {
            state.self_name.clone()
        } else {
            label.to_string()
        },
        provider_id: provider.map(|p| p.id.to_string()).unwrap_or_default(),
        icon: provider
            .map(|p| p.icon)
            .unwrap_or("network-vpn-symbolic")
            .into(),
        glyph: provider.map(|p| p.glyph).unwrap_or("\u{f0ea0}").into(),
        meta,
        action: "toggle".into(),
        toggle_visible: present,
        // Never gated on `busy`. A background status poll must not make the
        // switch unclickable, and a toggle already reports optimistically
        // through `active`, so there is nothing to protect against a second
        // click.
        toggle_enabled: present,
        toggle_checked: state.active,
        busy: state.busy,
        toggle_hint: toggle_hint(state),
        crossed: !state.active && !state.needs_login,
        warning: state.needs_login,
        dimmed: !state.active,
        actions: if present {
            vec![RowAction::new(
                "refresh",
                "Refresh",
                "view-refresh-symbolic",
                "\u{f0450}",
            )]
        } else {
            Vec::new()
        },
    }
}

/// Precedence, in one place: a command's own progress beats a stale error, and
/// both beat the idle line.
pub fn panel_status(state: &PanelState) -> PanelStatus {
    let Some(provider) = providers::active_provider(state) else {
        return PanelStatus {
            text: format!(
                "No supported VPN CLI on PATH. Looked for {}.",
                providers::provider_label_list()
            ),
            tone: "dim".into(),
        };
    };
    if !provider.supported {
        return PanelStatus {
            text: format!(
                "{} is installed, but TailGauge cannot drive it yet.",
                provider.label
            ),
            tone: "dim".into(),
        };
    }
    if !state.action_status.is_empty() {
        return PanelStatus {
            text: state.action_status.clone(),
            tone: "dim".into(),
        };
    }
    if !state.last_error.is_empty() {
        return PanelStatus {
            text: state.last_error.clone(),
            tone: "error".into(),
        };
    }
    PanelStatus {
        text: String::new(),
        tone: String::new(),
    }
}

/// The version of the widget you are looking at, in the quietest line the
/// panel has. Each frontend passes its own, read from the manifest it shipped
/// with.
pub fn panel_footer(state: &PanelState) -> String {
    if state.version.is_empty() {
        return String::new();
    }
    let mut text = format!("TailGauge v{}", state.version);
    // The widget and the binary install separately, so an update that only
    // half applied leaves them on different versions with nothing else on
    // screen saying which half is behind.
    let binary = binary_version(state);
    if !binary.is_empty() && binary != state.version {
        text.push_str(&format!(" \u{b7} binary v{binary}"));
    }
    text
}

pub fn binary_version(state: &PanelState) -> String {
    state
        .update
        .as_ref()
        .map(|u| u.current.clone())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// the sections
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PanelSection {
    pub id: String,
    pub title: String,
    pub visible: bool,
    pub empty: String,
    pub rows: Vec<PanelRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NavEntry {
    #[serde(rename = "sectionId")]
    pub section_id: String,
    #[serde(rename = "rowId")]
    pub row_id: String,
    pub action: String,
    #[serde(rename = "searchScope")]
    pub search_scope: String,
    #[serde(rename = "searchKey")]
    pub search_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Panel {
    pub bar: crate::bar::BarState,
    pub header: PanelHeader,
    pub status: PanelStatus,
    pub sections: Vec<PanelSection>,
    pub footer: String,
    pub navigation: Vec<NavEntry>,
}

/// What a frontend adds to the state: the things it, not the daemon, knows.
#[derive(Debug, Clone, Default)]
pub struct ResolveOptions {
    pub phrase_index: i64,
    pub recent_regions: Vec<String>,
    pub mullvad_picker_open: bool,
    pub expanded_peer_id: String,
    pub now_ms: i64,
}

fn payload<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn update_section(state: &PanelState) -> PanelSection {
    let update = state.update.clone().unwrap_or_default();
    let mut rows = Vec::new();
    if update.available {
        rows.push(PanelRow {
            id: "update".into(),
            kind: "update".into(),
            label: format!("TailGauge {} is available", update.latest),
            // No store owns any part of TailGauge, so there is never a copy
            // the binary may not replace.
            sublabel: "Install it now".into(),
            icon: "software-update-available-symbolic".into(),
            glyph: "\u{f06b0}".into(),
            action: "update".into(),
            busy: state.updating,
            current: true,
            payload: payload(&update),
            ..PanelRow::default()
        });
    }
    PanelSection {
        id: "update".into(),
        // No title: one banner does not need a section header over it.
        title: String::new(),
        visible: update.available,
        empty: String::new(),
        rows,
    }
}

fn providers_section(state: &PanelState) -> PanelSection {
    let current = providers::active_provider(state);
    let rows: Vec<PanelRow> = providers::drivable_providers(state)
        .into_iter()
        .map(|provider| {
            let selected = current.is_some_and(|c| c.id == provider.id);
            PanelRow {
                id: format!("provider:{}", provider.id),
                kind: "provider".into(),
                label: provider.label.into(),
                icon: if selected {
                    "checkmark-symbolic"
                } else {
                    "network-vpn-symbolic"
                }
                .into(),
                glyph: if selected { "\u{f00c}" } else { "\u{f0982}" }.into(),
                action: "switchProvider".into(),
                current: selected,
                bold: selected,
                payload: payload(provider),
                ..PanelRow::default()
            }
        })
        .collect();
    PanelSection {
        id: "providers".into(),
        title: "VPN".into(),
        // One provider is not a choice, and nought is not a list.
        visible: rows.len() > 1,
        empty: String::new(),
        rows,
    }
}

fn self_section(state: &PanelState) -> PanelSection {
    let mut rows = Vec::new();
    if let Some(peer) = state.self_peer.as_ref() {
        let copy_options = peer_copy_options(peer);
        if !copy_options.is_empty() {
            rows.push(PanelRow {
                id: "self".into(),
                kind: "self".into(),
                label: row_label(peer),
                sublabel: peer_subtitle(peer),
                icon: os_icon_name(&peer.os).into(),
                glyph: os_icon(&peer.os).into(),
                action: "copy".into(),
                actions: vec![RowAction::new(
                    "copy",
                    "Copy",
                    "edit-copy-symbolic",
                    "\u{f018f}",
                )],
                copy_options,
                payload: payload(peer),
                ..PanelRow::default()
            });
        }
    }
    PanelSection {
        id: "self".into(),
        title: "This device".into(),
        visible: providers::provider_ready(state) && state.active && !rows.is_empty(),
        empty: String::new(),
        rows,
    }
}

fn row_label(peer: &Peer) -> String {
    if !peer.display_name.is_empty() {
        peer.display_name.clone()
    } else if !peer.host_name.is_empty() {
        peer.host_name.clone()
    } else {
        "Unknown".to_string()
    }
}

fn connections_section(state: &PanelState) -> PanelSection {
    let mut rows = Vec::new();
    if state.accounts_access_denied {
        rows.push(PanelRow {
            id: "auth".into(),
            kind: "auth".into(),
            label: "Authorize Tailscale operator".into(),
            sublabel: "Allow this user to operate this Tailscale profile".into(),
            icon: "security-medium-symbolic".into(),
            glyph: "\u{f0483}".into(),
            action: "authorize".into(),
            busy: state.busy,
            ..PanelRow::default()
        });
    }
    for account in &state.accounts {
        let selected = account.selected == Some(true);
        rows.push(PanelRow {
            id: format!("account:{}", account.id),
            kind: "account".into(),
            label: crate::accounts::account_label(account),
            icon: if selected {
                "checkmark-symbolic"
            } else {
                "user-symbolic"
            }
            .into(),
            // No glyph either way: the account rows have never carried one, so
            // Omarchy - which draws the glyph rather than the icon name - shows
            // them bare. Faithful to the model rather than corrected here, since
            // fixing it changes what a panel looks like.
            glyph: String::new(),
            action: "switchAccount".into(),
            current: selected,
            bold: selected,
            busy: state.switching_account_id == account.id,
            payload: payload(account),
            ..PanelRow::default()
        });
    }
    PanelSection {
        id: "connections".into(),
        title: "Connections".into(),
        visible: providers::provider_supports(state, Capability::Accounts)
            && (state.accounts.len() > 1 || state.accounts_access_denied),
        empty: String::new(),
        rows,
    }
}

fn exit_node_row(state: &PanelState, node: &Peer) -> PanelRow {
    let active = node.exit_node;
    PanelRow {
        id: format!("exit:{}", node.id),
        kind: "exitNode".into(),
        label: row_label(node),
        icon: if node.mullvad {
            "network-vpn-symbolic"
        } else {
            "network-connect-symbolic"
        }
        .into(),
        glyph: if node.mullvad {
            "\u{f0582}"
        } else {
            "\u{f11e2}"
        }
        .into(),
        action: "setExitNode".into(),
        current: active,
        bold: active,
        busy: state.setting_exit_node_id == node.id,
        hint: if active { "Disconnect" } else { "Connect" }.into(),
        payload: payload(node),
        ..PanelRow::default()
    }
}

fn exit_node_rows(
    state: &PanelState,
    recent_regions: &[String],
    picker_open: bool,
) -> Vec<PanelRow> {
    let mut rows: Vec<PanelRow> = state
        .own_exit_nodes
        .iter()
        .map(|node| exit_node_row(state, node))
        .collect();

    let regions: &[Peer] = if providers::provider_supports(state, Capability::Mullvad) {
        &state.mullvad_regions
    } else {
        &[]
    };
    for node in crate::exit_nodes::recent_mullvad_nodes(regions, recent_regions, 5) {
        rows.push(exit_node_row(state, &node));
    }

    if !regions.is_empty() {
        let mut children = vec![PanelRow {
            id: "mullvad:empty".into(),
            kind: "empty".into(),
            label: "No Mullvad regions found.".into(),
            navigable: false,
            search_scope: "mullvad".into(),
            ..PanelRow::default()
        }];
        for region in regions {
            let active = region.exit_node;
            children.push(PanelRow {
                id: format!("region:{}", region.id),
                kind: "mullvadRegion".into(),
                label: crate::exit_nodes::mullvad_region_title(region),
                sublabel: crate::exit_nodes::mullvad_region_subtitle(region),
                icon: "network-vpn-symbolic".into(),
                glyph: "\u{f0582}".into(),
                action: "setExitNode".into(),
                current: active,
                bold: active,
                busy: state.setting_exit_node_id == region.id,
                hint: if active { "Disconnect" } else { "Connect" }.into(),
                search_scope: "mullvad".into(),
                search_key: crate::exit_nodes::mullvad_region_search_key(region),
                payload: payload(region),
                ..PanelRow::default()
            });
        }
        rows.push(PanelRow {
            id: "mullvad:add".into(),
            kind: "mullvadPicker".into(),
            label: "Choose Mullvad region".into(),
            icon: "list-add-symbolic".into(),
            glyph: "+".into(),
            action: "togglePicker".into(),
            current: picker_open,
            expanded: picker_open,
            search_placeholder: "Search regions".into(),
            children,
            ..PanelRow::default()
        });
    }
    rows
}

fn exit_nodes_section(
    state: &PanelState,
    recent_regions: &[String],
    picker_open: bool,
) -> PanelSection {
    let supported = providers::provider_supports(state, Capability::ExitNodes);
    let rows = if supported && state.active {
        exit_node_rows(state, recent_regions, picker_open)
    } else {
        Vec::new()
    };
    PanelSection {
        id: "exitNodes".into(),
        title: "Exit nodes".into(),
        visible: supported && state.active && !rows.is_empty(),
        empty: String::new(),
        rows,
    }
}

fn networks_section(state: &PanelState) -> PanelSection {
    let supported = providers::provider_supports(state, Capability::Networks);
    let networks: &[crate::netbird::Network] = if supported { &state.networks } else { &[] };
    let rows: Vec<PanelRow> = networks
        .iter()
        .map(|network| PanelRow {
            id: format!("network:{}", network.id),
            kind: "network".into(),
            label: network.id.clone(),
            sublabel: crate::netbird::network_subtitle(network),
            icon: if network.selected {
                "checkmark-symbolic"
            } else {
                "network-workgroup-symbolic"
            }
            .into(),
            glyph: if network.selected {
                "\u{f00c}"
            } else {
                "\u{f06db}"
            }
            .into(),
            action: "selectNetwork".into(),
            current: network.selected,
            bold: network.selected,
            busy: state.selecting_network_id == network.id,
            hint: if network.selected { "Leave" } else { "Join" }.into(),
            payload: payload(network),
            ..PanelRow::default()
        })
        .collect();
    PanelSection {
        id: "networks".into(),
        title: "Networks".into(),
        visible: supported && state.active && !rows.is_empty(),
        empty: String::new(),
        rows,
    }
}

/// A field over three machines is clutter; over eighty it is the only way to
/// find one.
const MACHINE_SEARCH_MIN: usize = 8;

fn machines_section(state: &PanelState, expanded_peer_id: &str, now_ms: i64) -> PanelSection {
    let peers: &[Peer] = if state.active { &state.peers } else { &[] };
    let mut rows: Vec<PanelRow> = Vec::new();

    if peers.len() > MACHINE_SEARCH_MIN {
        rows.push(PanelRow {
            id: "machines:search".into(),
            kind: "machineSearch".into(),
            search_placeholder: "Search machines".into(),
            navigable: false,
            ..PanelRow::default()
        });
    }
    if !peers.is_empty() {
        rows.push(PanelRow {
            id: "machines:empty".into(),
            kind: "empty".into(),
            label: "No machines match.".into(),
            navigable: false,
            search_scope: "machines".into(),
            ..PanelRow::default()
        });
    }

    for peer in peers {
        let copy_options = peer_copy_options(peer);
        let details = peer_detail_rows(peer, now_ms);
        let expanded = !details.is_empty() && expanded_peer_id == peer.id;

        let mut actions: Vec<RowAction> = Vec::new();
        if !details.is_empty() {
            actions.push(if expanded {
                RowAction::new("detail", "Hide details", "pan-up-symbolic", "\u{f0143}")
            } else {
                RowAction::new("detail", "Show details", "pan-down-symbolic", "\u{f0140}")
            });
        }
        if can_send_files(state, peer) {
            actions.push(RowAction::new(
                "send",
                "Send files",
                "document-send-symbolic",
                "\u{f048a}",
            ));
        }
        if !copy_options.is_empty() {
            actions.push(RowAction::new(
                "copy",
                "Copy",
                "edit-copy-symbolic",
                "\u{f018f}",
            ));
        }

        rows.push(PanelRow {
            id: format!("peer:{}", peer.id),
            kind: "peer".into(),
            label: row_label(peer),
            sublabel: peer_row_subtitle(peer),
            icon: os_icon_name(&peer.os).into(),
            glyph: os_icon(&peer.os).into(),
            action: if copy_options.is_empty() {
                String::new()
            } else {
                "copy".into()
            },
            actions,
            copy_options,
            children: details,
            expanded,
            search_scope: "machines".into(),
            search_key: crate::exit_nodes::machine_search_key(peer),
            payload: payload(peer),
            ..PanelRow::default()
        });
    }

    PanelSection {
        id: "machines".into(),
        title: "Machines".into(),
        visible: providers::provider_ready(state) && state.active,
        empty: "No machines found on this tailnet.".into(),
        rows,
    }
}

/// One traversal order for every desktop: the header, then every navigable row
/// of every visible section, in the order they are drawn. Cursor movement is
/// an index into this, so no frontend carries a focus state machine that
/// another could disagree with.
fn panel_navigation(header: &PanelHeader, sections: &[PanelSection]) -> Vec<NavEntry> {
    let entry = |section_id: &str, row: &PanelRow| NavEntry {
        section_id: section_id.into(),
        row_id: row.id.clone(),
        action: row.action.clone(),
        search_scope: row.search_scope.clone(),
        search_key: row.search_key.clone(),
    };

    let mut nav = vec![NavEntry {
        section_id: "header".into(),
        row_id: header.id.clone(),
        action: header.action.clone(),
        search_scope: String::new(),
        search_key: String::new(),
    }];
    for section in sections.iter().filter(|s| s.visible) {
        for row in section.rows.iter().filter(|r| r.navigable) {
            nav.push(entry(&section.id, row));
            // An expanded row's children are drawn between it and the next
            // row, so they are cursor stops in that position too. Collapsed,
            // they are not on screen and must not be.
            if !row.expanded {
                continue;
            }
            for child in row.children.iter().filter(|c| c.navigable) {
                nav.push(entry(&section.id, child));
            }
        }
    }
    nav
}

pub fn panel_spec(state: &PanelState, options: &ResolveOptions) -> Panel {
    let header = panel_header(state, options.phrase_index);
    let sections = vec![
        update_section(state),
        providers_section(state),
        self_section(state),
        connections_section(state),
        exit_nodes_section(state, &options.recent_regions, options.mullvad_picker_open),
        networks_section(state),
        machines_section(state, &options.expanded_peer_id, options.now_ms),
    ];
    Panel {
        bar: crate::bar::bar_state(state),
        navigation: panel_navigation(&header, &sections),
        header,
        status: panel_status(state),
        sections,
        footer: panel_footer(state),
    }
}

/// Resolve a navigation entry back to the row it points at, so a frontend can
/// act on the cursor without keeping its own copy of the panel.
pub fn panel_row_at(panel: &Panel, nav_index: usize) -> Option<&PanelRow> {
    let entry = panel.navigation.get(nav_index)?;
    if entry.section_id == "header" {
        return None;
    }
    let section = panel.sections.iter().find(|s| s.id == entry.section_id)?;
    for row in &section.rows {
        if row.id == entry.row_id {
            return Some(row);
        }
        if let Some(child) = row.children.iter().find(|c| c.id == entry.row_id) {
            return Some(child);
        }
    }
    None
}

/// Whether a row is drawn at all. A search field is the model's decision, so a
/// frontend holding its query needs to hear when it has gone.
pub fn panel_has_row(panel: &Panel, row_id: &str) -> bool {
    panel.sections.iter().any(|section| {
        section
            .rows
            .iter()
            .any(|row| row.id == row_id || row.children.iter().any(|c| c.id == row_id))
    })
}

/// What a row's single-letter keys are allowed to do follows the actions the
/// model put on it, not its kind, so a new copyable row does not have to be
/// taught to every frontend's key handler.
pub fn panel_row_has_action(row: &PanelRow, action_id: &str) -> bool {
    row.actions.iter().any(|a| a.id == action_id)
}

pub fn panel_nav_index_of(panel: &Panel, row_id: &str) -> usize {
    panel
        .navigation
        .iter()
        .position(|e| e.row_id == row_id)
        .unwrap_or(0)
}
