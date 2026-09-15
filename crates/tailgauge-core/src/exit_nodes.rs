//! `tailscale exit-node list`: a fixed-width table, and the Mullvad regions
//! drawn out of it.

use crate::peer::{self, Peer};

/// The columns are sliced at the header's offsets rather than split on
/// whitespace, because CITY holds names with spaces in them.
fn slice_column(line: &str, start: usize, end: Option<usize>) -> String {
    if start >= line.len() {
        return String::new();
    }
    let mut lo = start;
    while lo < line.len() && !line.is_char_boundary(lo) {
        lo += 1;
    }
    let hi = match end {
        None => line.len(),
        Some(end) => {
            let mut hi = end.min(line.len());
            while hi < line.len() && !line.is_char_boundary(hi) {
                hi += 1;
            }
            hi
        }
    };
    if hi <= lo {
        return String::new();
    }
    line[lo..hi].trim().to_string()
}

fn header_offsets(line: &str) -> Option<(usize, usize, usize, usize, usize)> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields != ["IP", "HOSTNAME", "COUNTRY", "CITY", "STATUS"] {
        return None;
    }
    Some((
        line.find("IP")?,
        line.find("HOSTNAME")?,
        line.find("COUNTRY")?,
        line.find("CITY")?,
        line.find("STATUS")?,
    ))
}

/// Every Mullvad server the table lists. Non-Mullvad rows are dropped: the
/// tailnet's own exit nodes come out of the status document, where they carry
/// an owner and an address.
pub fn parse_exit_node_list(raw: &str) -> Vec<Peer> {
    let lines: Vec<&str> = raw
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let Some(header_index) = lines.iter().position(|l| header_offsets(l).is_some()) else {
        return Vec::new();
    };
    let (ip_at, host_at, country_at, city_at, status_at) =
        header_offsets(lines[header_index]).expect("just matched");

    // Keyed by host, keeping the position a host was first seen: a later row
    // for the same server replaces it without moving it, which is what
    // reassigning an object key does in the TypeScript.
    let mut by_host: Vec<(String, Peer)> = Vec::new();

    for line in &lines[header_index + 1..] {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let ip = slice_column(line, ip_at, Some(host_at));
        let host = slice_column(line, host_at, Some(country_at));
        let country = slice_column(line, country_at, Some(city_at));
        let city = slice_column(line, city_at, Some(status_at));
        let status = slice_column(line, status_at, None);
        if !peer::is_mullvad_host(&host) {
            continue;
        }

        let node = Peer {
            id: format!("mullvad:{host}"),
            host_name: host.clone(),
            dns_name: host.clone(),
            display_name: if !city.is_empty() && city != "Any" {
                format!("{city}, {country}")
            } else {
                country.clone()
            },
            ipv4: if ip.is_empty() { Vec::new() } else { vec![ip] },
            ipv6: Vec::new(),
            online: true,
            os: "mullvad".into(),
            tags: Vec::new(),
            exit_node_option: true,
            exit_node: !status.is_empty() && status != "-",
            mullvad: true,
            country: Some(country),
            city: Some(city),
            status: Some(status),
            ..Peer::default()
        };

        match by_host.iter_mut().find(|(h, _)| *h == host) {
            Some(slot) => slot.1 = node,
            None => by_host.push((host, node)),
        }
    }

    let mut result: Vec<Peer> = by_host.into_iter().map(|(_, node)| node).collect();
    result.sort_by(|a, b| {
        peer::collate(
            a.country.as_deref().unwrap_or(""),
            b.country.as_deref().unwrap_or(""),
        )
        .then_with(|| peer::collate(&a.display_name, &b.display_name))
    });
    result
}

/// One entry per city, because a region is what the panel offers - the
/// individual servers behind it are Mullvad's business.
pub fn mullvad_region_options(nodes: &[Peer]) -> Vec<Peer> {
    let mut by_region: Vec<(String, Peer)> = Vec::new();
    for node in nodes {
        if !node.mullvad {
            continue;
        }
        let country = node.country.as_deref().unwrap_or("").trim().to_string();
        let city = node.city.as_deref().unwrap_or("").trim().to_string();
        if country.is_empty() || city.is_empty() || city == "Any" {
            continue;
        }
        let key = format!("{country}\n{city}");
        if by_region.iter().any(|(k, _)| *k == key) {
            continue;
        }
        let option = Peer {
            id: format!("mullvad-region:{key}"),
            display_name: format!("{city}, {country}"),
            country: Some(country),
            city: Some(city),
            mullvad_region: Some(true),
            ..node.clone()
        };
        by_region.push((key, option));
    }

    let mut result: Vec<Peer> = by_region.into_iter().map(|(_, node)| node).collect();
    result.sort_by(|a, b| {
        peer::collate(
            a.country.as_deref().unwrap_or(""),
            b.country.as_deref().unwrap_or(""),
        )
        .then_with(|| {
            peer::collate(
                a.city.as_deref().unwrap_or(""),
                b.city.as_deref().unwrap_or(""),
            )
        })
    });
    result
}

/// How a region is remembered between sessions: the pair that names it, not
/// the server that happened to serve it.
pub fn mullvad_region_key(node: &Peer) -> String {
    let country = node.country.as_deref().unwrap_or("");
    let city = node.city.as_deref().unwrap_or("");
    if country.is_empty() || city.is_empty() {
        return String::new();
    }
    format!("{country}\n{city}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
IP             HOSTNAME                      COUNTRY        CITY           STATUS
100.100.0.1    de-ber-wg-001.mullvad.ts.net  Germany        Berlin         -
100.100.0.2    fr-par-wg-101.mullvad.ts.net  France         Paris          selected
100.100.0.3    us-nyc-wg-201.mullvad.ts.net  USA            New York       -
100.100.0.4    router.example.ts.net         -              -              -
";

    #[test]
    fn a_table_without_its_header_reads_as_nothing() {
        assert!(parse_exit_node_list("").is_empty());
        assert!(parse_exit_node_list("some error\n").is_empty());
    }

    #[test]
    fn only_mullvad_rows_come_out_of_the_table() {
        let nodes = parse_exit_node_list(TABLE);
        assert_eq!(
            nodes.len(),
            3,
            "the tailnet's own node is not a Mullvad server"
        );
        assert!(nodes.iter().all(|n| n.mullvad && n.exit_node_option));
    }

    #[test]
    fn a_city_with_a_space_in_it_stays_one_field() {
        let nodes = parse_exit_node_list(TABLE);
        let nyc = nodes
            .iter()
            .find(|n| n.city.as_deref() == Some("New York"))
            .expect("New York");
        assert_eq!(nyc.display_name, "New York, USA");
        assert_eq!(nyc.country.as_deref(), Some("USA"));
    }

    #[test]
    fn the_status_column_says_which_node_is_in_use() {
        let nodes = parse_exit_node_list(TABLE);
        let active: Vec<&str> = nodes
            .iter()
            .filter(|n| n.exit_node)
            .map(|n| n.host_name.as_str())
            .collect();
        assert_eq!(active, ["fr-par-wg-101.mullvad.ts.net"]);
    }

    #[test]
    fn rows_are_ordered_by_country_then_by_what_the_row_shows() {
        let nodes = parse_exit_node_list(TABLE);
        assert_eq!(
            nodes
                .iter()
                .map(|n| n.display_name.as_str())
                .collect::<Vec<_>>(),
            ["Paris, France", "Berlin, Germany", "New York, USA"]
        );
    }

    #[test]
    fn a_region_is_one_entry_however_many_servers_serve_it() {
        let table = format!(
            "{TABLE}100.100.0.5    de-ber-wg-002.mullvad.ts.net  Germany        Berlin         -\n"
        );
        let regions = mullvad_region_options(&parse_exit_node_list(&table));
        assert_eq!(
            regions
                .iter()
                .map(|r| r.display_name.as_str())
                .collect::<Vec<_>>(),
            ["Paris, France", "Berlin, Germany", "New York, USA"]
        );
        assert!(regions.iter().all(|r| r.mullvad_region == Some(true)));
    }

    #[test]
    fn a_region_without_a_city_is_not_a_region() {
        let any = Peer {
            mullvad: true,
            country: Some("Germany".into()),
            city: Some("Any".into()),
            ..Peer::default()
        };
        assert!(mullvad_region_options(std::slice::from_ref(&any)).is_empty());
        assert_eq!(mullvad_region_key(&any), "Germany\nAny");
        assert_eq!(mullvad_region_key(&Peer::default()), "");
    }
}
