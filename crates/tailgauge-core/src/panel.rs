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
