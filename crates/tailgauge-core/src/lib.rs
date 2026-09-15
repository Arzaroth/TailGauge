//! Everything TailGauge knows, so that no frontend has to.
//!
//! The parsers read what a VPN CLI printed, and `panel` turns what they made
//! of it into the panel a frontend draws: the bar, the header, every section
//! and the cursor's traversal order. A frontend picks it up from
//! `tailgauge panel --json` and draws it.
//!
//! `tests/specification.rs` is where the behaviour is pinned, case by case,
//! against the daemon output in `tests/fixtures`.

pub mod accounts;
pub mod bar;
pub mod exit_nodes;
pub mod fmt;
pub mod netbird;
pub mod panel;
pub mod panel_state;
pub mod peer;
pub mod providers;
pub mod status;

pub use accounts::{Account, AccountsResult, account_label, parse_accounts};
pub use exit_nodes::{mullvad_region_key, mullvad_region_options, parse_exit_node_list};
pub use netbird::{Network, NetworksResult, parse_netbird_networks, parse_netbird_status};
pub use peer::Peer;
pub use status::{StatusResult, parse_status};
