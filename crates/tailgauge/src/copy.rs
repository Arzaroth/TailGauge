//! `tailgauge copy` - text to the clipboard, on Wayland or X11.

use anyhow::{Result, bail};
use std::io::Write;
use std::process::{Command, Stdio};

use crate::launch;

pub fn run(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    let on_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());

    for (program, args) in candidates(on_wayland) {
        if !launch::has(program) {
            continue;
        }
        if pipe_into(program, args, text)? {
            return Ok(());
        }
    }

    bail!("install wl-clipboard, xclip, or xsel")
}

/// wl-copy first under Wayland and last everywhere else: on an X11 session
/// with XWayland tooling installed it is the one that writes to a selection
/// nothing on screen is reading.
fn candidates(on_wayland: bool) -> Vec<(&'static str, &'static [&'static str])> {
    let mut candidates: Vec<(&str, &[&str])> = vec![
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
        ("wl-copy", &[]),
    ];
    if on_wayland {
        candidates.rotate_right(1);
    }
    candidates
}

fn pipe_into(program: &str, args: &[&str], text: &str) -> Result<bool> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(text.as_bytes())?;
    Ok(child.wait()?.success())
}
