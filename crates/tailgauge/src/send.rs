//! `tailgauge send` - files to a tailnet machine, over Taildrop.

use std::path::Path;

use crate::file_select::{self, Picked};
use crate::notify::{self, Notification};
use selvedge::proc;

pub enum Outcome {
    Sent,
    /// Nobody picked anything, which is not a failure.
    NothingToSend,
    Failed,
}

pub fn run(machine: &str, files: &[String]) -> Outcome {
    // Address the machine by whatever name we were handed, but talk about it
    // by its short name, so a MagicDNS name does not spill into every message.
    let name = machine.split('.').next().unwrap_or(machine);

    let files = if files.is_empty() {
        match file_select::run(&format!("Send to {name}"), true) {
            Picked::Files(picked) => picked,
            Picked::Cancelled => return Outcome::NothingToSend,
            Picked::NoChooser => {
                announce(
                    "critical",
                    &format!("Could not send to {name}"),
                    "The file chooser did not open",
                );
                return Outcome::Failed;
            }
        }
    } else {
        files.to_vec()
    };

    if files.is_empty() {
        return Outcome::NothingToSend;
    }

    let what = describe(&files);
    match proc::run("tailscale", taildrop_args(machine, &files)) {
        Ok(out) if out.status.success() => {
            announce("normal", &format!("Sent to {name}"), &what);
            Outcome::Sent
        }
        Ok(out) => {
            let why = why_failed(&out.stdout, &out.stderr);
            announce("critical", &format!("Could not send to {name}"), &why);
            Outcome::Failed
        }
        Err(e) => {
            announce(
                "critical",
                &format!("Could not send to {name}"),
                &e.to_string(),
            );
            Outcome::Failed
        }
    }
}

/// `--` before the files, or one named `-x` is read as an option. The trailing
/// colon is what makes the last argument a destination rather than a file.
fn taildrop_args(machine: &str, files: &[String]) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "file".into(),
        "cp".into(),
        "--update-interval=0".into(),
        "--".into(),
    ];
    args.extend(files.iter().cloned());
    args.push(format!("{machine}:"));
    args
}

/// The CLI explains itself on either stream depending on what went wrong, and
/// a notification that says nothing is worse than one that guesses.
fn why_failed(stdout: &[u8], stderr: &[u8]) -> String {
    for stream in [stderr, stdout] {
        let text = String::from_utf8_lossy(stream).trim().to_string();
        if !text.is_empty() {
            return text;
        }
    }
    "Taildrop transfer failed".into()
}

fn describe(files: &[String]) -> String {
    match files {
        [one] => Path::new(one)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| one.clone()),
        many => format!("{} files", many.len()),
    }
}

fn announce(urgency: &str, summary: &str, body: &str) {
    let _ = notify::run(&Notification {
        summary,
        body,
        urgency,
        image: None,
        open: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_file_is_named_and_several_are_counted() {
        assert_eq!(describe(&["/home/me/notes.md".into()]), "notes.md");
        assert_eq!(describe(&["a".into(), "b".into(), "c".into()]), "3 files");
    }

    #[test]
    fn a_file_named_like_a_flag_is_still_a_file() {
        let args = taildrop_args("box", &["-n".into(), "notes.md".into()]);
        let sep = args.iter().position(|a| a == "--").expect("a separator");
        assert_eq!(&args[sep + 1..], ["-n", "notes.md", "box:"]);
    }

    #[test]
    fn the_destination_is_the_machine_it_was_addressed_by() {
        // The trailing colon is what makes the last argument a destination and
        // not one more file, and the full name is what reaches the machine.
        let args = taildrop_args("box.tail.ts.net", &["notes.md".into()]);
        assert_eq!(args.last().unwrap(), "box.tail.ts.net:");
        assert!(args.contains(&"--update-interval=0".to_string()));
    }

    #[test]
    fn a_failure_is_explained_from_whichever_stream_said_something() {
        assert_eq!(why_failed(b"", b"refused by peer\n"), "refused by peer");
        assert_eq!(why_failed(b"no such file\n", b""), "no such file");
        assert_eq!(
            why_failed(b"progress\n", b"refused\n"),
            "refused",
            "the complaint wins over the transcript"
        );
        assert_eq!(why_failed(b"", b"  \n"), "Taildrop transfer failed");
    }
}
