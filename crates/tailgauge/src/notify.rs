//! `tailgauge notify` - a desktop notification, optionally openable.

use anyhow::Result;
use std::process::{Command, Stdio};

use crate::launch;

pub struct Notification<'a> {
    pub summary: &'a str,
    pub body: &'a str,
    pub urgency: &'a str,
    pub image: Option<&'a str>,
    pub open: Option<&'a str>,
}

impl Notification<'_> {
    fn base_args(&self) -> Vec<String> {
        let mut args = vec![
            "--app-name=TailGauge".to_string(),
            format!("--urgency={}", self.urgency),
            "--icon=network-vpn-symbolic".to_string(),
        ];
        if let Some(image) = self.image {
            args.push(format!("--hint=string:image-path:file://{image}"));
        }
        args
    }

    /// `--` before the summary, because a body that starts with a dash is a
    /// message and not an option.
    fn post_args(&self) -> Vec<String> {
        let mut args = self.base_args();
        args.push("--".into());
        args.push(self.summary.into());
        args.push(self.body.into());
        args
    }

    fn wait_args(&self) -> Vec<String> {
        let mut args = self.base_args();
        args.push("--wait".into());
        args.push("--action=default=Open".into());
        args.push("--".into());
        args.push(self.summary.into());
        args.push(self.body.into());
        args
    }

    /// The argv the detached copy is re-executed with. It has to parse back
    /// into this same notification, so the two are written together.
    fn relaunch_args(&self, path: &str) -> Vec<String> {
        let mut args = vec![
            "notify".to_string(),
            "--await-action".into(),
            "--urgency".into(),
            self.urgency.into(),
            "--open".into(),
            path.into(),
        ];
        if let Some(image) = self.image {
            args.push("--image".into());
            args.push(image.into());
        }
        args.push("--".into());
        args.push(self.summary.into());
        args.push(self.body.into());
        args
    }
}

/// Post the notification. A machine with no `notify-send` is not a failure:
/// every caller is reporting something it has already done.
pub fn run(n: &Notification<'_>) -> Result<()> {
    if !launch::has("notify-send") {
        return Ok(());
    }

    if let Some(path) = n.open
        && supports_actions()
        && spawn_actionable(n, path).is_ok()
    {
        return Ok(());
    }

    let _ = launch::run_quiet("notify-send", n.post_args());
    Ok(())
}

fn supports_actions() -> bool {
    launch::run("notify-send", ["--help"]).is_ok_and(|out| {
        let text = String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr);
        text.contains("--action")
    })
}

/// `--wait` blocks until the notification is dismissed or actioned, so whoever
/// waits on it cannot be the process that has a panel to get back to. Re-exec
/// ourselves detached and let that copy do the waiting.
fn spawn_actionable(n: &Notification<'_>, path: &str) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);
    cmd.args(n.relaunch_args(path));
    detached(&mut cmd).spawn().map(|_| ())
}

/// The blocking half of [`spawn_actionable`], running in the detached copy.
pub fn await_action(n: &Notification<'_>, path: &str) -> Result<()> {
    let chosen = launch::output("notify-send", n.wait_args()).unwrap_or_default();
    if chosen.trim() == "default" {
        let _ = detached(Command::new("xdg-open").arg(path)).spawn();
    }
    Ok(())
}

fn detached(cmd: &mut Command) -> &mut Command {
    let cmd = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn notification() -> Notification<'static> {
        Notification {
            summary: "Received shot.png",
            body: "Saved to ~/Downloads",
            urgency: "normal",
            image: Some("/home/me/Downloads/shot.png"),
            open: Some("/home/me/Downloads/shot.png"),
        }
    }

    #[test]
    fn the_message_is_never_read_as_options() {
        // A body that starts with a dash is a message: notify-send would take
        // "--wait" from a peer's name as its own flag otherwise.
        let mut n = notification();
        n.summary = "--wait";
        n.body = "-u critical";
        let args = n.post_args();
        let sep = args.iter().position(|a| a == "--").expect("a separator");
        assert_eq!(&args[sep + 1..], ["--wait", "-u critical"]);
    }

    #[test]
    fn an_image_is_a_file_url_and_is_absent_when_there_is_none() {
        assert!(
            notification()
                .base_args()
                .contains(&"--hint=string:image-path:file:///home/me/Downloads/shot.png".into())
        );
        let mut n = notification();
        n.image = None;
        assert!(!n.base_args().iter().any(|a| a.contains("image-path")));
    }

    #[test]
    fn only_the_waiting_copy_asks_for_an_action() {
        // `--wait` blocks until the notification is dismissed, so the copy that
        // has a panel to get back to must not pass it.
        assert!(!notification().post_args().iter().any(|a| a == "--wait"));
        let waiting = notification().wait_args();
        assert!(waiting.contains(&"--wait".to_string()));
        assert!(waiting.contains(&"--action=default=Open".to_string()));
    }

    /// The detached copy is this same binary, so what it is re-executed with
    /// has to parse back into the notification it came from.
    #[test]
    fn the_detached_copy_is_handed_the_same_notification() {
        let n = notification();
        let path = "/home/me/Downloads/shot.png";
        let mut argv = vec!["tailgauge".to_string()];
        argv.extend(n.relaunch_args(path));

        let cli = crate::Cli::parse_from(argv);
        let Some(crate::Command::Notify {
            image,
            open,
            urgency,
            await_action,
            summary,
            body,
        }) = cli.command
        else {
            panic!("the relaunch argv is not a notify command")
        };
        assert!(await_action, "or the copy would post and exit at once");
        assert_eq!(summary, n.summary);
        assert_eq!(body, n.body);
        assert_eq!(urgency, n.urgency);
        assert_eq!(image.as_deref(), n.image);
        assert_eq!(open.as_deref(), Some(path));
    }

    #[test]
    fn a_notification_with_nothing_to_open_is_still_relaunchable() {
        let mut n = notification();
        n.image = None;
        let mut argv = vec!["tailgauge".to_string()];
        argv.extend(n.relaunch_args("/tmp/x"));
        assert!(crate::Cli::try_parse_from(argv).is_ok());
    }
}
