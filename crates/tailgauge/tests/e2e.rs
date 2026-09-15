//! The binary, end to end, against daemons that are not there.
//!
//! Everything else in this repository tests a function. This runs the shipped
//! executable the way a panel runs it - probe PATH, poll whatever answered,
//! parse it, resolve the panel, print JSON - against fake `tailscale` and
//! `netbird` commands that print captures of what the real ones said.
//!
//! It is the only test that covers the gather, which is the one part of the
//! port with no counterpart to be held against: the TypeScript model never
//! gathered, the frontends did.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .join("tailgauge-core/tests/fixtures")
}

/// A machine with the given CLIs on its PATH and nothing else of ours.
struct Machine {
    home: PathBuf,
}

impl Machine {
    fn with(clis: &[&str]) -> Machine {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let home = std::env::temp_dir().join(format!("tg-e2e-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("bin")).expect("a bin dir");

        for cli in clis {
            let script = match *cli {
                "tailscale" => format!(
                    "#!/bin/sh\ncase \"$1 $2\" in\n\
                     'status --json') /bin/cat '{f}/status.json' ;;\n\
                     'exit-node list') /bin/cat '{f}/exit-nodes.txt' ;;\n\
                     'switch --list') /bin/cat '{f}/accounts.json' ;;\n\
                     *) exit 1 ;;\nesac\n",
                    f = fixtures().display()
                ),
                "netbird" => format!(
                    "#!/bin/sh\ncase \"$1 $2\" in\n\
                     'status --json') /bin/cat '{f}/netbird-status.json' ;;\n\
                     'networks list') /bin/cat '{f}/netbird-networks.txt' ;;\n\
                     *) exit 1 ;;\nesac\n",
                    f = fixtures().display()
                ),
                // A daemon that is installed but answering nothing, which is
                // what a stopped service looks like.
                "tailscale-down" => "#!/bin/sh\nexit 1\n".to_string(),
                other => panic!("no fake {other}"),
            };
            let name = if *cli == "tailscale-down" {
                "tailscale"
            } else {
                cli
            };
            let path = home.join("bin").join(name);
            std::fs::write(&path, script).expect("the fake CLI");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("executable");
            }
        }
        Machine { home }
    }

    /// Run the shipped binary as a panel would, with only our fakes visible.
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_tailgauge"))
            .args(args)
            // Nothing but the fakes. Leaving the system directories on would
            // let a real tailscale on the machine running these tests answer
            // instead of the capture - which is exactly what the first
            // version of this did, and what the no-CLI case caught.
            .env("PATH", format!("{}/bin", self.home.display()))
            .env("HOME", &self.home)
            .env("XDG_CACHE_HOME", self.home.join("cache"))
            .env("XDG_DATA_HOME", self.home.join("data"))
            .env("XDG_CONFIG_HOME", self.home.join("config"))
            .output()
            .expect("the binary runs");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    fn panel(&self, ui: &str) -> Value {
        let (code, stdout, stderr) = self.run(&["panel", "--json", "--ui", ui]);
        assert_eq!(code, 0, "panel failed: {stderr}");
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("not JSON: {e}\n{stdout}"))
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn section<'a>(panel: &'a Value, id: &str) -> &'a Value {
    panel["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .find(|s| s["id"] == id)
        .unwrap_or_else(|| panic!("no section {id}"))
}

fn labels(section: &Value) -> Vec<String> {
    section["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|r| r["label"].as_str().unwrap_or("").to_string())
        .collect()
}

#[test]
fn a_machine_with_no_vpn_cli_is_told_what_would_work() {
    let machine = Machine::with(&[]);
    let panel = machine.panel("{}");

    assert_eq!(
        panel["status"]["text"],
        "No supported VPN CLI on PATH. Looked for Tailscale, NetBird."
    );
    assert_eq!(panel["header"]["title"], "TailGauge");
    assert_eq!(panel["header"]["toggleVisible"], false);
    assert_eq!(
        panel["navigation"].as_array().expect("nav").len(),
        1,
        "the header alone"
    );
    assert_eq!(panel["bar"]["crossed"], true);
}

#[test]
fn a_running_tailnet_comes_back_as_the_panel_a_frontend_draws() {
    let machine = Machine::with(&["tailscale"]);
    let panel = machine.panel(r#"{"version":"9.9.9"}"#);

    assert_eq!(panel["header"]["title"], "workstation");
    assert_eq!(panel["header"]["toggleChecked"], true);
    assert_eq!(panel["header"]["toggleHint"], "Turn Tailscale off");
    assert_eq!(panel["status"]["text"], "");
    assert_eq!(panel["footer"], "TailGauge v9.9.9");
    assert_eq!(panel["bar"]["connected"], true);

    // The machines the capture holds, in the order the panel puts them.
    let machines = section(&panel, "machines");
    assert_eq!(machines["visible"], true);
    let peers: Vec<String> = machines["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|r| r["kind"] == "peer")
        .map(|r| r["label"].as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(peers, ["laptop", "phone", "router", "offline-box"]);

    // This device, with the copy options a machine row gets.
    let this = section(&panel, "self");
    assert_eq!(this["visible"], true);
    assert_eq!(this["rows"][0]["sublabel"], "100.64.0.1 · Alice");

    // Every cursor stop resolves to a row that is drawn.
    let drawn: Vec<&str> = panel["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .filter(|s| s["visible"] == true)
        .flat_map(|s| s["rows"].as_array().expect("rows"))
        .flat_map(|r| std::iter::once(r).chain(r["children"].as_array().expect("children")))
        .map(|r| r["id"].as_str().unwrap_or(""))
        .collect();
    for entry in panel["navigation"].as_array().expect("nav").iter().skip(1) {
        let id = entry["rowId"].as_str().unwrap_or("");
        assert!(
            drawn.contains(&id),
            "{id} is a cursor stop but is not drawn"
        );
    }
}

#[test]
fn the_exit_node_table_becomes_a_picker_of_regions() {
    let machine = Machine::with(&["tailscale"]);
    let panel = machine.panel(r#"{"mullvadPickerOpen":true}"#);

    let exit_nodes = section(&panel, "exitNodes");
    assert_eq!(exit_nodes["visible"], true);

    let picker = exit_nodes["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|r| r["kind"] == "mullvadPicker")
        .expect("the picker");
    assert_eq!(picker["expanded"], true);

    let cities: Vec<String> = picker["children"]
        .as_array()
        .expect("children")
        .iter()
        .filter(|c| c["kind"] == "mullvadRegion")
        .map(|c| c["label"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(cities.contains(&"Paris".to_string()), "{cities:?}");

    // Every region carries the key a frontend filters on, lowercased.
    for child in picker["children"].as_array().expect("children") {
        if child["kind"] != "mullvadRegion" {
            continue;
        }
        let key = child["searchKey"].as_str().expect("a key");
        assert_eq!(key, key.to_lowercase());
        assert_eq!(child["searchScope"], "mullvad");
    }
}

#[test]
fn the_bar_describes_the_machine_rather_than_the_provider_being_shown() {
    let machine = Machine::with(&["tailscale", "netbird"]);
    let panel = machine.panel(r#"{"activeProviderId":"netbird"}"#);

    // The panel shows NetBird...
    assert_eq!(panel["header"]["providerId"], "netbird");
    assert_eq!(section(&panel, "networks")["visible"], true);
    assert_eq!(
        section(&panel, "exitNodes")["visible"],
        false,
        "NetBird has none"
    );

    // ...and the tooltip still answers for both, the shown one first.
    let tooltip: Vec<String> = panel["bar"]["tooltip"]
        .as_array()
        .expect("tooltip")
        .iter()
        .map(|l| l.as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(tooltip.len(), 2);
    assert!(tooltip[0].starts_with("NetBird"), "{tooltip:?}");
    assert!(tooltip[1].starts_with("Tailscale"), "{tooltip:?}");
    // Both lines the same width, because the bar centres each on its own.
    assert_eq!(tooltip[0].chars().count(), tooltip[1].chars().count());

    // The switcher offers both.
    assert_eq!(
        labels(section(&panel, "providers")),
        ["Tailscale", "NetBird"]
    );
}

#[test]
fn a_daemon_that_answers_nothing_reads_as_disconnected() {
    let machine = Machine::with(&["tailscale-down"]);
    let panel = machine.panel("{}");

    assert_eq!(panel["header"]["toggleChecked"], false);
    assert_eq!(panel["header"]["toggleHint"], "Turn Tailscale on");
    assert_eq!(panel["header"]["meta"], "Tailscale is disconnected");
    assert_eq!(
        panel["header"]["toggleVisible"], true,
        "installed, just not up"
    );
    assert_eq!(section(&panel, "machines")["visible"], false);
    assert_eq!(panel["bar"]["crossed"], true);
}

#[test]
fn a_click_shows_what_was_asked_until_the_daemon_agrees() {
    // The optimistic toggle, which is the one piece of state a frontend keeps
    // and the binary honours: `false` while the daemon still says it is up.
    let machine = Machine::with(&["tailscale"]);

    let honest = machine.panel("{}");
    assert_eq!(honest["header"]["toggleChecked"], true);

    let clicked = machine.panel(r#"{"active":false}"#);
    assert_eq!(clicked["header"]["toggleChecked"], false);
    assert_eq!(clicked["header"]["dimmed"], true);
    assert_eq!(
        section(&clicked, "machines")["visible"],
        false,
        "an off panel draws no machines, however many the daemon still lists"
    );
}

#[test]
fn what_is_in_flight_reaches_the_row_it_belongs_to() {
    let machine = Machine::with(&["tailscale"]);
    let panel = machine.panel(r#"{"actionStatus":"Connecting…","lastError":"stale"}"#);

    // Progress beats a stale error, and both beat the idle line.
    assert_eq!(panel["status"]["text"], "Connecting…");
    assert_eq!(panel["status"]["tone"], "dim");

    let failed = machine.panel(r#"{"lastError":"Something broke"}"#);
    assert_eq!(failed["status"]["text"], "Something broke");
    assert_eq!(failed["status"]["tone"], "error");
}

#[test]
fn the_panel_never_waits_on_github() {
    // A cold cache means no update has been checked for, not a network round
    // trip while the panel is being drawn.
    let machine = Machine::with(&["tailscale"]);
    let started = std::time::Instant::now();
    let panel = machine.panel("{}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(3),
        "the panel waited on something it should not have"
    );
    assert_eq!(section(&panel, "update")["visible"], false);
}

#[test]
fn ctl_drives_whichever_provider_is_asked_for() {
    let machine = Machine::with(&["tailscale", "netbird"]);

    let (code, stdout, _) = machine.run(&["ctl", "status"]);
    assert_eq!(code, 0, "{stdout}");
    assert!(
        stdout.starts_with("Connected as workstation (100.64.0.1)"),
        "{stdout}"
    );

    let (code, stdout, _) = machine.run(&["ctl", "--provider", "netbird", "status"]);
    assert_eq!(code, 0);
    assert!(
        stdout.starts_with("Connected as workstation (100.85.0.1)"),
        "{stdout}"
    );

    // A capability the provider does not have is refused by name rather than
    // fired at the other provider's binary.
    let (code, _, stderr) = machine.run(&["ctl", "--provider", "netbird", "exit-nodes"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("NetBird has no exit nodes"), "{stderr}");
}

#[test]
fn ctl_reports_a_daemon_that_is_not_answering_without_failing() {
    let machine = Machine::with(&["tailscale-down"]);
    let (code, stdout, _) = machine.run(&["ctl", "status"]);
    // 3 is "not connected", which is a state a script can gate on rather than
    // a fault it has to handle.
    assert_eq!(code, 3);
    assert!(
        stdout.contains("not answering") || stdout.contains("Disconnected"),
        "{stdout}"
    );
}

#[test]
fn a_row_hands_its_own_payload_back_to_set_an_exit_node() {
    // The round trip a click makes: the panel names a node, the frontend hands
    // that payload back, and the binary works out the address from it. A
    // Mullvad node is set by address where a tailnet one is set by name, and
    // neither side of that is the frontend's to decide.
    let machine = Machine::with(&["tailscale"]);
    let panel = machine.panel(r#"{"mullvadPickerOpen":true}"#);
    let picker = section(&panel, "exitNodes")["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|r| r["kind"] == "mullvadPicker")
        .expect("the picker")
        .clone();
    let region = picker["children"]
        .as_array()
        .expect("children")
        .iter()
        .find(|c| c["kind"] == "mullvadRegion")
        .expect("a region")
        .clone();

    let payload = serde_json::to_string(&region["payload"]).expect("a payload");
    // The fake CLI refuses `set`, so this asserts the binary got as far as
    // running it rather than rejecting the payload.
    let (code, _, stderr) = machine.run(&["ctl", "exit-node", "--peer", &payload]);
    assert_eq!(code, 1, "the fake tailscale refuses `set`");
    assert!(stderr.contains("could not set the exit node"), "{stderr}");

    // A payload that is not a peer is refused before anything is run.
    let (code, _, stderr) = machine.run(&["ctl", "exit-node", "--peer", "not json"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("--peer is not a peer"), "{stderr}");
}

/// Every `tailgauge …` a frontend spells out has to be a command this binary
/// answers to. This is the drift the port is most exposed to: the frontends
/// name their commands in strings, and nothing else would notice a rename.
#[test]
fn every_command_the_frontends_invoke_exists() {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = crates
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let sources: Vec<String> = [
        "plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml",
        "omarchy/arzaroth.tailgauge/Service.qml",
        "omarchy/arzaroth.tailgauge/Panel.qml",
        "gnome/tailgauge@arzaroth.github.io/provider.ts",
    ]
    .iter()
    .map(|f| std::fs::read_to_string(repo.join(f)).unwrap_or_else(|e| panic!("{f}: {e}")))
    .collect();
    let source = sources.join("\n");

    let mut paths: Vec<Vec<String>> = Vec::new();

    // `["tailgauge", "panel", …]` and friends: the subcommand is the literal
    // after the binary's name.
    for open in ["\"tailgauge\", \"", "'tailgauge', '"] {
        let quote = open.chars().last().expect("a quote");
        let mut rest = source.as_str();
        while let Some(at) = rest.find(open) {
            rest = &rest[at + open.len()..];
            let Some(end) = rest.find(quote) else { break };
            paths.push(vec![rest[..end].to_string()]);
            rest = &rest[end..];
        }
    }

    // `_ctl(kind, "switch-account", …)`: the action is the second argument.
    for open in [
        "_ctl(actionProc, \"",
        "_ctl(switchProc, \"",
        "_ctl(exitNodeProc, \"",
        "_ctl(selectNetworkProc, \"",
        "_ctl(operatorProc, \"",
        "_ctl(\"action\", \"",
        "_ctl(\"switch\", \"",
        "_ctl(\"exitNode\", \"",
        "_ctl(\"network\", \"",
        "_ctl(\"operator\", \"",
        "_ctl('action', '",
        "_ctl('switch', '",
        "_ctl('exitNode', '",
        "_ctl('network', '",
        "_ctl('operator', '",
    ] {
        let quote = open.chars().last().expect("a quote");
        let mut rest = source.as_str();
        while let Some(at) = rest.find(open) {
            rest = &rest[at + open.len()..];
            let Some(end) = rest.find(quote) else { break };
            paths.push(vec!["ctl".to_string(), rest[..end].to_string()]);
            rest = &rest[end..];
        }
    }

    paths.sort();
    paths.dedup();
    assert!(paths.len() > 8, "found almost nothing to check: {paths:?}");

    let machine = Machine::with(&[]);
    for path in &paths {
        let mut args: Vec<&str> = path.iter().map(String::as_str).collect();
        args.push("--help");
        let (code, _, stderr) = machine.run(&args);
        assert_eq!(
            code,
            0,
            "the frontends invoke `tailgauge {}`: {stderr}",
            path.join(" ")
        );
    }
}
