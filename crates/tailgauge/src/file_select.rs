//! `tailgauge file-select` - the desktop's own file chooser.

use crate::launch;

pub enum Picked {
    Files(Vec<String>),
    /// The dialog opened and nothing came back, which is a decision.
    Cancelled,
    NoChooser,
}

pub fn run(title: &str, multiple: bool) -> Picked {
    let kde = std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| {
            let d = d.to_lowercase();
            d.contains("kde") || d.contains("plasma")
        })
        .unwrap_or(false);

    let order: [&str; 2] = if kde {
        ["kdialog", "zenity"]
    } else {
        ["zenity", "kdialog"]
    };

    for chooser in order {
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
        // Both choosers exit non-zero on cancel, which is a decision rather
        // than a fault.
        if !out.status.success() {
            return Picked::Cancelled;
        }
        let files: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        return if files.is_empty() {
            Picked::Cancelled
        } else {
            Picked::Files(files)
        };
    }

    Picked::NoChooser
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
}
