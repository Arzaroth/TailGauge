//! `tailgauge copy` - text to the clipboard, on Wayland or X11.

use anyhow::{Result, bail};
use std::io::Write;
use std::process::{Command, Stdio};

use selvedge::proc;

pub fn run(text: &str) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }

    let on_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());

    for (program, args) in candidates(on_wayland) {
        if !proc::has(program) {
            continue;
        }
        // A tool that is installed can still fail: xclip on a session with no
        // DISPLAY exits at once, and the write to it then fails with EPIPE.
        // That is a reason to try the next one, not to give up.
        if pipe_into(program, args, text).unwrap_or(false) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wl_copy_is_tried_first_only_under_wayland() {
        // On an X11 session with XWayland tooling installed, wl-copy writes to
        // a selection nothing on screen is reading: the paste silently does
        // nothing.
        let names = |wayland| -> Vec<&'static str> {
            candidates(wayland).into_iter().map(|(p, _)| p).collect()
        };
        assert_eq!(names(true), ["wl-copy", "xclip", "xsel"]);
        assert_eq!(names(false), ["xclip", "xsel", "wl-copy"]);
    }

    #[test]
    fn each_tool_is_asked_for_the_clipboard_rather_than_the_primary_selection() {
        // The default for both X11 tools is the middle-click selection, which
        // is not what a Copy row means.
        let args = |name: &str| -> Vec<&'static str> {
            candidates(false)
                .into_iter()
                .find(|(p, _)| *p == name)
                .map(|(_, a)| a.to_vec())
                .expect(name)
        };
        assert_eq!(args("xclip"), ["-selection", "clipboard"]);
        assert_eq!(args("xsel"), ["--clipboard", "--input"]);
        assert!(args("wl-copy").is_empty());
    }
}
