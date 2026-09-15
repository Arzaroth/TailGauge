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

// ---- the banner, the footer, and what a row says it is waiting on ---------

#[test]
fn every_user_visible_string_arrives_finished() {
    let bare = PanelState {
        providers: None,
        installed: false,
        ..state()
    };
    let panel = resolve(&bare);
    assert_eq!(
        panel.status.text,
        "No supported VPN CLI on PATH. Looked for Tailscale, NetBird."
    );
    assert!(
        !panel.header.toggle_hint.is_empty(),
        "the toggle says what it would do"
    );
    for s in panel.sections.iter().filter(|s| !s.title.is_empty()) {
        assert!(
            s.title.starts_with(|c: char| c.is_uppercase()),
            "{} has no title a reader could use",
            s.id
        );
    }

    let none = PanelState {
        peers: Vec::new(),
        ..state()
    };
    assert_eq!(
        section(&resolve(&none), "machines").empty,
        "No machines found on this tailnet."
    );
}

#[test]
fn a_busy_row_reports_which_command_it_is_waiting_on() {
    let personal_id = ACCOUNTS
        .accounts
        .iter()
        .find(|a| a.selected != Some(true))
        .map(|a| a.id.clone())
        .expect("a second account");
    let switching = PanelState {
        switching_account_id: personal_id.clone(),
        ..state()
    };
    let panel = resolve(&switching);
    for r in rows_of(&panel, "connections")
        .iter()
        .filter(|r| r.kind == "account")
    {
        assert_eq!(r.busy, r.id == format!("account:{personal_id}"), "{}", r.id);
    }

    let setting = PanelState {
        setting_exit_node_id: STATUS.exit_nodes[0].id.clone(),
        ..state()
    };
    let panel = resolve(&setting);
    assert!(rows_of(&panel, "exitNodes")[0].busy);
}

#[test]
fn the_update_section_is_hidden_until_there_is_an_update() {
    let panel = resolve(&state());
    let update = section(&panel, "update");
    assert!(!update.visible);
    assert!(update.rows.is_empty());
    assert_eq!(update.title, "", "one banner needs no section header");
    assert!(!panel.navigation.iter().any(|n| n.row_id == "update"));
}

#[test]
fn an_available_update_offers_to_install_itself() {
    let pending = PanelState {
        update: Some(core::panel_state::UpdateInfo {
            available: true,
            latest: "1.1.0".into(),
            ..Default::default()
        }),
        ..state()
    };
    let panel = resolve(&pending);
    assert!(section(&panel, "update").visible);

    let row = &rows_of(&panel, "update")[0];
    assert_eq!(row.kind, "update");
    assert_eq!(row.label, "TailGauge 1.1.0 is available");
    assert_eq!(row.sublabel, "Install it now");
    assert_eq!(row.action, "update");
    assert_eq!(
        panel.navigation[1].row_id, "update",
        "the banner leads the traversal"
    );
}

#[test]
fn an_update_in_flight_marks_the_row_busy() {
    let applying = PanelState {
        update: Some(core::panel_state::UpdateInfo {
            available: true,
            latest: "1.1.0".into(),
            ..Default::default()
        }),
        updating: true,
        ..state()
    };
    assert!(rows_of(&resolve(&applying), "update")[0].busy);
}

#[test]
fn the_footer_carries_the_version_the_frontend_passed() {
    let versioned = PanelState {
        version: "1.2.3".into(),
        ..state()
    };
    assert_eq!(resolve(&versioned).footer, "TailGauge v1.2.3");
}

#[test]
fn a_frontend_that_knows_no_version_gets_no_footer() {
    assert_eq!(resolve(&state()).footer, "");
    assert_eq!(
        resolve(&PanelState {
            version: String::new(),
            ..state()
        })
        .footer,
        ""
    );
}

/// The binary is the one part that replaces itself, so it is the one that can
/// be ahead of the widget the user is looking at.
#[test]
fn a_binary_left_behind_by_a_half_applied_update_shows_next_to_the_widget() {
    let skewed = PanelState {
        version: "1.2.3".into(),
        update: Some(core::panel_state::UpdateInfo {
            current: "1.2.2".into(),
            ..Default::default()
        }),
        ..state()
    };
    assert_eq!(
        resolve(&skewed).footer,
        "TailGauge v1.2.3 \u{b7} binary v1.2.2"
    );
}

#[test]
fn a_binary_on_the_widget_version_is_not_worth_a_second_number() {
    let level = PanelState {
        version: "1.2.3".into(),
        update: Some(core::panel_state::UpdateInfo {
            current: "1.2.3".into(),
            ..Default::default()
        }),
        ..state()
    };
    assert_eq!(resolve(&level).footer, "TailGauge v1.2.3");

    let unknown = PanelState {
        version: "1.2.3".into(),
        update: Some(core::panel_state::UpdateInfo::default()),
        ..state()
    };
    assert_eq!(resolve(&unknown).footer, "TailGauge v1.2.3");
}

#[test]
fn the_send_action_disappears_when_the_binary_is_not_installed() {
    let panel = resolve(&state());
    assert!(
        peer_rows(&panel)
            .iter()
            .any(|r| r.actions.iter().any(|a| a.id == "send")),
        "the fixture should have at least one Taildrop target"
    );

    let store = PanelState {
        helpers: false,
        ..state()
    };
    let panel = resolve(&store);
    let rows = peer_rows(&panel);
    assert!(
        !rows
            .iter()
            .any(|r| r.actions.iter().any(|a| a.id == "send")),
        "a store-installed widget has no tailgauge binary to call"
    );
    assert!(
        rows.iter()
            .all(|r| r.actions.iter().any(|a| a.id == "copy")),
        "copying still works without it"
    );
}

#[test]
fn can_send_files_agrees_with_the_resolved_actions() {
    let state = state();
    let panel = resolve(&state);
    for row in peer_rows(&panel) {
        let peer: Peer = serde_json::from_value(row.payload.clone()).expect("a peer payload");
        assert_eq!(
            row.actions.iter().any(|a| a.id == "send"),
            core::panel::can_send_files(&state, &peer),
            "{} disagrees with its own row",
            row.id
        );
    }
}

#[test]
fn the_switch_stays_enabled_while_a_background_poll_runs() {
    // A status poll must not make the switch unclickable: a toggle already
    // reports optimistically, so there is nothing a second click can break.
    let busy = PanelState {
        busy: true,
        ..state()
    };
    let header = resolve(&busy).header;
    assert!(header.toggle_enabled);
    assert!(header.busy);
}

#[test]
fn the_switch_is_disabled_only_when_there_is_no_cli_to_drive() {
    let bare = PanelState {
        providers: None,
        installed: false,
        ..state()
    };
    let header = resolve(&bare).header;
    assert!(!header.toggle_visible);
    assert!(!header.toggle_enabled);
    assert!(header.actions.is_empty(), "and carries no controls either");
}

// ---- the provider registry ------------------------------------------------

#[test]
fn the_registry_names_every_provider_and_the_cli_that_proves_it() {
    let ids: Vec<&str> = PROVIDERS.iter().map(|p| p.id).collect();
    assert_eq!(ids, ["tailscale", "netbird"]);
    assert_eq!(
        core::providers::provider_cli_names(),
        ["tailscale", "netbird"]
    );
    assert_eq!(
        core::providers::provider_by_id("netbird")
            .expect("netbird")
            .label,
        "NetBird"
    );
    assert!(core::providers::provider_by_id("nope").is_none());
}

#[test]
fn a_frontend_that_has_not_been_taught_to_probe_still_reports_one_provider() {
    let old = PanelState {
        installed: true,
        ..PanelState::default()
    };
    assert_eq!(
        core::providers::active_provider(&old).expect("one").id,
        "tailscale"
    );
    assert!(core::providers::active_provider(&PanelState::default()).is_none());
}

#[test]
fn detection_scopes_the_panel_to_what_is_actually_installed() {
    let netbird = PanelState {
        providers: Some(detected(&["netbird"])),
        ..PanelState::default()
    };
    assert_eq!(
        core::providers::active_provider(&netbird)
            .expect("netbird")
            .id,
        "netbird"
    );

    let nothing = PanelState {
        providers: Some(detected(&[])),
        ..PanelState::default()
    };
    assert!(core::providers::active_provider(&nothing).is_none());

    let both = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        ..PanelState::default()
    };
    assert_eq!(
        core::providers::installed_providers(&both)
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        ["tailscale", "netbird"]
    );
}

#[test]
fn with_several_installed_the_registry_order_decides_until_a_choice_is_made() {
    let both = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        ..PanelState::default()
    };
    assert_eq!(
        core::providers::active_provider(&both).expect("one").id,
        "tailscale"
    );

    let chosen = PanelState {
        active_provider_id: "netbird".into(),
        ..both.clone()
    };
    assert_eq!(
        core::providers::active_provider(&chosen).expect("one").id,
        "netbird"
    );

    let mut reversed = both.clone();
    reversed.providers.as_mut().expect("providers").reverse();
    assert_eq!(
        core::providers::active_provider(&reversed).expect("one").id,
        "tailscale",
        "however the frontend enumerated them"
    );
}

#[test]
fn a_choice_naming_a_provider_that_is_gone_falls_back_instead_of_blanking() {
    let stranded = PanelState {
        providers: Some(detected(&["netbird"])),
        active_provider_id: "tailscale".into(),
        ..PanelState::default()
    };
    assert_eq!(
        core::providers::active_provider(&stranded).expect("one").id,
        "netbird"
    );
}

#[test]
fn capabilities_are_read_from_the_active_provider_not_its_name() {
    use core::providers::Capability;
    let ts = PanelState {
        providers: Some(detected(&["tailscale"])),
        ..PanelState::default()
    };
    let nb = PanelState {
        providers: Some(detected(&["netbird"])),
        ..PanelState::default()
    };
    assert!(core::providers::provider_supports(&ts, Capability::Mullvad));
    assert!(!core::providers::provider_supports(
        &nb,
        Capability::Mullvad
    ));
    assert!(core::providers::provider_supports(
        &nb,
        Capability::Networks
    ));

    let nothing = PanelState {
        providers: Some(detected(&[])),
        ..PanelState::default()
    };
    assert!(!core::providers::provider_supports(
        &nothing,
        Capability::ExitNodes
    ));
}

#[test]
fn a_provider_without_a_feature_never_shows_the_section_that_needs_it() {
    let nb = PanelState {
        providers: Some(detected(&["netbird"])),
        active_provider_id: "netbird".into(),
        ..state()
    };
    let panel = resolve(&nb);
    assert!(
        !section(&panel, "exitNodes").visible,
        "NetBird has no exit nodes, even with tailnet exit nodes in the snapshot"
    );
    assert!(
        !section(&panel, "connections").visible,
        "NetBird has no account switching"
    );

    let panel = resolve(&state());
    assert!(section(&panel, "exitNodes").visible);
    assert!(section(&panel, "connections").visible);
}

#[test]
fn every_provider_the_registry_calls_drivable_has_what_driving_needs() {
    for provider in PROVIDERS.iter().filter(|p| p.supported) {
        assert!(
            !provider.status().is_empty(),
            "{} has no status command",
            provider.id
        );
        assert!(
            !provider.up().is_empty(),
            "{} has no up command",
            provider.id
        );
        assert!(
            !provider.down().is_empty(),
            "{} has no down command",
            provider.id
        );
        assert_eq!(
            provider.status()[0],
            provider.cli,
            "{} polls a binary other than the one detection probes",
            provider.id
        );
        if provider.capabilities.exit_nodes {
            assert!(provider.exit_node_list().is_some(), "{}", provider.id);
        }
        if provider.capabilities.accounts {
            assert!(provider.accounts().is_some(), "{}", provider.id);
        }
        if provider.capabilities.networks {
            assert!(provider.networks().is_some(), "{}", provider.id);
            assert!(
                provider.select_network("x", true).is_some(),
                "{}",
                provider.id
            );
        }
    }
}

#[test]
fn nothing_installed_is_never_ready_whatever_was_chosen() {
    let nothing = PanelState {
        providers: Some(detected(&[])),
        ..PanelState::default()
    };
    assert!(!core::providers::provider_ready(&nothing));

    let chosen = PanelState {
        active_provider_id: "tailscale".into(),
        ..nothing
    };
    assert!(!core::providers::provider_ready(&chosen));
    assert!(!core::providers::provider_ready(&PanelState::default()));

    let bare = PanelState {
        providers: Some(detected(&[])),
        installed: false,
        ..state()
    };
    let panel = resolve(&bare);
    assert!(!panel.header.toggle_enabled);
    assert!(panel.status.text.contains("No supported VPN CLI"));
}

#[test]
fn with_both_installed_the_drivable_one_wins_the_auto_choice() {
    let both = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        ..state()
    };
    assert_eq!(
        core::providers::active_provider(&both).expect("one").id,
        "tailscale"
    );
    assert!(core::providers::provider_ready(&both));
    assert_eq!(resolve(&both).header.title, STATUS.self_name);
}

#[test]
fn the_probe_reads_which_stdout_not_its_exit_code() {
    let probe = |out: &str| -> Vec<(String, bool)> {
        core::providers::parse_provider_probe(out)
            .into_iter()
            .map(|p| (p.id, p.installed))
            .collect()
    };
    let named =
        |ts: bool, nb: bool| vec![("tailscale".to_string(), ts), ("netbird".to_string(), nb)];

    assert_eq!(
        probe("/usr/bin/tailscale\n/usr/bin/netbird\n"),
        named(true, true)
    );
    assert_eq!(probe("/usr/bin/netbird\n"), named(false, true));
    assert_eq!(probe(""), named(false, false));
    assert_eq!(
        probe("which: no netbird in (/usr/bin)\n/usr/bin/tailscale\n"),
        named(true, false)
    );
    assert_eq!(probe("/home/me/.local/bin/tailscale\n"), named(true, false));
}

#[test]
fn sending_files_is_a_provider_capability_not_just_a_helper_check() {
    let ts = state();
    let peer = STATUS
        .peers
        .iter()
        .find(|p| core::panel::can_send_files(&ts, p))
        .expect("a Taildrop target");

    let nb = PanelState {
        providers: Some(detected(&["netbird"])),
        active_provider_id: "netbird".into(),
        ..state()
    };
    assert!(!core::panel::can_send_files(&nb, peer));
}

#[test]
fn the_panel_names_the_provider_it_is_driving() {
    let nb = PanelState {
        providers: Some(detected(&["netbird"])),
        active_provider_id: "netbird".into(),
        active: false,
        ..state()
    };
    assert_eq!(resolve(&nb).header.meta, "NetBird is disconnected");
    assert_eq!(core::panel::toggle_hint(&nb), "Turn NetBird on");
    assert_eq!(
        core::providers::provider_label(&PanelState::default()),
        "TailGauge"
    );
}

#[test]
fn the_argv_comes_from_the_registry_not_the_caller() {
    let tailscale = core::providers::provider_by_id("tailscale").expect("tailscale");
    let netbird = core::providers::provider_by_id("netbird").expect("netbird");
    assert_eq!(tailscale.status(), ["tailscale", "status", "--json"]);
    assert_eq!(netbird.status(), ["netbird", "status", "--json"]);
    assert_eq!(
        tailscale.switch_account("abc").expect("switch"),
        ["tailscale", "switch", "abc"]
    );
    assert_eq!(
        netbird.select_network("office", true).expect("select"),
        ["netbird", "networks", "select", "office"]
    );
    assert!(netbird.switch_account("abc").is_none());
}

#[test]
fn cycling_lands_on_the_next_drivable_provider_and_wraps() {
    let both = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        ..PanelState::default()
    };
    assert_eq!(
        core::providers::next_provider(&both).expect("next").id,
        "netbird"
    );

    let on_netbird = PanelState {
        active_provider_id: "netbird".into(),
        ..both
    };
    assert_eq!(
        core::providers::next_provider(&on_netbird)
            .expect("next")
            .id,
        "tailscale"
    );

    let alone = PanelState {
        providers: Some(detected(&["tailscale"])),
        ..PanelState::default()
    };
    assert!(core::providers::next_provider(&alone).is_none());
}

#[test]
fn the_toggle_hint_names_the_provider_it_will_act_on() {
    assert_eq!(core::panel::toggle_hint(&state()), "Turn Tailscale off");

    let off = PanelState {
        active: false,
        ..state()
    };
    assert_eq!(core::panel::toggle_hint(&off), "Turn Tailscale on");

    let login = PanelState {
        active: false,
        needs_login: true,
        ..state()
    };
    assert_eq!(core::panel::toggle_hint(&login), "Authorize this device");
}

// ---- NetBird --------------------------------------------------------------

static NB_STATUS: LazyLock<StatusOk> =
    LazyLock::new(
        || match core::parse_netbird_status(&fixture("netbird-status.json")) {
            core::StatusResult::Ok(status) => *status,
            other => panic!("the fixture should parse as a connected mesh, got {other:?}"),
        },
    );

static NB_IDLE: LazyLock<StatusOk> =
    LazyLock::new(
        || match core::parse_netbird_status(&fixture("netbird-status-idle.json")) {
            core::StatusResult::Ok(status) => *status,
            other => panic!("the idle capture should still parse, got {other:?}"),
        },
    );

static NB_NETWORKS: LazyLock<core::NetworksResult> =
    LazyLock::new(|| core::parse_netbird_networks(&fixture("netbird-networks.txt")));

#[test]
fn parse_netbird_status_reads_a_connected_mesh() {
    assert!(NB_STATUS.running);
    assert!(!NB_STATUS.needs_login);
    assert_eq!(NB_STATUS.daemon_state, "Connected");
    assert_eq!(NB_STATUS.self_name, "workstation");
    assert_eq!(
        NB_STATUS.self_ip, "100.85.0.1",
        "the CIDR suffix is not part of the address"
    );
    assert_eq!(NB_STATUS.self_dns_name, "workstation.netbird.selfhosted");
}

#[test]
fn parse_netbird_status_orders_peers_online_first_like_the_tailscale_one() {
    assert_eq!(
        NB_STATUS
            .peers
            .iter()
            .map(|p| p.host_name.as_str())
            .collect::<Vec<_>>(),
        ["laptop", "nas", "offline-box"]
    );
    let nas = NB_STATUS
        .peers
        .iter()
        .find(|p| p.host_name == "nas")
        .expect("nas");
    assert_eq!(nas.ipv4, ["100.85.0.2"]);
    assert!(nas.online);
    assert_eq!(nas.connection_type.as_deref(), Some("P2P"));
    assert_eq!(
        nas.latency_ms,
        Some(1.5),
        "nanoseconds are reported as milliseconds"
    );

    let off = NB_STATUS
        .peers
        .iter()
        .find(|p| p.host_name == "offline-box")
        .expect("offline");
    assert!(!off.online, "only \"Connected\" is reachable now");
    assert_eq!(off.latency_ms, Some(-1.0), "unmeasured is -1, not 0");
}

#[test]
fn parse_netbird_status_offers_nothing_it_cannot_do() {
    assert!(NB_STATUS.exit_nodes.is_empty(), "NetBird has no exit nodes");
    assert!(!NB_STATUS.file_sharing, "nor Taildrop");
    assert_eq!(
        NB_STATUS.auth_url, "",
        "the login URL arrives on the `up` stream"
    );
    assert!(
        NB_STATUS
            .peers
            .iter()
            .all(|p| !p.mullvad && !p.exit_node_option)
    );
}

#[test]
fn parse_netbird_status_reads_the_real_idle_capture() {
    assert!(!NB_IDLE.running);
    assert_eq!(NB_IDLE.daemon_state, "Idle");
    assert!(
        NB_IDLE.peers.is_empty(),
        "a null details list is no peers, not a crash"
    );
    assert_eq!(NB_IDLE.self_ip, "");
}

#[test]
fn parse_netbird_status_grades_every_login_shaped_daemon_state() {
    let graded = |daemon: &str| -> StatusOk {
        let raw = format!(r#"{{"daemonStatus":"{daemon}","peers":{{"details":null}}}}"#);
        match core::parse_netbird_status(&raw) {
            core::StatusResult::Ok(status) => *status,
            other => panic!("{daemon}: {other:?}"),
        }
    };
    for daemon in ["NeedsLogin", "SessionExpired", "LoginFailed"] {
        assert!(graded(daemon).needs_login, "{daemon} needs a login");
    }
    assert!(!graded("Connecting").needs_login);
    assert!(!graded("Connecting").running);
    assert!(graded("Connected").running);
}

#[test]
fn parse_netbird_status_survives_empty_and_malformed_input() {
    assert!(matches!(
        core::parse_netbird_status(""),
        core::StatusResult::Unavailable(_)
    ));
    assert!(matches!(
        core::parse_netbird_status("{not json"),
        core::StatusResult::Error(_)
    ));
    assert!(matches!(
        core::parse_netbird_status("[]"),
        core::StatusResult::Error(_)
    ));
}

#[test]
fn parse_netbird_networks_reads_the_block_format() {
    assert!(NB_NETWORKS.ok);
    assert_eq!(
        NB_NETWORKS
            .networks
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        ["prod-vpc", "office-lan", "dns-only"]
    );
    let named = |id: &str| NB_NETWORKS.networks.iter().find(|n| n.id == id).expect(id);

    let prod = named("prod-vpc");
    assert_eq!(prod.range, "10.10.0.0/16");
    assert!(prod.selected);
    assert!(prod.domains.is_empty());

    let office = named("office-lan");
    assert!(!office.selected);
    assert_eq!(office.domains, ["office.internal", "printers.internal"]);

    assert_eq!(
        named("dns-only").range,
        "",
        "a \"-\" range is absent, not a dash to show"
    );
}

#[test]
fn parse_netbird_networks_tells_empty_apart_from_broken() {
    for quiet in ["", "No networks available."] {
        let parsed = core::parse_netbird_networks(quiet);
        assert!(parsed.ok && parsed.networks.is_empty() && parsed.message.is_empty());
    }
    let failed = core::parse_netbird_networks("Error: failed to list network: not connected");
    assert!(!failed.ok);
    assert!(failed.message.contains("not connected"));
}

fn netbird_state() -> PanelState {
    PanelState {
        providers: Some(detected(&["netbird"])),
        active_provider_id: "netbird".into(),
        active: true,
        running: true,
        networks: NB_NETWORKS.networks.clone(),
        ..PanelState::default()
    }
}

#[test]
fn the_networks_section_belongs_to_the_provider_that_has_networks() {
    let panel = resolve(&netbird_state());
    let networks = section(&panel, "networks");
    assert!(networks.visible);
    assert_eq!(
        networks
            .rows
            .iter()
            .map(|r| r.label.as_str())
            .collect::<Vec<_>>(),
        ["prod-vpc", "office-lan", "dns-only"]
    );
    assert!(networks.rows[0].current, "the selected one is marked");
    assert_eq!(networks.rows[0].sublabel, "10.10.0.0/16");
    assert!(networks.rows.iter().all(|r| r.action == "selectNetwork"));
    assert_eq!(
        labelled(&networks.rows, "dns-only").sublabel,
        "apps.internal"
    );

    // Tailscale has no networks, whatever is in the snapshot.
    let ts = PanelState {
        networks: NB_NETWORKS.networks.clone(),
        ..state()
    };
    assert!(!section(&resolve(&ts), "networks").visible);
}

#[test]
fn a_network_being_joined_reports_busy_on_its_own_row() {
    let joining = PanelState {
        selecting_network_id: "office-lan".into(),
        ..netbird_state()
    };
    let panel = resolve(&joining);
    for row in rows_of(&panel, "networks") {
        assert_eq!(row.busy, row.label == "office-lan", "{}", row.label);
    }
}

#[test]
fn the_provider_switcher_appears_only_when_there_is_a_choice() {
    assert!(
        !section(&resolve(&state()), "providers").visible,
        "one provider is not a choice"
    );

    let both = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        ..state()
    };
    let panel = resolve(&both);
    let switcher = section(&panel, "providers");
    assert!(switcher.visible);
    assert_eq!(
        switcher
            .rows
            .iter()
            .map(|r| r.label.as_str())
            .collect::<Vec<_>>(),
        ["Tailscale", "NetBird"]
    );
    assert!(switcher.rows[0].current);
    assert!(switcher.rows.iter().all(|r| r.action == "switchProvider"));
}

#[test]
fn the_switcher_follows_the_choice_and_the_whole_panel_with_it() {
    let on_netbird = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        active_provider_id: "netbird".into(),
        ..state()
    };
    let panel = resolve(&on_netbird);
    let rows = rows_of(&panel, "providers");
    assert!(labelled(rows, "NetBird").current);
    assert!(!labelled(rows, "Tailscale").current);
    assert_eq!(panel.header.provider_id, "netbird");
    assert!(
        !section(&panel, "exitNodes").visible,
        "and the sections follow"
    );
}

// ---- the bar --------------------------------------------------------------

#[test]
fn summarize_provider_reads_a_status_result_or_survives_not_having_one() {
    let summary =
        core::bar::summarize_provider("tailscale", &core::parse_status(&fixture("status.json")));
    assert_eq!(summary.id, "tailscale");
    assert_eq!(summary.label, "Tailscale");
    assert!(summary.running);
    assert_eq!(summary.self_name, "workstation");
    assert_eq!(summary.state, "Running");

    let broken = core::bar::summarize_provider("tailscale", &core::parse_status("nonsense"));
    assert!(!broken.running && broken.state.is_empty());
}

#[test]
fn the_bar_icon_describes_the_machine_not_the_panel_view() {
    let up = core::panel_state::ProviderSummary {
        id: "netbird".into(),
        label: "NetBird".into(),
        running: true,
        ..Default::default()
    };
    let down = core::panel_state::ProviderSummary {
        id: "tailscale".into(),
        label: "Tailscale".into(),
        ..Default::default()
    };
    let mixed = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        summaries: Some(vec![down, up]),
        ..PanelState::default()
    };
    let bar = core::bar::bar_state(&mixed);
    assert!(bar.connected, "one provider up is a connected machine");
    assert!(!bar.crossed);
}

#[test]
fn the_bar_goes_dark_only_when_every_provider_is_down() {
    let all_down: Vec<core::panel_state::ProviderSummary> = PROVIDERS
        .iter()
        .map(|p| core::panel_state::ProviderSummary {
            id: p.id.into(),
            label: p.label.into(),
            ..Default::default()
        })
        .collect();
    let dark = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        summaries: Some(all_down),
        ..PanelState::default()
    };
    let bar = core::bar::bar_state(&dark);
    assert!(!bar.connected && !bar.warning && bar.crossed);
}

#[test]
fn the_tooltip_says_what_each_installed_provider_is_doing_active_first() {
    let state = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        active_provider_id: "netbird".into(),
        summaries: Some(vec![
            core::panel_state::ProviderSummary {
                id: "tailscale".into(),
                label: "Tailscale".into(),
                running: true,
                self_name: "workstation".into(),
                self_ip: "100.64.0.1".into(),
                state: "Running".into(),
                needs_login: false,
            },
            core::panel_state::ProviderSummary {
                id: "netbird".into(),
                label: "NetBird".into(),
                needs_login: true,
                state: "NeedsLogin".into(),
                ..Default::default()
            },
        ]),
        ..PanelState::default()
    };
    let tooltip = core::bar::bar_tooltip(&state);
    assert_eq!(tooltip.len(), 2);
    assert!(
        tooltip[0].starts_with("NetBird"),
        "the provider being shown leads"
    );
    assert!(tooltip[0].contains("needs login"));
    assert!(tooltip[1].contains("connected") && tooltip[1].contains("100.64.0.1"));

    // Every line the same width, because the bar centres each one on its own.
    let widths: Vec<usize> = tooltip.iter().map(|l| l.chars().count()).collect();
    assert!(widths.windows(2).all(|w| w[0] == w[1]));
}

#[test]
fn the_tooltip_falls_back_rather_than_lying_about_what_it_knows() {
    let nothing = core::bar::bar_tooltip(&PanelState::default());
    assert_eq!(
        nothing,
        ["No supported VPN CLI on PATH. Looked for Tailscale, NetBird."]
    );

    let unreported = PanelState {
        providers: Some(detected(&["tailscale"])),
        ..PanelState::default()
    };
    assert!(core::bar::bar_tooltip(&unreported)[0].contains("checking\u{2026}"));
}

#[test]
fn with_no_summaries_the_bar_still_follows_the_one_provider_we_know_about() {
    let up = PanelState {
        providers: Some(detected(&["tailscale"])),
        active: true,
        ..PanelState::default()
    };
    assert!(core::bar::bar_state(&up).connected);

    let login = PanelState {
        providers: Some(detected(&["tailscale"])),
        needs_login: true,
        ..PanelState::default()
    };
    let bar = core::bar::bar_state(&login);
    assert!(bar.warning && !bar.connected && !bar.crossed);
}

// ---- what the expanded row shows ------------------------------------------

fn from_status(raw: serde_json::Value) -> Peer {
    core::status::peer_from_status("x", &raw, &std::collections::BTreeMap::new())
}

#[test]
fn both_providers_report_what_the_expanded_row_shows() {
    let nas = NB_STATUS
        .peers
        .iter()
        .find(|p| p.host_name == "nas")
        .expect("nas");
    assert_eq!(nas.connection_type.as_deref(), Some("P2P"));
    assert_eq!(nas.rx_bytes, Some(4096));
    assert_eq!(nas.tx_bytes, Some(2048));

    let direct =
        from_status(serde_json::json!({"CurAddr": "1.2.3.4:41641", "RxBytes": 10, "TxBytes": 20}));
    assert_eq!(direct.connection_type.as_deref(), Some("P2P"));
    assert_eq!(direct.endpoint.as_deref(), Some("1.2.3.4:41641"));

    let relayed = from_status(serde_json::json!({"CurAddr": "", "Relay": "ams"}));
    assert_eq!(relayed.connection_type.as_deref(), Some("Relayed"));
    assert_eq!(relayed.relay.as_deref(), Some("ams"));

    let unknown = from_status(serde_json::json!({"CurAddr": "", "Relay": ""}));
    assert_eq!(
        unknown.connection_type.as_deref(),
        Some(""),
        "no reading is not a guess"
    );
}

fn details_of(peer: &Peer) -> std::collections::BTreeMap<String, String> {
    core::panel::peer_detail_rows(peer, NOW)
        .into_iter()
        .map(|r| (r.id, r.sublabel))
        .collect()
}

#[test]
fn peer_detail_rows_shows_only_what_was_actually_reported() {
    let peer = Peer {
        connection_type: Some("P2P".into()),
        latency_ms: Some(20.0),
        endpoint: Some("[2001:db8::1]:51820".into()),
        rx_bytes: Some(424),
        tx_bytes: Some(472),
        last_handshake: Some("2026-09-14T05:59:00Z".into()),
        routes: Some(vec!["192.168.42.0/24".into()]),
        ..Peer::default()
    };
    let rows = core::panel::peer_detail_rows(&peer, NOW);
    assert!(rows.iter().all(|r| r.kind == "detail" && !r.navigable));

    let by = details_of(&peer);
    assert_eq!(by["detail:connection"], "Direct peer-to-peer \u{b7} 20 ms");
    assert_eq!(by["detail:endpoint"], "[2001:db8::1]:51820");
    assert_eq!(by["detail:handshake"], "1 minute ago");
    assert_eq!(by["detail:transfer"], "\u{2193} 424 B   \u{2191} 472 B");
    assert_eq!(by["detail:routes"], "192.168.42.0/24");

    assert!(core::panel::peer_detail_rows(&Peer::default(), NOW).is_empty());
}

#[test]
fn a_relayed_peer_names_its_relay() {
    let summary = |kind: &str, relay: &str| {
        core::panel::connection_summary(&Peer {
            connection_type: Some(kind.into()),
            relay: Some(relay.into()),
            ..Peer::default()
        })
    };
    assert_eq!(summary("Relayed", "ams"), "Relayed via ams");
    assert_eq!(summary("", "ams"), "Relayed via ams");
    assert_eq!(summary("P2P", ""), "Direct peer-to-peer");
    assert_eq!(core::panel::connection_summary(&Peer::default()), "");
}

#[test]
fn bytes_and_elapsed_time_read_like_a_human_wrote_them() {
    assert_eq!(core::fmt::format_bytes(0), "0 B");
    assert_eq!(core::fmt::format_bytes(424), "424 B");
    assert_eq!(core::fmt::format_bytes(1536), "1.5 KB");
    assert_eq!(core::fmt::format_bytes(15 * 1024 * 1024), "15 MB");
    assert_eq!(
        core::fmt::format_since("2026-09-14T05:59:30Z", NOW),
        "just now"
    );
    assert_eq!(
        core::fmt::format_since("2026-09-14T05:59:00Z", NOW),
        "1 minute ago"
    );
    assert_eq!(
        core::fmt::format_since("2026-09-14T03:00:00Z", NOW),
        "3 hours ago"
    );
    assert_eq!(core::fmt::format_since("", NOW), "");
    assert_eq!(core::fmt::format_since("not a date", NOW), "");
}

#[test]
fn netbird_writes_never_as_a_zero_date_not_an_absent_field() {
    let raw = serde_json::json!({
        "daemonStatus": "Connected",
        "peers": {"details": [
            {"fqdn": "a.example", "status": "Connected",
             "lastWireguardHandshake": "0001-01-01T00:00:00Z"},
            {"fqdn": "b.example", "status": "Connected",
             "lastWireguardHandshake": "2026-09-14T05:59:00Z"},
        ]},
    })
    .to_string();
    let parsed = match core::parse_netbird_status(&raw) {
        core::StatusResult::Ok(status) => *status,
        other => panic!("{other:?}"),
    };
    let named = |name: &str| {
        parsed
            .peers
            .iter()
            .find(|p| p.host_name == name)
            .expect(name)
    };
    assert_eq!(
        named("a").last_handshake.as_deref(),
        Some(""),
        "a zero date is no handshake, not 2001 years ago"
    );
    assert_ne!(named("b").last_handshake.as_deref(), Some(""));
}

#[test]
fn a_machine_row_carries_its_detail_behind_a_disclosure_arrow() {
    let nb = PanelState {
        peers: NB_STATUS.peers.clone(),
        ..netbird_state()
    };
    let panel = resolve(&nb);
    let nas = labelled(rows_of(&panel, "machines"), "nas");
    assert!(
        !nas.children.is_empty(),
        "the detail is carried, not fetched on expand"
    );
    assert!(!nas.expanded);
    assert_eq!(
        nas.actions
            .iter()
            .find(|a| a.id == "detail")
            .expect("detail action")
            .label,
        "Show details"
    );

    let open = resolve_with(
        &nb,
        ResolveOptions {
            expanded_peer_id: nas.payload["id"].as_str().unwrap_or("").into(),
            ..ResolveOptions::default()
        },
    );
    let rows = rows_of(&open, "machines");
    let open_nas = labelled(rows, "nas");
    assert!(open_nas.expanded);
    assert_eq!(
        open_nas
            .actions
            .iter()
            .find(|a| a.id == "detail")
            .expect("detail action")
            .label,
        "Hide details"
    );
    assert!(
        !labelled(rows, "laptop").expanded,
        "and only the one that was asked for"
    );
}

#[test]
fn a_zero_date_is_never_from_either_provider() {
    let idle = from_status(serde_json::json!({
        "LastHandshake": "0001-01-01T00:00:00Z",
        "LastSeen": "2026-09-14T05:00:00Z",
        "Relay": "par",
    }));
    assert_eq!(idle.last_handshake.as_deref(), Some(""));
    assert_eq!(idle.last_seen.as_deref(), Some("2026-09-14T05:00:00Z"));

    let by = details_of(&idle);
    assert!(!by.contains_key("detail:handshake"), "no handshake, no row");
    assert_eq!(
        by["detail:seen"], "1 hour ago",
        "last seen stands in for it"
    );
    assert_eq!(by["detail:connection"], "Relayed via par");
}

#[test]
fn a_live_handshake_wins_over_last_seen() {
    let live = from_status(serde_json::json!({
        "CurAddr": "1.2.3.4:41641",
        "LastHandshake": "2026-09-14T05:59:00Z",
        "LastSeen": "2026-09-14T05:00:00Z",
    }));
    let by = details_of(&live);
    assert_eq!(by["detail:handshake"], "1 minute ago");
    assert!(!by.contains_key("detail:seen"), "not both");
}

#[test]
fn an_idle_peer_still_opens_onto_something() {
    // Nothing about a session it does not have, so the one thing always known
    // about it keeps the row from opening onto almost nothing.
    let idle = from_status(serde_json::json!({"Created": "2026-01-01T00:00:00Z"}));
    let by = details_of(&idle);
    assert!(by.contains_key("detail:added"), "{by:?}");
}

#[test]
fn the_hero_carries_the_active_provider_mark() {
    let ts = resolve(&state()).header;
    let nb = resolve(&netbird_state()).header;
    assert_ne!(ts.glyph, nb.glyph, "each provider is drawn as itself");
    assert_eq!(
        ts.glyph,
        core::providers::provider_by_id("tailscale")
            .expect("ts")
            .glyph
    );
    assert_eq!(
        nb.icon,
        core::providers::provider_by_id("netbird").expect("nb").icon
    );

    let none = resolve(&PanelState {
        providers: None,
        installed: false,
        ..state()
    })
    .header;
    assert!(
        !none.glyph.is_empty(),
        "and something is drawn even with no provider"
    );
}

#[test]
fn every_provider_brings_its_own_mark() {
    for provider in PROVIDERS {
        assert!(!provider.icon.is_empty(), "{} has no icon", provider.id);
        assert!(!provider.glyph.is_empty(), "{} has no glyph", provider.id);
    }
    let glyphs: Vec<&str> = PROVIDERS.iter().map(|p| p.glyph).collect();
    let mut unique = glyphs.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        glyphs.len(),
        "two providers drawn the same is one provider"
    );
}

#[test]
fn a_public_key_is_a_copy_option_from_either_provider() {
    let keyed = Peer {
        display_name: "box".into(),
        public_key: Some("nodekey:abc".into()),
        ..Peer::default()
    };
    let kinds: Vec<String> = core::panel::peer_copy_options(&keyed)
        .into_iter()
        .map(|o| o.kind)
        .collect();
    assert!(kinds.contains(&"key".to_string()));

    let bare = Peer {
        display_name: "box".into(),
        ..Peer::default()
    };
    let kinds: Vec<String> = core::panel::peer_copy_options(&bare)
        .into_iter()
        .map(|o| o.kind)
        .collect();
    assert!(
        !kinds.contains(&"key".to_string()),
        "nothing to copy is not an option"
    );
}

#[test]
fn the_header_carries_its_controls_so_every_desktop_draws_the_same_ones() {
    let header = resolve(&state()).header;
    assert_eq!(
        header
            .actions
            .iter()
            .map(|a| a.id.as_str())
            .collect::<Vec<_>>(),
        ["refresh"]
    );
    assert_eq!(header.actions[0].label, "Refresh");
    assert!(!header.actions[0].icon.is_empty());
    assert!(!header.actions[0].glyph.is_empty());
}

#[test]
fn every_row_is_fully_formed_so_no_frontend_has_to_fill_a_gap() {
    // Serialized, because that is the shape a frontend receives: a field the
    // resolver left off is a field the far side has to invent.
    let panel = resolve_with(
        &state(),
        ResolveOptions {
            mullvad_picker_open: true,
            recent_regions: vec!["France\nParis".into()],
            ..ResolveOptions::default()
        },
    );
    const KEYS: [&str; 20] = [
        "id",
        "kind",
        "label",
        "sublabel",
        "icon",
        "glyph",
        "action",
        "current",
        "busy",
        "bold",
        "navigable",
        "hint",
        "actions",
        "copyOptions",
        "children",
        "expanded",
        "searchPlaceholder",
        "searchScope",
        "searchKey",
        "payload",
    ];
    fn visit(row: &serde_json::Value) {
        let id = row["id"].as_str().unwrap_or("?");
        for key in KEYS {
            assert!(row.get(key).is_some(), "{id} is missing {key}");
        }
        assert!(row["label"].is_string());
        for key in ["actions", "copyOptions", "children"] {
            assert!(row[key].is_array(), "{id}'s {key} is not a list");
        }
        for child in row["children"].as_array().expect("children") {
            visit(child);
        }
    }
    let drawn = serde_json::to_value(&panel).expect("a panel");
    for section in drawn["sections"].as_array().expect("sections") {
        for row in section["rows"].as_array().expect("rows") {
            visit(row);
        }
    }
}

#[test]
fn login_plan_turns_on_whatever_provider_it_was_handed() {
    let up: Vec<String> = vec!["netbird".into(), "up".into()];
    let plan = core::providers::login_plan(false, "", &up);
    assert_eq!(plan.auth_url, "");
    assert_eq!(plan.command, up);

    // A pending authorization is a URL to open, whoever the provider is.
    let pending = core::providers::login_plan(true, "https://login.example/x", &up);
    assert_eq!(pending.auth_url, "https://login.example/x");
    assert!(pending.command.is_empty());

    // No provider, no command to run.
    let nothing = core::providers::login_plan(false, "", &[]);
    assert_eq!(nothing.auth_url, "");
    assert!(nothing.command.is_empty());
}

#[test]
fn the_switcher_never_offers_a_provider_it_cannot_drive() {
    let both = PanelState {
        providers: Some(detected(&["tailscale", "netbird"])),
        ..PanelState::default()
    };
    assert_eq!(
        core::providers::drivable_providers(&both)
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        PROVIDERS
            .iter()
            .filter(|p| p.supported)
            .map(|p| p.id)
            .collect::<Vec<_>>()
    );

    let nothing = PanelState {
        providers: Some(detected(&[])),
        ..PanelState::default()
    };
    assert!(core::providers::drivable_providers(&nothing).is_empty());
}

#[test]
fn the_hero_says_which_provider_to_draw_not_just_what_to_print() {
    assert_eq!(resolve(&state()).header.provider_id, "tailscale");
    assert_eq!(resolve(&netbird_state()).header.provider_id, "netbird");

    let bare = PanelState {
        providers: Some(detected(&[])),
        installed: false,
        ..state()
    };
    assert_eq!(resolve(&bare).header.provider_id, "");
}

#[test]
fn the_version_substitutes_into_its_template() {
    for version in ["1.1.0", "2.3.4"] {
        let pending = PanelState {
            update: Some(core::panel_state::UpdateInfo {
                available: true,
                latest: version.into(),
                ..Default::default()
            }),
            ..state()
        };
        assert_eq!(
            rows_of(&resolve(&pending), "update")[0].label,
            format!("TailGauge {version} is available")
        );
    }
}
