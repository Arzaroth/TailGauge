//! Everything TailGauge knows, so that no frontend has to.
//!
//! This is the Rust half of a port in progress. `shared/model.ts` is still the
//! copy the three frontends run on; nothing here drives a panel yet. What keeps
//! the two honest is the differential harness: `tailgauge internal-model` feeds
//! a fixture to the code below and prints the result as JSON, and
//! `test/differential.test.ts` feeds the same fixture to the TypeScript and
//! fails if the two answers differ.
//!
//! Ported in dependency order - the parsers first, then the panel they feed -
//! so the harness has something to compare at every step.

pub mod accounts;
pub mod exit_nodes;
pub mod netbird;
pub mod peer;
pub mod status;

pub use accounts::{Account, AccountsResult, account_label, parse_accounts};
pub use exit_nodes::{mullvad_region_key, mullvad_region_options, parse_exit_node_list};
pub use netbird::{Network, NetworksResult, parse_netbird_networks, parse_netbird_status};
pub use peer::Peer;
pub use status::{StatusResult, parse_status};
