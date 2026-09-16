//! The parity rule, enforced rather than remembered.
//!
//! This crate decides what the panel contains; a frontend decides only how a
//! row looks. These tests fail when a layout decision leaks back into one
//! desktop, which is how the three would start to disagree.
//!
//! Ported from `test/parity.test.ts`, which went with the TypeScript model it
//! was guarding. The rules did not go with it: they are about the boundary
//! between this crate and the three frontends, and that boundary outlived the
//! model.

use std::path::PathBuf;

fn repo_file(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

const PLASMA: &[&str] = &[
    "plasma/org.tailgauge.plasmoid/contents/ui/FullRep.qml",
    "plasma/org.tailgauge.plasmoid/contents/ui/PanelRowView.qml",
    "plasma/org.tailgauge.plasmoid/contents/ui/CompactRep.qml",
    "plasma/org.tailgauge.plasmoid/contents/ui/main.qml",
    "plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml",
];
const GNOME: &[&str] = &[
    "gnome/tailgauge@arzaroth.github.io/extension.ts",
    "gnome/tailgauge@arzaroth.github.io/provider.ts",
];
const OMARCHY: &[&str] = &[
    "omarchy/arzaroth.tailgauge/Panel.qml",
    "omarchy/arzaroth.tailgauge/Service.qml",
];

fn source_of(files: &[&str]) -> String {
    files
        .iter()
        .map(|f| repo_file(f))
        .collect::<Vec<_>>()
        .join("\n")
}

fn frontends() -> Vec<(&'static str, String)> {
    vec![
        ("plasma", source_of(PLASMA)),
        ("gnome", source_of(GNOME)),
        ("omarchy", source_of(OMARCHY)),
    ]
}

/// Every string a frontend passes to its desktop's translator. Those are the
/// ones it wrote itself; everything else on screen arrived finished.
fn translated(source: &str, open: &str) -> Vec<String> {
    let quote = open.chars().last().expect("an opening delimiter");
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(open) {
        rest = &rest[at + open.len()..];
        let Some(end) = rest.find(quote) else {
            break;
        };
        let text = &rest[..end];
        if text.chars().count() >= 4 && !text.contains('\n') {
            out.push(text.to_string());
        }
        rest = &rest[end..];
    }
    out.sort();
    out.dedup();
    out
}

fn plasma_strings() -> Vec<String> {
    translated(&source_of(PLASMA), "i18n(\"")
}

fn gnome_strings() -> Vec<String> {
    translated(&source_of(GNOME), "_('")
}

/// Rows GNOME shows that Plasma puts in the applet context menu instead, and
/// the words a settings dialog needs. Both are desktop conventions, not panel
/// content, so they are allowed to differ - and they are the only strings a
/// frontend may write at all.
const DESKTOP_ONLY: &[&str] = &["Refresh", "Settings"];

/// Strings a frontend still writes about work it is doing or a command that
/// failed. The panel has no vocabulary for these yet, so they are recorded
/// here rather than passing unnoticed: a rule with a list of exceptions is
/// still a rule, and an empty list is the goal.
const PROGRESS_STRINGS: &[&str] = &[
    // The moment before the first answer arrives, which the binary cannot
    // provide because it has not answered yet.
    "TailGauge",
    "Checking…",
    "Turning it on…",
    "Authorizing the operator…",
    "Updating TailGauge…",
    "The command failed",
    "The update failed",
    "The panel could not be read",
    "Exit node: %1",
];

fn allowed(text: &str) -> bool {
    DESKTOP_ONLY.contains(&text) || PROGRESS_STRINGS.contains(&text)
}

#[test]
fn no_user_visible_string_is_written_in_both_frontends() {
    let gnome = gnome_strings();
    let shared: Vec<String> = plasma_strings()
        .into_iter()
        .filter(|s| gnome.contains(s) && !allowed(s))
        .collect();
    assert!(
        shared.is_empty(),
        "these belong in the panel, not in each frontend: {shared:?}"
    );
}

#[test]
fn frontend_local_strings_are_conventions_and_progress_only() {
    let stray: Vec<String> = plasma_strings()
        .into_iter()
        .chain(gnome_strings())
        .filter(|s| !allowed(s))
        .collect();
    assert!(
        stray.is_empty(),
        "the panel should be producing these: {stray:?}"
    );
}

/// Omarchy's shell has no translation layer, so its labels would be plain
/// strings rather than a call this can spot. The rule is checked from the
/// other end there: nothing user-visible is written in the file at all.
#[test]
fn the_omarchy_frontend_writes_no_user_visible_string() {
    let source = source_of(OMARCHY);
    let mut written = Vec::new();
    for property in [
        "text: ",
        "placeholderText: ",
        "tooltipText: ",
        "title: ",
        "label: ",
    ] {
        let mut rest = source.as_str();
        while let Some(at) = rest.find(property) {
            rest = &rest[at + property.len()..];
            let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                continue;
            };
            let Some(end) = rest[1..].find(quote) else {
                continue;
            };
            let text = &rest[1..1 + end];
            if text.chars().count() >= 4 && !allowed(text) {
                written.push(text.to_string());
            }
        }
    }
    assert!(
        written.is_empty(),
        "the panel should be producing these: {written:?}"
    );
}

#[test]
fn no_frontend_re_derives_which_sections_are_visible() {
    for (name, source) in frontends() {
        for marker in ["showConnections", "showExitNodes", "showPeers"] {
            assert!(
                !source.contains(marker),
                "{name} computes section visibility; the panel already did"
            );
        }
    }
}

#[test]
fn no_frontend_rebuilds_the_exit_node_or_copy_option_lists() {
    for (name, source) in frontends() {
        assert!(
            !source.contains("displayExitNodes"),
            "{name} assembles its own exit-node list"
        );
        assert!(
            !source.contains("peerCopyOptions"),
            "{name} assembles its own copy options"
        );
    }
}

#[test]
fn no_frontend_re_derives_the_status_precedence() {
    for (name, source) in frontends() {
        assert!(
            !source.contains("actionStatus !== \"\" ?")
                && !source.contains("actionStatus !== '' ?"),
            "{name} ranks actionStatus against lastError; the panel already did"
        );
    }
}

/// The panel writes the strings; a frontend that wraps one in its desktop's
/// translator is either translating something already finished, or writing one
/// of its own.
#[test]
fn no_frontend_hands_a_panel_string_to_a_translator() {
    for (name, source) in frontends() {
        assert!(
            !source.contains("Translate"),
            "{name} still passes a translator"
        );
    }
}

/// Every frontend draws what the binary sent and nothing it worked out itself.
/// The panel arrives; it is not derived.
#[test]
fn every_frontend_is_flipped_and_none_has_a_model_left_to_call() {
    let sets: [(&str, &[&str]); 3] = [("plasma", PLASMA), ("gnome", GNOME), ("omarchy", OMARCHY)];
    for (name, files) in sets {
        let source = source_of(files);
        assert!(
            !source.contains("model.js") && !source.contains("Model.js"),
            "{name} still imports a model"
        );
        for derived in [
            "resolvePanel",
            "parseStatus",
            "parseAccounts",
            "parseExitNodeList",
        ] {
            assert!(
                !source.contains(derived),
                "{name} still calls {derived} for itself"
            );
        }
        assert!(
            source.contains(r#"'tailgauge', 'panel', '--json'"#)
                || source.contains(r#""tailgauge", "panel", "--json""#),
            "{name} does not ask the binary for its panel"
        );
    }
}

/// `panel` is a new object whenever the binary answers. A QML Repeater handed
/// that array rebuilds every delegate it holds, which destroyed the field being
/// typed into. Counted models keep the delegates and re-read them by index.
#[test]
fn no_qml_frontend_rebuilds_its_rows_just_to_redraw_them() {
    for (name, file) in [
        (
            "plasma",
            "plasma/org.tailgauge.plasmoid/contents/ui/FullRep.qml",
        ),
        ("omarchy", "omarchy/arzaroth.tailgauge/Panel.qml"),
    ] {
        let source = repo_file(file);
        assert!(
            source.contains("model: full.panel.sections.length")
                || source.contains("model: root.panel.sections.length"),
            "{name} hands the sections array to a Repeater"
        );
        assert!(
            source.contains(".modelData.rows.length : 0"),
            "{name} hands rows to a Repeater"
        );
        assert!(
            source.contains(".children.length : 0"),
            "{name} hands children to a Repeater"
        );
    }
}

/// Whatever a frontend does to redraw, the field someone is typing into has to
/// come out the other side.
#[test]
fn every_frontend_keeps_its_search_field_across_a_redraw() {
    for file in [
        "plasma/org.tailgauge.plasmoid/contents/ui/PanelRowView.qml",
        "omarchy/arzaroth.tailgauge/Panel.qml",
    ] {
        assert!(
            repo_file(file).contains("function syncRegistration()"),
            "{file} registers rows by component lifetime, which no longer tracks the row"
        );
    }
    // GNOME rebuilds its menu outright, so it hides the rows a query excludes
    // rather than rebuilding around the entry, and gates a rebuild on the
    // signature - which the query is not an input to.
    let gnome = repo_file("gnome/tailgauge@arzaroth.github.io/extension.ts");
    assert!(gnome.contains("this._applySearch()"));
    assert!(gnome.contains("signature !== this._signature"));
}

/// The panel says what a row is findable by and hands it over as `searchKey`.
/// A frontend that builds a haystack of its own is a fourth search behaviour.
#[test]
fn no_frontend_assembles_its_own_search_haystack() {
    // The file that draws the panel, rather than the one that runs commands: a
    // service legitimately reads a peer's name to copy it, but nothing that
    // decides which rows a query leaves behind may look past `searchKey`.
    for (name, file) in [
        (
            "plasma",
            "plasma/org.tailgauge.plasmoid/contents/ui/FullRep.qml",
        ),
        ("gnome", "gnome/tailgauge@arzaroth.github.io/extension.ts"),
        ("omarchy", "omarchy/arzaroth.tailgauge/Panel.qml"),
    ] {
        let source = repo_file(file);
        assert!(source.contains("searchKey"), "{name} never reads searchKey");
        for field in [
            "DisplayName",
            "HostName",
            "DNSName",
            "UserName",
            "City",
            "Country",
        ] {
            assert!(
                !source.contains(&format!(".{field}")),
                "{name} reads {field} where it filters; searchKey already carries it"
            );
        }
    }
}

#[test]
fn no_frontend_gates_a_control_on_background_work() {
    // One line at a time: `source` is every file of a frontend joined, so
    // asking whether it contains both spellings anywhere passes whenever one
    // of them is absent and fails on two unrelated lines.
    for (name, source) in frontends() {
        for (number, line) in source.lines().enumerate() {
            let gated = line.contains("enabled:") || line.contains("enabled =");
            assert!(
                !(gated && line.contains("busy")),
                "{name}:{} disables a control while busy; the panel decides that: {}",
                number + 1,
                line.trim()
            );
        }
    }
}

#[test]
fn every_frontend_tells_the_service_when_the_panel_is_on_screen() {
    for (name, source) in frontends() {
        assert!(
            source.contains("attentive"),
            "{name} never says whether it is on screen"
        );
    }
}

/// A provider binary named in a frontend is a command that will one day be
/// fired at the wrong daemon. The argv belongs to the registry, which is the
/// only place that knows which CLI is active.
#[test]
fn no_frontend_names_a_provider_binary_in_an_argv() {
    for (name, source) in frontends() {
        for provider in tailgauge_core::providers::PROVIDERS {
            // In argv position, which is what would reach a daemon. A provider
            // id compared against to pick an icon is this file's own business.
            for spelling in [
                format!("\"{}\",", provider.cli),
                format!("'{}',", provider.cli),
            ] {
                assert!(
                    !source.contains(&spelling),
                    "{name} names {} in an argv; the registry owns that",
                    provider.cli
                );
            }
        }
    }
}

/// A control the panel puts on the header has to appear on every desktop, or
/// the panels differ in what you can do rather than in how they look.
#[test]
fn every_frontend_draws_the_header_controls_it_is_handed() {
    for (name, source) in frontends() {
        assert!(
            source.contains("header.actions"),
            "{name} never reads the header's actions"
        );
    }
}

/// The GNOME extension builds one menu slot per section id and silently drops
/// a section it has no slot for. That is how provider switching and network
/// selection went missing there while their actions sat in the dispatch, so
/// the list it builds from is held against the sections the panel emits.
#[test]
fn the_gnome_extension_has_a_slot_for_every_section_the_panel_emits() {
    let source = repo_file("gnome/tailgauge@arzaroth.github.io/panel.ts");
    let declared = source
        .split_once("export const SECTION_IDS = [")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(list, _)| {
            list.split(',')
                .map(|id| id.trim().trim_matches('\'').trim_matches('"').to_string())
                .filter(|id| !id.is_empty())
                .collect::<Vec<_>>()
        })
        .expect("panel.ts declares SECTION_IDS");

    let state = tailgauge_core::panel_state::PanelState {
        installed: true,
        running: true,
        helpers: true,
        active: true,
        active_provider_id: "tailscale".into(),
        ..Default::default()
    };
    let emitted: Vec<String> = tailgauge_core::panel::panel_spec(&state, &Default::default())
        .sections
        .iter()
        .map(|s| s.id.clone())
        .collect();

    assert_eq!(
        declared, emitted,
        "panel.ts must list every section the panel emits, in the order it emits them"
    );
}
