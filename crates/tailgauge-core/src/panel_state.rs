//! What a frontend hands the resolver: everything it has read, and nothing it
//! has decided.

use serde::{Deserialize, Serialize};

use crate::accounts::Account;
use crate::netbird::Network;
use crate::peer::Peer;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderState {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub installed: bool,
}

/// One provider's state, gathered whether or not the panel is showing it: the
/// bar describes the machine, not the view.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderSummary {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub running: bool,
    #[serde(default, rename = "needsLogin")]
    pub needs_login: bool,
    #[serde(default, rename = "selfName")]
    pub self_name: String,
    #[serde(default, rename = "selfIp")]
    pub self_ip: String,
    #[serde(default)]
    pub state: String,
}

/// What `tailgauge --check-update` last reported, as the panel reads it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateInfo {
    #[serde(default)]
    pub current: String,
    #[serde(default)]
    pub latest: String,
    #[serde(default)]
    pub available: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelState {
    pub providers: Option<Vec<ProviderState>>,
    #[serde(rename = "activeProviderId")]
    pub active_provider_id: String,
    pub summaries: Option<Vec<ProviderSummary>>,
    pub installed: bool,
    pub running: bool,
    pub active: bool,
    #[serde(rename = "needsLogin")]
    pub needs_login: bool,
    pub busy: bool,
    pub helpers: bool,
    pub updating: bool,
    #[serde(rename = "selfName")]
    pub self_name: String,
    #[serde(rename = "selfIp")]
    pub self_ip: String,
    #[serde(rename = "selfUserId")]
    pub self_user_id: String,
    #[serde(rename = "selfPeer")]
    pub self_peer: Option<Peer>,
    #[serde(rename = "fileSharing")]
    pub file_sharing: bool,
    pub peers: Vec<Peer>,
    #[serde(rename = "ownExitNodes")]
    pub own_exit_nodes: Vec<Peer>,
    pub networks: Vec<Network>,
    #[serde(rename = "selectingNetworkId")]
    pub selecting_network_id: String,
    #[serde(rename = "mullvadRegions")]
    pub mullvad_regions: Vec<Peer>,
    pub accounts: Vec<Account>,
    #[serde(rename = "selectedAccountId")]
    pub selected_account_id: String,
    #[serde(rename = "switchingAccountId")]
    pub switching_account_id: String,
    #[serde(rename = "settingExitNodeId")]
    pub setting_exit_node_id: String,
    #[serde(rename = "accountsAccessDenied")]
    pub accounts_access_denied: bool,
    #[serde(rename = "actionStatus")]
    pub action_status: String,
    #[serde(rename = "lastError")]
    pub last_error: String,
    pub update: Option<UpdateInfo>,
    pub version: String,
}
