//! `tailgauge file-select` - the desktop's own file chooser.

use crate::launch;

pub enum Picked {
    Files(Vec<String>),
    /// The dialog opened and nothing came back, which is a decision.
    Cancelled,
    NoChooser,
}

pub fn run(title: &str, multiple: bool) -> Picked {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();

    for chooser in order(&desktop) {
        if !launch::has(chooser) {
            continue;
        }
        let args = match chooser {
            "kdialog" => kdialog_args(title, multiple),
            _ => zenity_args(title, multiple),
        };
        let Ok(out) = launch::run(chooser, &args) else {
            continue;
        };
        return picked(out.status.success(), &out.stdout);
    }

    Picked::NoChooser
}

/// A Plasma session gets kdialog, because zenity there draws a GTK dialog over
/// a Qt desktop. Anywhere else zenity is the one more likely to be installed.
fn order(desktop: &str) -> [&'static str; 2] {
    let desktop = desktop.to_lowercase();
    if desktop.contains("kde") || desktop.contains("plasma") {
        ["kdialog", "zenity"]
    } else {
        ["zenity", "kdialog"]
    }
}

/// Both choosers exit non-zero on cancel, which is a decision rather than a
/// fault.
fn picked(ok: bool, stdout: &[u8]) -> Picked {
    if !ok {
        return Picked::Cancelled;
    }
    let files: Vec<String> = String::from_utf8_lossy(stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    if files.is_empty() {
        Picked::Cancelled
    } else {
        Picked::Files(files)
    }
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".into())
}

fn kdialog_args(title: &str, multiple: bool) -> Vec<String> {
    let mut args = vec!["--title".to_string(), title.to_string()];
    if multiple {
        args.push("--multiple".into());
        args.push("--separate-output".into());
    }
    args.push("--getopenfilename".into());
    args.push(home());
    args
}

fn zenity_args(title: &str, multiple: bool) -> Vec<String> {
    let mut args = vec!["--file-selection".to_string(), format!("--title={title}")];
    if multiple {
        args.push("--multiple".into());
        args.push("--separator=\n".into());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_multiple_selection_asks_each_chooser_for_one_path_per_line() {
        // Both default to a separator this cannot split on: kdialog joins with
        // spaces, zenity with a pipe, and a path may contain either.
        assert!(
            kdialog_args("Send", true)
                .iter()
                .any(|a| a == "--separate-output")
        );
        assert!(
            zenity_args("Send", true)
                .iter()
                .any(|a| a == "--separator=\n")
        );

        assert!(
            !kdialog_args("Send", false)
                .iter()
                .any(|a| a == "--multiple")
        );
        assert!(!zenity_args("Send", false).iter().any(|a| a == "--multiple"));
    }

    #[test]
    fn kdialog_takes_its_title_before_the_mode() {
        // `--getopenfilename` consumes the positional that follows it, so an
        // option placed after it is read as the starting directory.
        let args = kdialog_args("Send to box", true);
        let mode = args.iter().position(|a| a == "--getopenfilename").unwrap();
        assert!(args.iter().position(|a| a == "--title").unwrap() < mode);
        assert_eq!(args.last().unwrap(), &home());
    }

    #[test]
    fn a_plasma_session_gets_the_qt_chooser_and_everything_else_gets_zenity() {
        assert_eq!(order("KDE"), ["kdialog", "zenity"]);
        assert_eq!(order("plasma"), ["kdialog", "zenity"]);
        assert_eq!(order("KDE:plasmawayland"), ["kdialog", "zenity"]);
        assert_eq!(order("GNOME"), ["zenity", "kdialog"]);
        assert_eq!(order(""), ["zenity", "kdialog"]);
    }

    #[test]
    fn cancelling_is_an_answer_rather_than_a_fault() {
        // Both choosers exit non-zero on cancel. Reading that as a fault would
        // put "the file chooser did not open" on screen every time somebody
        // changed their mind.
        assert!(matches!(
            picked(false, b"/home/me/notes.md\n"),
            Picked::Cancelled
        ));
        assert!(matches!(picked(true, b""), Picked::Cancelled));
        assert!(matches!(picked(true, b"\n  \n"), Picked::Cancelled));
    }

    #[test]
    fn one_path_per_line_and_nothing_else() {
        let Picked::Files(files) = picked(true, b"/home/me/a b.md\n/home/me/c.png\n") else {
            panic!("nothing was picked")
        };
        // A path with a space in it is one file: this is why both choosers are
        // asked for a newline separator rather than their defaults.
        assert_eq!(files, ["/home/me/a b.md", "/home/me/c.png"]);
    }
}
