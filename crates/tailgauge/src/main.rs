//! The binary every TailGauge frontend shells out to.
//!
//! One executable with a subcommand per job, and a symlink per subcommand
//! under the name the shell helper used to have - so a key binding on
//! `tailgauge-ctl toggle` keeps working, and there is one copy of the argument
//! parsing rather than eight.

mod copy;
mod ctl;
mod file_select;
mod launch;
mod notify;
mod tailscale;
mod watch;

use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};

/// Nothing to pick with, which is a different answer from picking nothing.
const EXIT_NO_CHOOSER: u8 = 2;

#[derive(Parser)]
#[command(name = "tailgauge", version, propagate_version = true)]
#[command(about = "Tailscale in the panel: the binary every TailGauge frontend drives")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Drive tailscaled: the connection, and the exit node.
    Ctl {
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
    ExitNode { name: Option<String> },
    /// List the exit nodes this tailnet offers.
    ExitNodes,
    /// Print the version of the binary, not of the widgets.
    Version,
}

fn main() -> ExitCode {
    match Cli::parse_from(argv()).command {
        Command::Ctl { action } => {
            // Answered before the tailscale check: which parts are installed
            // is a fair question on a machine where the CLI is not.
            if matches!(action, CtlAction::Version) {
                println!("tailgauge-ctl {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            if !tailscale::installed() {
                eprintln!("tailgauge: the tailscale CLI is not on PATH");
                return ExitCode::FAILURE;
            }
            match action {
                CtlAction::Status => settle(ctl::status()),
                CtlAction::Toggle => settle(ctl::toggle()),
                CtlAction::Up => settle(ctl::up()),
                CtlAction::Down => settle(ctl::down()),
                CtlAction::ExitNode { name } => settle(ctl::exit_node(name.as_deref())),
                CtlAction::ExitNodes => settle(ctl::exit_nodes()),
                CtlAction::Version => unreachable!("answered before the CLI check"),
            }
        }

        Command::Watch { timeout_seconds } => {
            ExitCode::from(watch::run(Duration::from_secs(timeout_seconds)).code())
        }

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
