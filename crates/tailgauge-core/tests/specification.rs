//! The specification, ported from `test/model.test.ts`.
//!
//! The differential harness proves the Rust agrees with the TypeScript, but it
//! dies with the TypeScript. These are the tests that say what the panel
//! *should* do, against the same fixtures, so that deleting `shared/model.ts`
//! deletes a second implementation rather than the specification.
//!
//! Kept in the order and the wording of the file they came from, so the two
//! can be read side by side while both exist.

use std::path::PathBuf;
use std::sync::LazyLock;

use tailgauge_core as core;
use tailgauge_core::accounts::AccountsResult;
use tailgauge_core::panel::{Panel, PanelRow, PanelSection, ResolveOptions};
use tailgauge_core::panel_state::{PanelState, ProviderState};
use tailgauge_core::peer::Peer;
use tailgauge_core::providers::PROVIDERS;
use tailgauge_core::status::StatusOk;

// ---- the fixtures the panel is resolved from ------------------------------

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("test/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

static STATUS: LazyLock<StatusOk> =
    LazyLock::new(|| match core::parse_status(&fixture("status.json")) {
        core::StatusResult::Ok(status) => *status,
        other => panic!("the fixture should parse as a running tailnet, got {other:?}"),
    });

static ACCOUNTS: LazyLock<AccountsResult> =
    LazyLock::new(|| core::parse_accounts(&fixture("accounts.json")));

static MULLVAD_REGIONS: LazyLock<Vec<Peer>> = LazyLock::new(|| {
    core::mullvad_region_options(&core::parse_exit_node_list(&fixture("exit-nodes.txt")))
});

/// Past the threshold the machines section grows a search field, which the
/// fixture's peers are deliberately too few to trigger.
static MANY_PEERS: LazyLock<Vec<Peer>> = LazyLock::new(|| {
    (0..12)
        .map(|i| Peer {
            id: format!("peer-{i}"),
            host_name: format!("box-{i}"),
            display_name: format!("box-{i}"),
            dns_name: format!("box-{i}.example.ts.net"),
            user_id: Some(STATUS.self_user_id.clone()),
            taildrop_target: Some(1),
            ipv4: vec![format!("100.64.1.{i}")],
            online: true,
            os: if i % 2 == 0 {
                "linux".into()
            } else {
                "windows".into()
            },
            ..Peer::default()
        })
        .collect()
});

fn detected(ids: &[&str]) -> Vec<ProviderState> {
    PROVIDERS
        .iter()
        .map(|p| ProviderState {
            id: p.id.into(),
            installed: ids.contains(&p.id),
        })
        .collect()
}

/// The state the frontends hand the resolver on a healthy tailnet.
fn state() -> PanelState {
    PanelState {
        providers: Some(detected(&["tailscale"])),
        installed: true,
        running: STATUS.running,
        active: STATUS.running,
        needs_login: STATUS.needs_login,
        helpers: true,
        self_name: STATUS.self_name.clone(),
        self_ip: STATUS.self_ip.clone(),
        self_user_id: STATUS.self_user_id.clone(),
        self_peer: Some(STATUS.self_peer.clone()),
        file_sharing: STATUS.file_sharing,
        peers: STATUS.peers.clone(),
        own_exit_nodes: STATUS.exit_nodes.clone(),
        mullvad_regions: MULLVAD_REGIONS.clone(),
        accounts: ACCOUNTS.accounts.clone(),
        selected_account_id: ACCOUNTS.selected_account_id.clone(),
        ..PanelState::default()
    }
}

/// 2026-09-14T06:00:00Z, the moment the fixtures were captured around.
const NOW: i64 = 1_789_365_600_000;

fn resolve(state: &PanelState) -> Panel {
    core::panel::panel_spec(
        state,
        &ResolveOptions {
            now_ms: NOW,
            ..ResolveOptions::default()
        },
    )
}

fn resolve_with(state: &PanelState, options: ResolveOptions) -> Panel {
    core::panel::panel_spec(
        state,
        &ResolveOptions {
            now_ms: NOW,
            ..options
        },
    )
}

fn section<'a>(panel: &'a Panel, id: &str) -> &'a PanelSection {
    panel
        .sections
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("no section {id}"))
}

fn rows_of<'a>(panel: &'a Panel, id: &str) -> &'a [PanelRow] {
    &section(panel, id).rows
}

fn peer_rows(panel: &Panel) -> Vec<&PanelRow> {
    rows_of(panel, "machines")
        .iter()
        .filter(|r| r.kind == "peer")
        .collect()
}

fn labelled<'a>(rows: &'a [PanelRow], label: &str) -> &'a PanelRow {
    rows.iter()
        .find(|r| r.label == label)
        .unwrap_or_else(|| panic!("no row labelled {label}"))
}

/// What each frontend does with a query: the model decides what a row is
/// findable by, and the substring test lives on the other side of the seam.
fn matching<'a>(rows: &'a [PanelRow], scope: &str, query: &str) -> Vec<&'a PanelRow> {
    let needle = query.trim().to_lowercase();
    rows.iter()
        .filter(|r| {
            r.search_scope == scope && !r.search_key.is_empty() && r.search_key.contains(&needle)
        })
        .collect()
}

fn picker_of(rows: &[PanelRow]) -> &PanelRow {
    rows.iter()
        .find(|r| r.kind == "mullvadPicker")
        .expect("Mullvad picker row")
}

// ---- the parsers ----------------------------------------------------------

#[test]
fn parse_status_reads_the_running_tailnet() {
    assert!(STATUS.running);
    assert!(!STATUS.needs_login);
    assert_eq!(STATUS.self_name, "workstation");
    assert_eq!(STATUS.self_dns_name, "workstation.example.ts.net");
    assert_eq!(STATUS.self_ip, "100.64.0.1");
    assert!(STATUS.file_sharing);
}

#[test]
fn parse_status_keeps_every_non_mullvad_peer_online_ones_first() {
    let names: Vec<&str> = STATUS.peers.iter().map(|p| p.host_name.as_str()).collect();
    assert_eq!(names, ["laptop", "phone", "router", "offline-box"]);
    assert!(
        !STATUS.peers.iter().any(|p| p.mullvad),
        "a Mullvad node is not a machine on the tailnet"
    );
}

#[test]
fn parse_status_separates_tailscale_ipv4_from_ipv6() {
    let laptop = STATUS
        .peers
        .iter()
        .find(|p| p.host_name == "laptop")
        .expect("laptop");
    assert_eq!(laptop.ipv4, ["100.64.0.2"]);
    assert!(
        laptop
            .ipv6
            .iter()
            .all(|ip| ip.starts_with("fd7a:115c:a1e0:"))
    );
}

#[test]
fn parse_status_collects_exit_node_options() {
    assert!(!STATUS.exit_nodes.is_empty());
    assert!(
        STATUS
            .exit_nodes
            .iter()
            .all(|n| n.exit_node_option && n.online)
    );
}

#[test]
fn parse_status_survives_empty_and_malformed_input() {
    assert!(matches!(
        core::parse_status(""),
        core::StatusResult::Unavailable(_)
    ));
    assert!(matches!(
        core::parse_status("   "),
        core::StatusResult::Unavailable(_)
    ));
    assert!(matches!(
        core::parse_status("not json"),
        core::StatusResult::Error(_)
    ));
}

#[test]
fn parse_accounts_finds_the_selected_profile() {
    assert!(ACCOUNTS.accounts.len() > 1);
    assert!(!ACCOUNTS.selected_account_id.is_empty());
    assert!(!ACCOUNTS.selected_account_label.is_empty());
    let selected = ACCOUNTS
        .accounts
        .iter()
        .find(|a| a.selected == Some(true))
        .expect("a selected account");
    assert_eq!(selected.id, ACCOUNTS.selected_account_id);
}

#[test]
fn parse_exit_node_list_reads_the_table_and_drops_non_mullvad_hosts() {
    let nodes = core::parse_exit_node_list(&fixture("exit-nodes.txt"));
    assert!(!nodes.is_empty());
    assert!(
        nodes
            .iter()
            .all(|n| n.mullvad && n.exit_node_option && n.os == "mullvad")
    );
}

#[test]
fn mullvad_region_options_drops_the_any_city_and_dedupes() {
    let regions = &*MULLVAD_REGIONS;
    assert!(!regions.is_empty());
    assert!(regions.iter().all(|r| r.mullvad_region == Some(true)));
    assert!(
        !regions.iter().any(|r| r.city.as_deref() == Some("Any")),
        "a country-wide entry is not a region"
    );
    let mut keys: Vec<String> = regions.iter().map(core::mullvad_region_key).collect();
    let before = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), before, "one entry per city");
}

#[test]
fn recent_mullvad_regions_dedupe_and_put_the_active_one_first() {
    let mut regions = MULLVAD_REGIONS.clone();
    regions[1].exit_node = true;
    let active = core::mullvad_region_key(&regions[1]);
    let recent = vec![core::mullvad_region_key(&regions[0]), active.clone()];

    let shortlist = core::exit_nodes::recent_mullvad_nodes(&regions, &recent, 5);
    assert_eq!(core::mullvad_region_key(&shortlist[0]), active);
    let keys: Vec<String> = shortlist.iter().map(core::mullvad_region_key).collect();
    let mut unique = keys.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), keys.len(), "and never twice");
}

#[test]
fn taildrop_targets_follow_the_daemon_grading() {
    let graded = Peer {
        taildrop_target: Some(1),
        user_id: Some("other".into()),
        ..Peer::default()
    };
    assert!(
        core::panel::is_taildrop_target(&graded, "me"),
        "the daemon's answer wins"
    );

    let refused = Peer {
        taildrop_target: Some(2),
        user_id: Some("me".into()),
        ..Peer::default()
    };
    assert!(!core::panel::is_taildrop_target(&refused, "me"));

    // No grading: your own machines, which is the older rule.
    let mine = Peer {
        user_id: Some("me".into()),
        ..Peer::default()
    };
    assert!(core::panel::is_taildrop_target(&mine, "me"));
    assert!(!core::panel::is_taildrop_target(&mine, "someone-else"));
    assert!(!core::panel::is_taildrop_target(&Peer::default(), ""));
}

#[test]
fn the_machine_subtitle_spends_the_second_slot_on_the_owner() {
    let laptop = STATUS
        .peers
        .iter()
        .find(|p| p.host_name == "laptop")
        .expect("laptop");
    assert_eq!(
        core::panel::peer_subtitle(laptop),
        "100.64.0.2 \u{b7} Alice"
    );

    // With no owner, the DNS name takes the slot rather than leaving it empty.
    let anonymous = Peer {
        ipv4: vec!["100.64.0.9".into()],
        dns_name: "box.example.ts.net".into(),
        ..Peer::default()
    };
    assert_eq!(
        core::panel::peer_subtitle(&anonymous),
        "100.64.0.9 \u{b7} box.example.ts.net"
    );
}

#[test]
fn peers_carry_the_owner_the_status_map_names() {
    let laptop = STATUS
        .peers
        .iter()
        .find(|p| p.host_name == "laptop")
        .expect("laptop");
    assert_eq!(laptop.user_name.as_deref(), Some("Alice"));
}

#[test]
fn shell_quoting_survives_an_apostrophe() {
    assert_eq!(
        core::fmt::shell_command(&["echo".into(), "it's".into()]),
        "'echo' 'it'\\''s'"
    );
}

// ---- the sections ---------------------------------------------------------

#[test]
fn sections_come_back_in_a_fixed_order() {
    let panel = resolve(&state());
    let ids: Vec<&str> = panel.sections.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
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
}

#[test]
fn this_device_carries_the_same_copy_options_a_machine_row_does() {
    let panel = resolve(&state());
    let this = section(&panel, "self");
    assert!(this.visible);
    assert_eq!(this.rows.len(), 1);

    let row = &this.rows[0];
    assert_eq!(row.id, "self");
    assert_eq!(row.label, "workstation");
    assert_eq!(row.sublabel, "100.64.0.1 \u{b7} Alice");
    assert_eq!(
        row.copy_options
            .iter()
            .map(|o| o.kind.as_str())
            .collect::<Vec<_>>(),
        ["name", "dns", "ipv6", "ip"]
    );
    assert_eq!(
        row.actions
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["copy"]
    );
    assert!(!core::panel::panel_row_has_action(row, "send"));
}

#[test]
fn this_device_disappears_with_the_tailnet_it_belongs_to() {
    let down = PanelState {
        active: false,
        running: false,
        ..state()
    };
    assert!(!section(&resolve(&down), "self").visible);

    let gone = PanelState {
        providers: None,
        installed: false,
        ..state()
    };
    assert!(!section(&resolve(&gone), "self").visible);

    let nameless = PanelState {
        self_peer: None,
        ..state()
    };
    assert!(!section(&resolve(&nameless), "self").visible);
}

#[test]
fn connections_appear_only_with_a_choice_to_make() {
    assert!(section(&resolve(&state()), "connections").visible);

    let alone = PanelState {
        accounts: Vec::new(),
        ..state()
    };
    assert!(!section(&resolve(&alone), "connections").visible);

    let refused = PanelState {
        accounts_access_denied: true,
        ..alone
    };
    assert!(section(&resolve(&refused), "connections").visible);
}

#[test]
fn the_operator_prompt_leads_the_connections_section() {
    let refused = PanelState {
        accounts_access_denied: true,
        ..state()
    };
    let panel = resolve(&refused);
    let rows = rows_of(&panel, "connections");
    assert_eq!(rows[0].kind, "auth");
    assert_eq!(rows[0].action, "authorize");
}

#[test]
fn the_selected_account_is_the_current_row() {
    let panel = resolve(&state());
    let selected: Vec<&PanelRow> = rows_of(&panel, "connections")
        .iter()
        .filter(|r| r.current)
        .collect();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].label, "work");
    assert!(selected[0].bold);
}

#[test]
fn exit_nodes_list_the_tailnet_then_recents_then_the_picker() {
    let panel = resolve_with(
        &state(),
        ResolveOptions {
            recent_regions: vec!["France\nParis".into()],
            ..ResolveOptions::default()
        },
    );
    let rows = rows_of(&panel, "exitNodes");
    assert_eq!(
        rows.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>(),
        ["exitNode", "exitNode", "mullvadPicker"]
    );
    assert_eq!(rows[0].label, "router");
    assert_eq!(rows[1].label, "Paris, France");
    assert_eq!(rows[2].action, "togglePicker");
}

#[test]
fn exit_nodes_hide_when_tailscale_is_down() {
    let down = PanelState {
        active: false,
        running: false,
        ..state()
    };
    assert!(!section(&resolve(&down), "exitNodes").visible);
}

#[test]
fn the_active_exit_node_is_current_and_carries_the_disconnect_hint() {
    let panel = resolve(&state());
    let rows = rows_of(&panel, "exitNodes");
    assert!(rows[0].current);
    assert_eq!(rows[0].hint, "Disconnect");

    let idle_node = Peer {
        exit_node: false,
        ..STATUS.exit_nodes[0].clone()
    };
    let idle = PanelState {
        own_exit_nodes: vec![idle_node],
        ..state()
    };
    assert_eq!(rows_of(&resolve(&idle), "exitNodes")[0].hint, "Connect");
}

#[test]
fn the_picker_keys_every_region_and_carries_the_message_for_an_empty_result() {
    let panel = resolve_with(
        &state(),
        ResolveOptions {
            mullvad_picker_open: true,
            ..ResolveOptions::default()
        },
    );
    let picker = picker_of(rows_of(&panel, "exitNodes"));
    let cities = |query: &str| -> Vec<String> {
        matching(&picker.children, "mullvad", query)
            .iter()
            .map(|c| c.label.clone())
            .collect()
    };
    assert_eq!(cities("par"), ["Paris"]);
    assert_eq!(cities("france"), ["Marseille", "Paris"]);
    assert!(cities("zzz").is_empty());

    let none = picker
        .children
        .iter()
        .find(|c| c.kind == "empty")
        .expect("empty row");
    assert_eq!(none.label, "No Mullvad regions found.");
    assert!(!none.navigable);
}

// ---- the machines section, and what finds a machine -----------------------

#[test]
fn machine_rows_carry_their_subtitle_icon_copy_options_and_actions() {
    let panel = resolve(&state());
    let rows = rows_of(&panel, "machines");
    let laptop = labelled(rows, "laptop");
    assert_eq!(laptop.sublabel, "100.64.0.2 \u{b7} Alice");
    assert_eq!(laptop.icon, "computer-symbolic");
    assert_eq!(
        laptop
            .copy_options
            .iter()
            .map(|o| o.kind.as_str())
            .collect::<Vec<_>>(),
        ["name", "dns", "ipv6", "ip"]
    );
    assert_eq!(
        laptop
            .actions
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["send", "copy"]
    );
}

#[test]
fn the_send_action_appears_only_for_a_taildrop_target() {
    let panel = resolve(&state());
    let rows = rows_of(&panel, "machines");
    for name in ["router", "phone"] {
        assert_eq!(
            labelled(rows, name)
                .actions
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["copy"],
            "{name}"
        );
    }

    let no_sharing = PanelState {
        file_sharing: false,
        ..state()
    };
    let panel = resolve(&no_sharing);
    assert!(
        rows_of(&panel, "machines")
            .iter()
            .all(|r| !r.actions.iter().any(|a| a.id == "send"))
    );
}

#[test]
fn offline_machines_are_listed_last_marked_and_cannot_be_sent_to() {
    let panel = resolve(&state());
    let rows = peer_rows(&panel);
    assert_eq!(
        rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
        ["laptop", "phone", "router", "offline-box"]
    );

    let offline = rows.last().expect("a last row");
    assert_eq!(offline.sublabel, "Offline \u{b7} 100.64.0.5 \u{b7} Alice");
    assert_eq!(
        offline
            .actions
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["copy"]
    );
    assert_eq!(
        offline
            .copy_options
            .iter()
            .map(|o| o.kind.as_str())
            .collect::<Vec<_>>(),
        ["name", "dns", "ip"]
    );
}

#[test]
fn the_machines_section_states_its_own_empty_case() {
    let none = PanelState {
        peers: Vec::new(),
        ..state()
    };
    let panel = resolve(&none);
    let machines = section(&panel, "machines");
    assert!(machines.visible);
    assert!(machines.rows.is_empty());
    assert_eq!(machines.empty, "No machines found on this tailnet.");
}

#[test]
fn a_machine_key_matches_every_field_its_row_shows_and_the_os() {
    let panel = resolve(&state());
    let rows = rows_of(&panel, "machines");
    let names = |query: &str| -> Vec<String> {
        matching(rows, "machines", query)
            .iter()
            .map(|r| r.payload["HostName"].as_str().unwrap_or("").to_string())
            .collect()
    };
    assert_eq!(names("").len(), STATUS.peers.len());
    assert_eq!(names("ANDROID"), ["phone"]);
    assert_eq!(names("100.64.0.5"), ["offline-box"]);

    let mut found = names("example.ts.net");
    found.sort();
    let mut expected: Vec<String> = STATUS.peers.iter().map(|p| p.host_name.clone()).collect();
    expected.sort();
    assert_eq!(found, expected);
}

/// Reported as "the search is case sensitive", which it never was: the panel's
/// key catcher was swallowing lowercase h, j, k, l and x before they reached
/// the field, so the same query typed in capitals arrived intact and the one
/// typed normally did not. The keys are lowercased here so a future change
/// cannot make the complaint true.
#[test]
fn both_searches_ignore_case_in_the_query_and_in_what_they_match() {
    let panel = resolve_with(
        &state(),
        ResolveOptions {
            mullvad_picker_open: true,
            ..ResolveOptions::default()
        },
    );
    let machines = rows_of(&panel, "machines");
    let names = |query: &str| -> Vec<String> {
        matching(machines, "machines", query)
            .iter()
            .map(|r| r.label.clone())
            .collect()
    };
    assert_eq!(names("android"), names("ANDROID"));
    assert_eq!(names("android"), names("AnDrOiD"));
    assert!(
        !names("android").is_empty(),
        "the fixture no longer carries an Android peer"
    );

    let children = &picker_of(rows_of(&panel, "exitNodes")).children;
    let cities = |query: &str| -> Vec<String> {
        matching(children, "mullvad", query)
            .iter()
            .map(|c| c.label.clone())
            .collect()
    };
    let any_city = MULLVAD_REGIONS[0].city.clone().expect("a city");
    assert_eq!(
        cities(&any_city.to_uppercase()),
        cities(&any_city.to_lowercase())
    );
    assert!(!cities(&any_city.to_uppercase()).is_empty());
}

#[test]
fn every_filtered_row_carries_a_lowercased_key_and_the_scope_that_filters_it() {
    let panel = resolve(&state());
    let laptop = labelled(rows_of(&panel, "machines"), "laptop");
    assert_eq!(laptop.search_scope, "machines");
    for needle in [
        "laptop",
        "laptop.example.ts.net",
        "100.64.0.2",
        "linux",
        "alice",
    ] {
        assert!(
            laptop.search_key.contains(needle),
            "the key does not carry {needle}"
        );
    }
    assert_eq!(laptop.search_key, laptop.search_key.to_lowercase());

    let open = resolve_with(
        &state(),
        ResolveOptions {
            mullvad_picker_open: true,
            ..ResolveOptions::default()
        },
    );
    let paris = labelled(&picker_of(rows_of(&open, "exitNodes")).children, "Paris");
    assert_eq!(paris.search_scope, "mullvad");
    assert_eq!(paris.search_key, "paris france");
}

#[test]
fn the_search_field_and_the_unfiltered_rows_carry_no_scope() {
    let long = PanelState {
        peers: MANY_PEERS.clone(),
        ..state()
    };
    let panel = resolve(&long);
    let field = rows_of(&panel, "machines")
        .iter()
        .find(|r| r.kind == "machineSearch")
        .expect("search row");
    assert_eq!(field.search_scope, "");

    let open = resolve_with(
        &state(),
        ResolveOptions {
            mullvad_picker_open: true,
            ..ResolveOptions::default()
        },
    );
    let picker = picker_of(rows_of(&open, "exitNodes"));
    assert_eq!(picker.search_scope, "");
    assert_eq!(picker.search_key, "");
}

#[test]
fn each_empty_result_row_is_the_keyless_row_of_its_scope() {
    let long = PanelState {
        peers: MANY_PEERS.clone(),
        ..state()
    };
    let panel = resolve(&long);
    let none = rows_of(&panel, "machines")
        .iter()
        .find(|r| r.kind == "empty")
        .expect("machines empty row");
    assert_eq!(none.search_scope, "machines");
    assert_eq!(none.search_key, "");

    let open = resolve_with(
        &state(),
        ResolveOptions {
            mullvad_picker_open: true,
            ..ResolveOptions::default()
        },
    );
    let message = picker_of(rows_of(&open, "exitNodes"))
        .children
        .iter()
        .find(|c| c.kind == "empty")
        .expect("picker empty row");
    assert_eq!(message.search_scope, "mullvad");
    assert_eq!(message.search_key, "");
}

#[test]
fn navigation_repeats_the_scope_and_key_of_the_row_it_points_at() {
    let long = PanelState {
        peers: MANY_PEERS.clone(),
        ..state()
    };
    let panel = resolve_with(
        &long,
        ResolveOptions {
            mullvad_picker_open: true,
            ..ResolveOptions::default()
        },
    );

    let mut by_id: Vec<&PanelRow> = Vec::new();
    for s in &panel.sections {
        for row in &s.rows {
            by_id.push(row);
            by_id.extend(row.children.iter());
        }
    }

    for entry in &panel.navigation {
        if entry.section_id == "header" {
            assert_eq!(entry.search_scope, "");
            continue;
        }
        let row = by_id
            .iter()
            .find(|r| r.id == entry.row_id)
            .unwrap_or_else(|| panic!("row {}", entry.row_id));
        assert_eq!(entry.search_scope, row.search_scope);
        assert_eq!(entry.search_key, row.search_key);
    }
    assert!(
        panel
            .navigation
            .iter()
            .any(|e| e.search_scope == "machines")
    );
    assert!(panel.navigation.iter().any(|e| e.search_scope == "mullvad"));
}

#[test]
fn the_machines_search_appears_only_for_a_list_long_enough_to_need_it() {
    let panel = resolve(&state());
    assert!(
        !rows_of(&panel, "machines")
            .iter()
            .any(|r| r.kind == "machineSearch")
    );

    let long = PanelState {
        peers: MANY_PEERS.clone(),
        ..state()
    };
    let panel = resolve(&long);
    let rows = rows_of(&panel, "machines");
    assert_eq!(rows[0].kind, "machineSearch");
    assert!(!rows[0].navigable);
    assert_eq!(rows[0].search_placeholder, "Search machines");
}

#[test]
fn the_machines_search_leaves_behind_whatever_its_query_matches() {
    let long = PanelState {
        peers: MANY_PEERS.clone(),
        ..state()
    };
    let panel = resolve(&long);
    let rows = rows_of(&panel, "machines");
    let labels = |query: &str| -> Vec<String> {
        matching(rows, "machines", query)
            .iter()
            .map(|r| r.label.clone())
            .collect()
    };
    assert_eq!(labels("100.64.1.7"), ["box-7"]);
    assert_eq!(labels("BOX-11"), ["box-11"]);
    assert_eq!(
        labels("windows"),
        MANY_PEERS
            .iter()
            .filter(|p| p.os == "windows")
            .map(|p| p.display_name.clone())
            .collect::<Vec<_>>()
    );
    assert!(labels("nowhere").is_empty());
}

#[test]
fn the_machines_search_matches_the_owner_it_shows() {
    let panel = resolve(&state());
    let rows = rows_of(&panel, "machines");
    let labels = |query: &str| -> Vec<String> {
        matching(rows, "machines", query)
            .iter()
            .map(|r| r.label.clone())
            .collect()
    };
    assert_eq!(labels("bob"), ["phone"]);
    assert_eq!(labels("alice"), ["laptop", "router", "offline-box"]);
}

/// The message is resolved with the rest of the section rather than when the
/// query empties it, because by then no frontend can ask the model for it.
#[test]
fn the_machines_section_carries_the_message_for_a_search_that_matches_nothing() {
    let long = PanelState {
        peers: MANY_PEERS.clone(),
        ..state()
    };
    let panel = resolve(&long);
    let rows = rows_of(&panel, "machines");
    assert_eq!(
        rows[..2]
            .iter()
            .map(|r| r.kind.as_str())
            .collect::<Vec<_>>(),
        ["machineSearch", "empty"]
    );
    assert_eq!(rows[1].label, "No machines match.");
    assert!(!rows[1].navigable);

    let none = PanelState {
        peers: Vec::new(),
        ..state()
    };
    let panel = resolve(&none);
    assert!(
        !rows_of(&panel, "machines")
            .iter()
            .any(|r| r.kind == "empty")
    );
}

#[test]
fn panel_has_row_reports_whether_a_row_is_drawn_children_included() {
    let long = PanelState {
        peers: MANY_PEERS.clone(),
        ..state()
    };
    let panel = resolve_with(
        &long,
        ResolveOptions {
            mullvad_picker_open: true,
            ..ResolveOptions::default()
        },
    );
    for id in ["machines:search", "mullvad:add", "mullvad:empty"] {
        assert!(core::panel::panel_has_row(&panel, id), "{id}");
    }
    assert!(!core::panel::panel_has_row(&panel, "nothing:here"));

    let short = resolve(&state());
    assert!(!core::panel::panel_has_row(&short, "machines:search"));
}

#[test]
fn neither_the_search_field_nor_its_empty_case_is_a_cursor_stop() {
    let long = PanelState {
        peers: MANY_PEERS.clone(),
        ..state()
    };
    let panel = resolve(&long);
    let stops: Vec<&str> = panel.navigation.iter().map(|e| e.row_id.as_str()).collect();
    assert!(!stops.contains(&"machines:search"));
    assert!(!stops.contains(&"machines:empty"));
}

// ---- the header, the status line, and the traversal -----------------------

#[test]
fn the_header_reflects_every_connection_state() {
    let on = resolve(&state()).header;
    assert_eq!(on.title, "workstation");
    assert!(on.toggle_checked);
    assert_eq!(on.toggle_hint, "Turn Tailscale off");
    assert!(!on.crossed);

    let off = resolve(&PanelState {
        active: false,
        running: false,
        ..state()
    })
    .header;
    assert_eq!(off.toggle_hint, "Turn Tailscale on");
    assert!(off.crossed);
    assert!(off.dimmed);

    let login = resolve(&PanelState {
        active: false,
        running: false,
        needs_login: true,
        ..state()
    })
    .header;
    assert_eq!(login.toggle_hint, "Authorize this device");
    assert!(login.warning);
    assert!(!login.crossed);

    let missing = resolve(&PanelState {
        providers: None,
        installed: false,
        ..state()
    })
    .header;
    assert_eq!(missing.title, "TailGauge");
    assert!(!missing.toggle_visible);
}

#[test]
fn the_hero_phrase_rotates_and_wraps_in_both_directions() {
    let phrase = |i: i64| {
        resolve_with(
            &state(),
            ResolveOptions {
                phrase_index: i,
                ..ResolveOptions::default()
            },
        )
        .header
        .meta
    };
    let first = phrase(0);
    assert_eq!(phrase(10), first, "and wraps forward");
    assert_ne!(phrase(1), first);
    assert_eq!(phrase(-1), phrase(9), "and backward");
    assert_eq!(phrase(-10), first);
}

#[test]
fn status_precedence_missing_cli_then_progress_then_error() {
    let missing = resolve(&PanelState {
        providers: None,
        installed: false,
        ..state()
    })
    .status;
    assert!(missing.text.contains("No supported VPN CLI"));

    let both = resolve(&PanelState {
        action_status: "Working".into(),
        last_error: "boom".into(),
        ..state()
    })
    .status;
    assert_eq!(both.text, "Working");
    assert_eq!(both.tone, "dim");

    let failed = resolve(&PanelState {
        last_error: "boom".into(),
        ..state()
    })
    .status;
    assert_eq!(failed.text, "boom");
    assert_eq!(failed.tone, "error");

    assert_eq!(resolve(&state()).status.text, "");
}

#[test]
fn navigation_visits_the_header_then_every_visible_row_in_draw_order() {
    let panel = resolve_with(
        &state(),
        ResolveOptions {
            recent_regions: vec!["France\nParis".into()],
            ..ResolveOptions::default()
        },
    );
    let ids: Vec<&str> = panel.navigation.iter().map(|n| n.row_id.as_str()).collect();
    assert_eq!(ids[0], "header");

    let mut expected = vec!["header"];
    for s in panel.sections.iter().filter(|s| s.visible) {
        for row in s.rows.iter().filter(|r| r.navigable) {
            expected.push(&row.id);
        }
    }
    assert_eq!(ids, expected);
}

#[test]
fn an_expanded_picker_puts_its_regions_in_the_traversal_a_closed_one_does_not() {
    let closed = resolve(&state());
    let closed_ids: Vec<&str> = closed
        .navigation
        .iter()
        .map(|n| n.row_id.as_str())
        .collect();
    assert!(!closed_ids.iter().any(|id| id.starts_with("region:")));

    let open = resolve_with(
        &state(),
        ResolveOptions {
            mullvad_picker_open: true,
            ..ResolveOptions::default()
        },
    );
    let open_ids: Vec<&str> = open.navigation.iter().map(|n| n.row_id.as_str()).collect();
    assert_eq!(
        open_ids
            .iter()
            .filter(|id| id.starts_with("region:"))
            .count(),
        MULLVAD_REGIONS.len()
    );
    let picker_at = open_ids
        .iter()
        .position(|id| *id == "mullvad:add")
        .expect("the picker");
    let first_region = open_ids
        .iter()
        .position(|id| id.starts_with("region:"))
        .expect("a region");
    assert_eq!(
        picker_at + 1,
        first_region,
        "the regions follow the row that opened them"
    );
}

#[test]
fn hidden_sections_contribute_no_cursor_stops() {
    let bare = PanelState {
        providers: None,
        installed: false,
        peers: Vec::new(),
        accounts: Vec::new(),
        own_exit_nodes: Vec::new(),
        mullvad_regions: Vec::new(),
        ..state()
    };
    let panel = resolve(&bare);
    assert_eq!(
        panel
            .navigation
            .iter()
            .map(|n| n.row_id.as_str())
            .collect::<Vec<_>>(),
        ["header"]
    );
}

#[test]
fn every_cursor_stop_resolves_back_to_its_row() {
    let panel = resolve_with(
        &state(),
        ResolveOptions {
            mullvad_picker_open: true,
            recent_regions: vec!["France\nParis".into()],
            ..ResolveOptions::default()
        },
    );
    assert!(
        core::panel::panel_row_at(&panel, 0).is_none(),
        "the header is not a row"
    );
    for i in 1..panel.navigation.len() {
        let row = core::panel::panel_row_at(&panel, i).unwrap_or_else(|| panic!("stop {i}"));
        assert_eq!(row.id, panel.navigation[i].row_id);
        assert_eq!(core::panel::panel_nav_index_of(&panel, &row.id), i);
    }
    assert!(core::panel::panel_row_at(&panel, panel.navigation.len()).is_none());
}
