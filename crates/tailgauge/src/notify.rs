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
