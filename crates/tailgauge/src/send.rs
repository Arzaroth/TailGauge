//! `tailgauge send` - files to a tailnet machine, over Taildrop.

use std::path::Path;

use crate::file_select::{self, Picked};
use crate::launch;
use crate::notify::{self, Notification};

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
    let mut args: Vec<String> = vec![
        "file".into(),
        "cp".into(),
        "--update-interval=0".into(),
        "--".into(),
    ];
    args.extend(files.iter().cloned());
    args.push(format!("{machine}:"));

    match launch::run("tailscale", &args) {
        Ok(out) if out.status.success() => {
            announce("normal", &format!("Sent to {name}"), &what);
            Outcome::Sent
        }
        Ok(out) => {
            let mut why = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if why.is_empty() {
                why = String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
            if why.is_empty() {
                why = "Taildrop transfer failed".into();
            }
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
}
