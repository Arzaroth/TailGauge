//! The binary every TailGauge frontend shells out to.
//!
//! One executable with a subcommand per job, and a symlink per subcommand
//! under the name the shell helper used to have - so a key binding on
//! `tailgauge-ctl toggle` keeps working, and there is one copy of the argument
//! parsing rather than eight.

mod copy;
mod ctl;
mod file_select;
mod frontend;
mod gather;
mod launch;
mod notify;
mod receive;
mod send;
mod state;
mod tailscale;
mod update;
mod watch;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

/// Nothing to pick with, which is a different answer from picking nothing.
const EXIT_NO_CHOOSER: u8 = 2;

#[derive(Parser)]
#[command(name = "tailgauge", version, propagate_version = true)]
#[command(about = "Tailscale in the panel: the binary every TailGauge frontend drives")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Download the latest matching release from GitHub and replace the
    /// installed binary. Used by the panel's Install it now row too.
    #[arg(long)]
    update: bool,

    /// Query GitHub for the latest release, cache the result, and print it as
    /// JSON. Does not install anything.
    #[arg(long)]
    check_update: bool,

    /// Ignore the cached answer `--check-update` would otherwise serve.
    #[arg(long)]
    force: bool,

    /// Install a desktop frontend from the release this binary belongs to:
    /// `plasma`, `gnome`, `omarchy`, or `all`. Use it after switching desktops;
    /// `--update` already refreshes whichever are present.
    #[arg(long, value_name = "NAME")]
    install_frontend: Option<String>,
}

/// What one invocation does. The update flags are mutually exclusive and clap
/// cannot say so across a subcommand, so resolving them here makes the
/// precedence a list you can read, and turns a combination nobody meant into
/// an error rather than a silently dropped flag.
enum Action {
    CheckUpdate,
    Update,
    InstallFrontend(String),
    Sub(Command),
    /// No subcommand and no flag: there is no default job to fall back on.
    Nothing,
}

fn resolve(cli: Cli) -> Result<Action, String> {
    let flags = [cli.check_update, cli.update, cli.install_frontend.is_some()];
    let asked = flags.iter().filter(|set| **set).count();
    if asked > 1 {
        return Err("--check-update, --update and --install-frontend do one thing each".into());
    }
    if let Some(command) = cli.command {
        if asked > 0 {
            return Err("an update flag and a subcommand do different jobs".into());
        }
        return Ok(Action::Sub(command));
    }
    if cli.check_update {
        return Ok(Action::CheckUpdate);
    }
    if cli.update {
        return Ok(Action::Update);
    }
    if let Some(spec) = cli.install_frontend {
        return Ok(Action::InstallFrontend(spec));
    }
    Ok(Action::Nothing)
}

#[derive(Subcommand)]
enum Command {
    /// Drive the VPN daemon: the connection, and the exit node.
    Ctl {
        /// Which provider to act on. Defaults to the one the panel is showing,
        /// which is the first drivable one installed.
        #[arg(long)]
        provider: Option<String>,
        #[command(subcommand)]
        action: CtlAction,
    },

    /// Block until tailscaled reports a state change.
    ///
    /// Exits 0 when something moved, 2 when the wait expired, 1 when tailscale
    /// is unusable.
    Watch {
        #[arg(default_value_t = 300)]
        timeout_seconds: u64,
    },

    /// Send files to a tailnet machine, picking them when none are named.
    Send { machine: String, files: Vec<String> },

    /// Save incoming Taildrop files and announce each one.
    Receive {
        /// Deliver one batch and stop, rather than running as a service.
        #[arg(long)]
        once: bool,
        directory: Option<PathBuf>,
    },

    /// Internal: feed a fixture to the Rust model and print what it made of
    /// it, so `test/differential.test.ts` can hold the answer against the
    /// TypeScript's. Reads the fixture on stdin.
    #[command(hide = true)]
    InternalModel {
        #[arg(value_enum)]
        op: ModelOp,
    },

    /// Read the machine and print the panel every frontend draws.
    ///
    /// This is the call a panel makes on its own tick: one spawn, every
    /// provider polled at once, and the whole panel resolved.
    Panel {
        /// What only the frontend knows, as one JSON object: which provider is
        /// being shown, what it is optimistically showing, and what it has in
        /// flight. See `gather::Ui`.
        #[arg(long, default_value = "{}")]
        ui: String,
        /// Accepted and ignored: the output is JSON either way, and a frontend
        /// that spells it out reads more clearly at the call site.
        #[arg(long)]
        json: bool,
    },

    /// Copy text to the clipboard.
    Copy { text: Option<String> },

    /// Post a desktop notification.
    Notify {
        #[arg(long)]
        image: Option<String>,
        /// Make the notification open this path when it is clicked.
        #[arg(long)]
        open: Option<String>,
        #[arg(long, short = 'u', default_value = "normal")]
        urgency: String,
        /// Internal: this copy is the detached one that waits for the click.
        #[arg(long, hide = true)]
        await_action: bool,
        #[arg(default_value = "TailGauge")]
        summary: String,
        #[arg(default_value = "")]
        body: String,
    },

    /// Ask the desktop for files, one path per line on stdout.
    FileSelect {
        #[arg(long, default_value = "Select file")]
        title: String,
        #[arg(long)]
        multiple: bool,
    },
}

/// What the differential harness can ask the model for. One per ported
/// function, added as each one lands.
#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ModelOp {
    ParseStatus,
    ParseAccounts,
    ParseExitNodeList,
    MullvadRegionOptions,
    ParseNetbirdStatus,
    ParseNetbirdNetworks,
    ParseProviderProbe,
    /// Takes a PanelState rather than raw CLI output.
    BarState,
    /// Takes one value per line and prints one answer per line, so a whole
    /// table of cases crosses in a single spawn.
    FormatBytes,
    FormatSince,
    ElideStatus,
    FirstUrl,
    ShellCommand,
    /// Take a Peer per case and answer with what its row shows.
    PeerRow,
    /// Take a PanelState and answer with the header, status and footer.
    PanelChrome,
    /// The whole panel: a `[state, options]` pair per case.
    Panel,
}

#[derive(Subcommand)]
enum CtlAction {
    /// Print the connection state, and the exit node if there is one.
    ///
    /// Exits 0 when connected and 3 when it is not, so it can gate a script.
    Status,
    /// Turn Tailscale off if it is on, on if it is off.
    Toggle,
    /// Turn Tailscale on, opening the login page if it needs one.
    Up,
    /// Turn Tailscale off.
    Down,
    /// Print the current exit node, or route through NAME; "off" clears it.
    ExitNode { name: Option<String> },
    /// List the exit nodes this tailnet offers.
    ExitNodes,
    /// Print the version of the binary, not of the widgets.
    Version,
}

fn main() -> ExitCode {
    let cli = Cli::parse_from(argv());
    let force = cli.force;
    let action = match resolve(cli) {
        Ok(action) => action,
        Err(why) => {
            eprintln!("tailgauge: {why}");
            return ExitCode::from(2);
        }
    };

    let command = match action {
        Action::Nothing => {
            let _ = <Cli as clap::CommandFactory>::command().print_help();
            return ExitCode::from(2);
        }
        Action::CheckUpdate => return report(handle_check_update(force)),
        Action::Update => return report(handle_update()),
        Action::InstallFrontend(spec) => return report(handle_install_frontend(&spec)),
        Action::Sub(command) => command,
    };

    match command {
        Command::Ctl { provider, action } => {
            // Answered before the provider check: which parts are installed is
            // a fair question on a machine where no CLI is.
            if matches!(action, CtlAction::Version) {
                println!("tailgauge-ctl {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            let Some(provider) = resolve_provider(provider.as_deref()) else {
                return ExitCode::FAILURE;
            };
            match action {
                CtlAction::Status => settle(ctl::status(provider)),
                CtlAction::Toggle => settle(ctl::toggle(provider)),
                CtlAction::Up => settle(ctl::up(provider)),
                CtlAction::Down => settle(ctl::down(provider)),
                CtlAction::ExitNode { name } => settle(ctl::exit_node(provider, name.as_deref())),
                CtlAction::ExitNodes => settle(ctl::exit_nodes(provider)),
                CtlAction::Version => unreachable!("answered before the provider check"),
            }
        }

        Command::Watch { timeout_seconds } => {
            ExitCode::from(watch::run(Duration::from_secs(timeout_seconds)).code())
        }

        Command::Send { machine, files } => {
            if !tailscale::installed() {
                eprintln!("tailgauge: the tailscale CLI is not on PATH");
                return ExitCode::FAILURE;
            }
            match send::run(&machine, &files) {
                send::Outcome::Sent | send::Outcome::NothingToSend => ExitCode::SUCCESS,
                send::Outcome::Failed => ExitCode::FAILURE,
            }
        }

        Command::Receive { once, directory } => {
            if !tailscale::installed() {
                eprintln!("tailgauge: the tailscale CLI is not on PATH");
                return ExitCode::FAILURE;
            }
            let dir = directory.unwrap_or_else(receive::default_dir);
            report(receive::run(&dir, once))
        }

        Command::InternalModel { op } => report(run_model_op(op)),

        Command::Panel { ui, json: _ } => report(run_panel(&ui)),

        Command::Copy { text } => report(copy::run(text.as_deref().unwrap_or(""))),

        Command::Notify {
            image,
            open,
            urgency,
            await_action,
            summary,
            body,
        } => {
            let n = notify::Notification {
                summary: &summary,
                body: &body,
                urgency: &urgency,
                image: image.as_deref(),
                open: open.as_deref(),
            };
            match (await_action, open.as_deref()) {
                (true, Some(path)) => report(notify::await_action(&n, path)),
                _ => report(notify::run(&n)),
            }
        }

        Command::FileSelect { title, multiple } => match file_select::run(&title, multiple) {
            file_select::Picked::Files(files) => {
                for file in files {
                    println!("{file}");
                }
                ExitCode::SUCCESS
            }
            file_select::Picked::Cancelled => ExitCode::FAILURE,
            file_select::Picked::NoChooser => {
                eprintln!("tailgauge: install zenity or kdialog to pick files");
                ExitCode::from(EXIT_NO_CHOOSER)
            }
        },
    }
}

// ---------------------------------------------------------------------------
// updating
// ---------------------------------------------------------------------------

/// `--check-update`: cached where it can be, live otherwise, and the status as
/// JSON either way. This is what the three panels poll.
fn handle_check_update(force: bool) -> Result<()> {
    let status = update::check_cached(&state::update_cache_file(), force)?;
    println!("{}", serde_json::to_string(&status)?);
    Ok(())
}

/// `--update`: download the latest release, swap the binary, and refresh
/// whichever frontends are installed.
fn handle_update() -> Result<()> {
    let current = update::current_version();
    println!("Current version: {current}");
    println!("Checking for updates...");
    let applied = update::apply(&state::update_cache_file())?;
    if !update::version_gt(&applied.version, current) {
        println!("Already up to date ({current}).");
        report_frontend_skew(current);
        return Ok(());
    }

    println!("Updated to {}.", applied.version);
    report_frontends(&applied.frontends);
    Ok(())
}

/// `--install-frontend`: put one on a machine that does not have it yet.
fn handle_install_frontend(spec: &str) -> Result<()> {
    let spec = spec.trim().to_lowercase();
    let wanted: Vec<&'static frontend::Frontend> = if spec == "all" {
        frontend::FRONTENDS.iter().collect()
    } else {
        vec![frontend::find(&spec).ok_or_else(|| {
            let ids: Vec<&str> = frontend::FRONTENDS.iter().map(|f| f.id).collect();
            anyhow::anyhow!("unknown frontend '{spec}' (known: {}, all)", ids.join(", "))
        })?]
    };

    let version = update::current_version();
    for target in &wanted {
        println!("Installing the {} from v{version}...", target.label);
    }

    let outcomes = update::install_frontends(&wanted, version)?;
    report_frontends(&outcomes);

    let failed: Vec<&str> = outcomes
        .iter()
        .filter(|o| o.error.is_some())
        .map(|o| o.id)
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "{} frontend(s) failed to install: {}",
            failed.len(),
            failed.join(", ")
        ))
    }
}

/// The desktop frontends are QML and JavaScript installed outside the binary
/// directory, so an update that only swapped the binary would leave them
/// behind - silently, because a panel reports the version it was built with.
fn report_frontends(outcomes: &[update::FrontendOutcome]) {
    if outcomes.is_empty() {
        return;
    }
    println!();
    for f in outcomes {
        match &f.error {
            Some(e) => eprintln!("{}: NOT updated - {e}", f.label),
            None => match &f.version {
                Some(v) => println!("{} updated to {v}.", f.label),
                None => println!("{} updated.", f.label),
            },
        }
    }

    let hints: Vec<&update::FrontendOutcome> =
        outcomes.iter().filter(|f| f.error.is_none()).collect();
    if hints.is_empty() {
        return;
    }
    println!();
    for f in hints {
        let urgency = if f.needs_session_restart {
            "required"
        } else {
            "to load it"
        };
        println!("  {} ({urgency}): {}", f.label, f.restart_hint);
    }
}

/// An installed frontend that disagrees with the binary is the failure this all
/// exists to catch, so say so even on the path where nothing was updated.
fn report_frontend_skew(binary: &str) {
    for f in frontend::installed() {
        match f.installed_version() {
            Some(v) if v == binary => {}
            Some(v) => println!(
                "{} is still v{v} - update it: tailgauge --install-frontend {}",
                f.label, f.id
            ),
            None => println!(
                "{} has no readable version - reinstall it: tailgauge --install-frontend {}",
                f.label, f.id
            ),
        }
    }
    for f in frontend::installed() {
        if !f.schemas_ready() {
            println!(
                "{} has no compiled GSettings schemas - reinstall it: tailgauge --install-frontend {}",
                f.label, f.id
            );
        }
    }
}

/// The harness prints compact JSON on one line, because the test parses it
/// rather than reads it.
fn run_model_op(op: ModelOp) -> Result<()> {
    let mut raw = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut raw)?;
    use tailgauge_core as core;
    let answer = match op {
        ModelOp::ParseStatus => serde_json::to_string(&core::parse_status(&raw))?,
        ModelOp::ParseAccounts => serde_json::to_string(&core::parse_accounts(&raw))?,
        ModelOp::ParseExitNodeList => serde_json::to_string(&core::parse_exit_node_list(&raw))?,
        ModelOp::MullvadRegionOptions => serde_json::to_string(&core::mullvad_region_options(
            &core::parse_exit_node_list(&raw),
        ))?,
        ModelOp::ParseNetbirdStatus => serde_json::to_string(&core::parse_netbird_status(&raw))?,
        ModelOp::ParseNetbirdNetworks => {
            serde_json::to_string(&core::parse_netbird_networks(&raw))?
        }
        ModelOp::ParseProviderProbe => {
            serde_json::to_string(&core::providers::parse_provider_probe(&raw))?
        }
        ModelOp::BarState => {
            serde_json::to_string(&core::bar::bar_state(&serde_json::from_str(&raw)?))?
        }
        // The table ops take a JSON array of cases and answer in order. JSON
        // rather than a separator, because every separator worth choosing
        // turns up inside a shell argument or a CLI's complaint sooner or
        // later - which is exactly what the first attempt at this hit.
        ModelOp::FormatBytes => table::<i64>(&raw, |v| core::fmt::format_bytes(*v))?,
        ModelOp::FormatSince => {
            table::<(String, i64)>(&raw, |(v, now)| core::fmt::format_since(v, *now))?
        }
        ModelOp::ElideStatus => table::<String>(&raw, |t| {
            core::fmt::elide_status(t, core::fmt::STATUS_LIMIT)
        })?,
        ModelOp::FirstUrl => table::<(String, String)>(&raw, |(t, f)| core::fmt::first_url(t, f))?,
        ModelOp::ShellCommand => table::<Vec<String>>(&raw, |a| core::fmt::shell_command(a))?,
        ModelOp::PeerRow => {
            let cases: Vec<(core::Peer, i64)> = serde_json::from_str(&raw)?;
            let rows: Vec<_> = cases
                .iter()
                .map(|(peer, now)| {
                    serde_json::json!({
                        "address": core::panel::peer_address(peer),
                        "exitNodeTarget": core::panel::exit_node_target(peer),
                        "copyOptions": core::panel::peer_copy_options(peer),
                        "subtitle": core::panel::peer_subtitle(peer),
                        "rowSubtitle": core::panel::peer_row_subtitle(peer),
                        "connection": core::panel::connection_summary(peer),
                        "osIcon": core::panel::os_icon(&peer.os),
                        "osIconName": core::panel::os_icon_name(&peer.os),
                        "details": core::panel::peer_detail_rows(peer, *now),
                    })
                })
                .collect();
            serde_json::to_string(&rows)?
        }
        ModelOp::PanelChrome => {
            let cases: Vec<(core::panel_state::PanelState, i64)> = serde_json::from_str(&raw)?;
            let out: Vec<_> = cases
                .iter()
                .map(|(state, phrase)| {
                    serde_json::json!({
                        "header": core::panel::panel_header(state, *phrase),
                        "status": core::panel::panel_status(state),
                        "footer": core::panel::panel_footer(state),
                        "toggleHint": core::panel::toggle_hint(state),
                    })
                })
                .collect();
            serde_json::to_string(&out)?
        }
        ModelOp::Panel => {
            #[derive(serde::Deserialize, Default)]
            #[serde(default)]
            struct Options {
                #[serde(rename = "phraseIndex")]
                phrase_index: i64,
                #[serde(rename = "recentRegions")]
                recent_regions: Vec<String>,
                #[serde(rename = "mullvadPickerOpen")]
                mullvad_picker_open: bool,
                #[serde(rename = "expandedPeerId")]
                expanded_peer_id: String,
                #[serde(rename = "nowMs")]
                now_ms: i64,
            }
            let cases: Vec<(core::panel_state::PanelState, Options)> = serde_json::from_str(&raw)?;
            let panels: Vec<_> = cases
                .iter()
                .map(|(state, o)| {
                    core::panel::panel_spec(
                        state,
                        &core::panel::ResolveOptions {
                            phrase_index: o.phrase_index,
                            recent_regions: o.recent_regions.clone(),
                            mullvad_picker_open: o.mullvad_picker_open,
                            expanded_peer_id: o.expanded_peer_id.clone(),
                            now_ms: o.now_ms,
                        },
                    )
                })
                .collect();
            serde_json::to_string(&panels)?
        }
    };
    println!("{answer}");
    Ok(())
}

/// One answer per case, so a whole table crosses the process boundary in a
/// single spawn.
fn table<T: serde::de::DeserializeOwned>(
    raw: &str,
    answer: impl Fn(&T) -> String,
) -> serde_json::Result<String> {
    let cases: Vec<T> = serde_json::from_str(raw)?;
    serde_json::to_string(&cases.iter().map(&answer).collect::<Vec<String>>())
}

/// The provider to act on: the one named, or the one the panel would be
/// showing. Complains on stderr rather than returning an error, because the
/// two failures want different words.
fn resolve_provider(
    named: Option<&str>,
) -> Option<&'static tailgauge_core::providers::ProviderDescriptor> {
    use tailgauge_core::providers;
    if let Some(named) = named {
        let Some(provider) = providers::provider_by_id(named) else {
            let ids: Vec<&str> = providers::PROVIDERS.iter().map(|p| p.id).collect();
            eprintln!(
                "tailgauge: no provider '{named}' (known: {})",
                ids.join(", ")
            );
            return None;
        };
        if !launch::has(provider.cli) {
            eprintln!("tailgauge: {} is not on PATH", provider.cli);
            return None;
        }
        return Some(provider);
    }

    let installed: Vec<_> = providers::PROVIDERS
        .iter()
        .filter(|p| launch::has(p.cli))
        .collect();
    match installed.iter().find(|p| p.supported).or(installed.first()) {
        Some(provider) => Some(provider),
        None => {
            eprintln!(
                "tailgauge: no supported VPN CLI on PATH - looked for {}",
                providers::provider_cli_names().join(", ")
            );
            None
        }
    }
}

fn run_panel(ui: &str) -> Result<()> {
    let ui: gather::Ui =
        serde_json::from_str(ui).with_context(|| format!("--ui is not a JSON object: {ui}"))?;
    let state = gather::panel_state(&ui);
    let panel = tailgauge_core::panel::panel_spec(&state, &gather::resolve_options(&ui));
    println!("{}", serde_json::to_string(&panel)?);
    Ok(())
}

fn settle(outcome: ctl::Outcome) -> ExitCode {
    match outcome {
        ctl::Outcome::Ok => ExitCode::SUCCESS,
        ctl::Outcome::Disconnected => ExitCode::from(ctl::EXIT_DISCONNECTED),
        ctl::Outcome::Failed(why) => {
            eprintln!("tailgauge: {why}");
            ExitCode::FAILURE
        }
    }
}

fn report(result: anyhow::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tailgauge: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Invoked through one of the `tailgauge-*` symlinks, the name is the
/// subcommand. Splicing it in rather than branching keeps one parser: the
/// symlink and the subcommand cannot drift in what they accept.
fn argv() -> Vec<OsString> {
    let mut args: Vec<OsString> = std::env::args_os().collect();
    let alias = args
        .first()
        .and_then(|a| Path::new(a).file_name())
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix("tailgauge-"))
        .filter(|sub| !sub.is_empty())
        .map(str::to_string);
    if let Some(sub) = alias {
        args.insert(1, sub.into());
    }
    args
}
