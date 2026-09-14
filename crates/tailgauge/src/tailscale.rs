//! What the `tailscale` CLI is asked, in the one place that asks it.

use serde_json::Value;

use crate::launch;

pub fn installed() -> bool {
    launch::has("tailscale")
}

pub fn status() -> Option<Value> {
    let raw = launch::output("tailscale", ["status", "--json"])?;
    serde_json::from_str(&raw).ok()
}

pub fn backend_state(status: &Value) -> &str {
    status
        .pointer("/BackendState")
        .and_then(Value::as_str)
        .unwrap_or("")
}

pub fn running(status: &Value) -> bool {
    backend_state(status) == "Running"
}

pub fn self_host_name(status: &Value) -> Option<&str> {
    status.pointer("/Self/HostName").and_then(Value::as_str)
}

/// The machine's own tailnet address, v4 for preference: it is the one anybody
/// reads out loud.
pub fn self_ip(status: &Value) -> Option<&str> {
    let ips = status
        .pointer("/Self/TailscaleIPs")
        .and_then(Value::as_array)?;
    let addrs = || ips.iter().filter_map(Value::as_str);
    addrs()
        .find(|ip| ip.contains('.'))
        .or_else(|| addrs().next())
}

/// The exit node currently routing this machine, by hostname.
///
/// Read off `exit-node list` rather than the status JSON, because a Mullvad
/// node is not in the peer list until it is in use, and the name in the table
/// is the one the panel and the CLI both accept back.
pub fn current_exit_node() -> Option<String> {
    let table = launch::output("tailscale", ["exit-node", "list"])?;
    first_active_host(&table)
}

/// `exit-node list` is a fixed-width table, and CITY holds names with spaces
/// in them - so the columns are sliced at the header offsets rather than split
/// on whitespace.
fn first_active_host(table: &str) -> Option<String> {
    let mut lines = table.lines();
    let header = lines.find(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        fields == ["IP", "HOSTNAME", "COUNTRY", "CITY", "STATUS"]
    })?;
    let host_at = header.find("HOSTNAME")?;
    let country_at = header.find("COUNTRY")?;
    let status_at = header.find("STATUS")?;

    for line in lines {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let state = column(line, status_at, usize::MAX);
        if state.is_empty() || state == "-" {
            continue;
        }
        let host = column(line, host_at, country_at);
        if !host.is_empty() {
            return Some(host.to_string());
        }
    }
    None
}

fn column(line: &str, start: usize, end: usize) -> &str {
    if line.len() <= start {
        return "";
    }
    let end = end.min(line.len());
    if end <= start {
        return "";
    }
    // Byte offsets from an ASCII header, so a multi-byte name in a later
    // column must not panic the slice.
    let mut lo = start;
    while lo < line.len() && !line.is_char_boundary(lo) {
        lo += 1;
    }
    let mut hi = end;
    while hi < line.len() && !line.is_char_boundary(hi) {
        hi += 1;
    }
    line[lo..hi].trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
IP             HOSTNAME                      COUNTRY        CITY           STATUS
100.100.0.1    de-ber-wg-001.mullvad.ts.net  Germany        Berlin         -
100.100.0.2    fr-par-wg-101.mullvad.ts.net  France         Paris          selected
100.100.0.3    us-nyc-wg-201.mullvad.ts.net  USA            New York       -
";

    #[test]
    fn the_selected_row_is_the_current_exit_node() {
        assert_eq!(
            first_active_host(TABLE).as_deref(),
            Some("fr-par-wg-101.mullvad.ts.net")
        );
    }

    #[test]
    fn a_table_with_nothing_selected_reports_none() {
        let idle = TABLE.replace("selected", "-");
        assert_eq!(first_active_host(&idle), None);
        assert_eq!(first_active_host(""), None);
        assert_eq!(first_active_host("no header here\n"), None);
    }

    #[test]
    fn a_city_with_a_space_in_it_does_not_split_the_row() {
        // The reason the columns are sliced at all: "New York" would be two
        // fields to anything splitting on whitespace, and STATUS would read as
        // the second half of the city.
        let table = TABLE.replace("New York       -", "New York       selected");
        assert_eq!(
            first_active_host(&table).as_deref(),
            Some("fr-par-wg-101.mullvad.ts.net"),
            "the first selected row still wins"
        );
    }

    #[test]
    fn a_short_row_is_not_a_slice_out_of_bounds() {
        let table = format!("{TABLE}100.100.0.4\n");
        assert!(first_active_host(&table).is_some());
    }

    #[test]
    fn the_v4_address_is_the_one_reported() {
        let status = serde_json::json!({
            "Self": {"HostName": "workstation", "TailscaleIPs": ["fd7a::1", "100.64.0.1"]}
        });
        assert_eq!(self_ip(&status), Some("100.64.0.1"));
        assert_eq!(self_host_name(&status), Some("workstation"));

        let v6_only = serde_json::json!({"Self": {"TailscaleIPs": ["fd7a::1"]}});
        assert_eq!(self_ip(&v6_only), Some("fd7a::1"));
        assert_eq!(self_ip(&serde_json::json!({})), None);
    }
}
