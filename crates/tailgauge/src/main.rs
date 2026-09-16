//! The binary every TailGauge frontend shells out to.
//!
//! One executable with a subcommand per job, and a symlink per subcommand
//! under the name the shell helper used to have - so a key binding on
//! `tailgauge-ctl toggle` keeps working, and there is one copy of the argument
//! parsing rather than eight.

mod copy;
mod ctl;
mod file_select;
mod gather;
mod notify;
mod project;
mod receive;
mod send;
mod tailscale;
mod watch;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result};
use selvedge::{frontend, proc, state, update};

use crate::project::TAILGAUGE;
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
    Send {
        /// A row's payload instead of a name, so the address is the model's
        /// answer rather than the caller's guess.
        #[arg(long)]
        peer: Option<String>,
        /// MACHINE then the files, or just the files when --peer names the
        /// machine. One list, because clap would otherwise read the first file
        /// of a `--peer` call as the machine and refuse the command.
        #[arg(value_name = "MACHINE|FILES", required_unless_present = "peer")]
        target: Vec<String>,
    },

    /// Save incoming Taildrop files and announce each one.
    Receive {
        /// Deliver one batch and stop, rather than running as a service.
        #[arg(long)]
        once: bool,
        directory: Option<PathBuf>,
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
    ExitNode {
        name: Option<String>,
        /// A row's payload, handed back rather than read: which address a peer
        /// is reached at is the model's rule, not the caller's.
        #[arg(long, conflicts_with = "name")]
        peer: Option<String>,
    },
    /// Switch to another profile of the active provider.
    SwitchAccount { id: String },
    /// Join a network, or leave it with --leave.
    SelectNetwork {
        id: String,
        #[arg(long)]
        leave: bool,
    },
    /// List the exit nodes this tailnet offers.
    ExitNodes,
    /// Let this user operate the daemon's profile.
    Authorize,
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
                CtlAction::ExitNode { name, peer } => {
                    let resolved = match peer.as_deref().map(ctl::target_of) {
                        Some(Err(why)) => {
                            eprintln!("tailgauge: {why}");
                            return ExitCode::FAILURE;
                        }
                        Some(Ok(target)) => Some(target),
                        None => name,
                    };
                    settle(ctl::exit_node(provider, resolved.as_deref()))
                }
                CtlAction::SwitchAccount { id } => settle(ctl::switch_account(provider, &id)),
                CtlAction::SelectNetwork { id, leave } => {
                    settle(ctl::select_network(provider, &id, !leave))
                }
                CtlAction::ExitNodes => settle(ctl::exit_nodes(provider)),
                CtlAction::Authorize => settle(ctl::authorize(provider)),
                CtlAction::Version => unreachable!("answered before the provider check"),
            }
        }

        Command::Watch { timeout_seconds } => {
            ExitCode::from(watch::run(Duration::from_secs(timeout_seconds)).code())
        }

        Command::Send { peer, target } => {
            if !tailscale::installed() {
                eprintln!("tailgauge: the tailscale CLI is not on PATH");
                return ExitCode::FAILURE;
            }
            let (machine, files) = match peer.as_deref().map(ctl::address_of) {
                Some(Err(why)) => {
                    eprintln!("tailgauge: {why}");
                    return ExitCode::FAILURE;
                }
                Some(Ok(address)) => (address, target),
                None => {
                    let mut rest = target.into_iter();
                    (rest.next().unwrap_or_default(), rest.collect())
                }
            };
            if machine.is_empty() {
                eprintln!("tailgauge: nothing to send to");
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
    let status = update::check_cached(&TAILGAUGE, &state::update_cache_file(&TAILGAUGE), force)?;
    println!("{}", serde_json::to_string(&status)?);
    Ok(())
}

/// `--update`: download the latest release, swap the binary, and refresh
/// whichever frontends are installed.
fn handle_update() -> Result<()> {
    let current = TAILGAUGE.version;
    println!("Current version: {current}");
    println!("Checking for updates...");
    let applied = update::apply(&TAILGAUGE, &state::update_cache_file(&TAILGAUGE))?;
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
        TAILGAUGE.frontends.iter().collect()
    } else {
        vec![frontend::find(&TAILGAUGE, &spec).ok_or_else(|| {
            let ids: Vec<&str> = TAILGAUGE.frontends.iter().map(|f| f.id).collect();
            anyhow::anyhow!("unknown frontend '{spec}' (known: {}, all)", ids.join(", "))
        })?]
    };

    let version = TAILGAUGE.version;
    for target in &wanted {
        println!("Installing the {} from v{version}...", target.label);
    }

    let outcomes = update::install_frontends(&TAILGAUGE, &wanted, version)?;
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
    for f in frontend::installed(&TAILGAUGE) {
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
    for f in frontend::installed(&TAILGAUGE) {
        if !f.schemas_ready() {
            println!(
                "{} has no compiled GSettings schemas - reinstall it: tailgauge --install-frontend {}",
                f.label, f.id
            );
        }
    }
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
        if !proc::has(provider.cli) {
            eprintln!("tailgauge: {} is not on PATH", provider.cli);
            return None;
        }
        return Some(provider);
    }

    let installed: Vec<_> = providers::PROVIDERS
        .iter()
        .filter(|p| proc::has(p.cli))
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
    splice_alias(std::env::args_os().collect())
}

fn splice_alias(mut args: Vec<OsString>) -> Vec<OsString> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> Cli {
        Cli::parse_from(splice_alias(args.iter().map(OsString::from).collect()))
    }

    fn spliced(args: &[&str]) -> Vec<String> {
        splice_alias(args.iter().map(OsString::from).collect())
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_symlink_names_the_subcommand_it_stands_for() {
        assert_eq!(
            spliced(&["/home/me/.local/bin/tailgauge-ctl", "toggle"]),
            ["/home/me/.local/bin/tailgauge-ctl", "ctl", "toggle"]
        );
        // Spliced rather than replaced: the parser still wants a program name
        // in the first slot, and what follows keeps its order.
        assert_eq!(
            spliced(&["tailgauge-watch", "5"]),
            ["tailgauge-watch", "watch", "5"]
        );
        assert_eq!(
            spliced(&["tailgauge-file-select", "--multiple"]),
            ["tailgauge-file-select", "file-select", "--multiple"]
        );
    }

    #[test]
    fn the_binary_under_its_own_name_is_left_alone() {
        assert_eq!(
            spliced(&["tailgauge", "ctl", "up"]),
            ["tailgauge", "ctl", "up"]
        );
        // A trailing dash names no subcommand, and splicing the empty string
        // in would make every invocation through it a parse error.
        assert_eq!(spliced(&["tailgauge-"]), ["tailgauge-"]);
        assert_eq!(spliced(&[]), [] as [&str; 0]);
    }

    /// The installer writes one symlink per alias, and a symlink whose name is
    /// not a subcommand is a command that exits 2 however it is invoked.
    #[test]
    fn every_alias_the_installer_writes_is_a_subcommand() {
        let installer = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/install.sh"),
        )
        .expect("the installer");
        let line = installer
            .lines()
            .find(|l| l.contains("for alias in"))
            .expect("the alias loop");
        let aliases: Vec<&str> = line
            .trim()
            .trim_start_matches("for alias in ")
            .trim_end_matches("; do")
            .split_whitespace()
            .collect();
        assert!(aliases.len() >= 7, "found {aliases:?}");

        for alias in aliases {
            let name = format!("tailgauge-{alias}");
            let argv = splice_alias(vec![OsString::from(&name)]);
            // `tailgauge-ctl` alone is a usage error rather than a parse
            // failure, so what is checked is that the name was recognised: an
            // alias with no subcommand behind it reads as garbage instead.
            if let Err(e) = Cli::try_parse_from(&argv) {
                assert!(
                    !matches!(
                        e.kind(),
                        clap::error::ErrorKind::InvalidSubcommand
                            | clap::error::ErrorKind::UnknownArgument
                    ),
                    "the {name} symlink names no subcommand: {e}"
                );
            }
        }
    }

    #[test]
    fn the_update_flags_do_one_thing_each() {
        assert!(resolve(Cli::parse_from(["tailgauge", "--update", "--check-update"])).is_err());
        assert!(
            resolve(Cli::parse_from(["tailgauge", "--update", "watch"])).is_err(),
            "an update flag and a subcommand do different jobs"
        );

        assert!(matches!(
            resolve(Cli::parse_from(["tailgauge", "--check-update", "--force"])),
            Ok(Action::CheckUpdate)
        ));
        assert!(matches!(
            resolve(Cli::parse_from(["tailgauge", "--install-frontend", "gnome"])),
            Ok(Action::InstallFrontend(spec)) if spec == "gnome"
        ));
        assert!(matches!(
            resolve(Cli::parse_from(["tailgauge"])),
            Ok(Action::Nothing)
        ));
    }

    #[test]
    fn a_peer_payload_and_a_name_are_two_ways_to_say_the_same_thing() {
        // Both set an exit node, and a caller passing each of them at once
        // means something this cannot resolve.
        assert!(
            Cli::try_parse_from(["tailgauge", "ctl", "exit-node", "berlin", "--peer", "{}"])
                .is_err()
        );
        let cli = parse(&["tailgauge-ctl", "exit-node", "--peer", r#"{"id":"n1"}"#]);
        let Some(Command::Ctl {
            action: CtlAction::ExitNode { name, peer },
            ..
        }) = cli.command
        else {
            panic!("not an exit-node command")
        };
        assert_eq!(name, None);
        assert_eq!(peer.as_deref(), Some(r#"{"id":"n1"}"#));
    }

    #[test]
    fn a_peer_payload_can_still_name_the_files_to_send() {
        // The frontends send with --peer and no files, so the chooser opens.
        // A caller naming both used to be refused: the first file bound to the
        // machine positional, which --peer conflicted with.
        let cli = parse(&["tailgauge-send", "--peer", r#"{"id":"n1"}"#, "a.md", "b.md"]);
        let Some(Command::Send { peer, target }) = cli.command else {
            panic!("not a send command")
        };
        assert_eq!(peer.as_deref(), Some(r#"{"id":"n1"}"#));
        assert_eq!(target, ["a.md", "b.md"]);

        // Without --peer the first positional is still the machine.
        let cli = parse(&["tailgauge-send", "box", "a.md"]);
        let Some(Command::Send { peer, target }) = cli.command else {
            panic!("not a send command")
        };
        assert_eq!(peer, None);
        assert_eq!(target, ["box", "a.md"]);

        // And naming nothing at all is still a usage error.
        assert!(Cli::try_parse_from(["tailgauge", "send"]).is_err());
    }

    #[test]
    fn the_panel_answers_a_frontend_that_names_no_state() {
        // Every frontend's first call is made before it knows anything, and a
        // required --ui would make that call an error.
        let cli = parse(&["tailgauge", "panel", "--json"]);
        let Some(Command::Panel { ui, .. }) = cli.command else {
            panic!("not a panel command")
        };
        assert_eq!(ui, "{}");
        assert!(serde_json::from_str::<gather::Ui>(&ui).is_ok());
    }

    #[test]
    fn a_notification_says_who_it_is_from_when_nothing_else_is_given() {
        let cli = parse(&["tailgauge-notify"]);
        let Some(Command::Notify {
            summary,
            body,
            urgency,
            ..
        }) = cli.command
        else {
            panic!("not a notify command")
        };
        assert_eq!(summary, "TailGauge");
        assert_eq!(body, "");
        assert_eq!(urgency, "normal");
    }
}
