//! `tailgauge receive` - incoming Taildrop files, saved and announced.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::launch;
use crate::notify::{self, Notification};

/// Taildrop lands in a staging directory next door rather than straight in the
/// downloads directory: waiting for a delivery can take hours, and everything
/// else that shows up meanwhile is somebody else's file. Same filesystem, so
/// handing the finished file over is a rename.
const STAGING: &str = ".tailgauge-taildrop";

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "tif", "tiff",
];

pub fn run(dir: &Path, once: bool) -> anyhow::Result<()> {
    let staging = dir.join(STAGING);
    std::fs::create_dir_all(&staging)?;

    // Anything left staged by an interrupted run still deserves delivering.
    deliver(dir, &staging);

    loop {
        let got = launch::run_quiet(
            "tailscale",
            [
                "file".as_ref(),
                "get".as_ref(),
                "--wait".as_ref(),
                "--conflict=rename".as_ref(),
                staging.as_os_str(),
            ],
        );
        if !got {
            if once {
                anyhow::bail!("tailscale file get failed");
            }
            thread::sleep(Duration::from_secs(10));
            continue;
        }

        deliver(dir, &staging);
        if once {
            return Ok(());
        }
    }
}

fn deliver(dir: &Path, staging: &Path) {
    let Ok(entries) = std::fs::read_dir(staging) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        if let Some(target) = claim_path(dir, &entry.path()) {
            announce(dir, &target);
        }
    }
}

/// Take the name by linking to it rather than by looking and then renaming.
/// `link(2)` refuses an existing name, so nothing can land on the chosen one in
/// the gap between the two. Staging shares the filesystem with the downloads
/// directory, so the link always resolves and unlinking the staged name
/// finishes the move.
fn claim_path(dir: &Path, staged: &Path) -> Option<PathBuf> {
    let name = staged.file_name()?.to_string_lossy().into_owned();
    let (base, ext) = split_name(&name);

    for index in 0..1000 {
        let candidate = if index == 0 {
            dir.join(&name)
        } else {
            dir.join(format!("{base}-{index}{ext}"))
        };

        if std::fs::hard_link(staged, &candidate).is_ok() {
            let _ = std::fs::remove_file(staged);
            return Some(candidate);
        }

        // Only a taken name is worth another spin. Anything else failed the
        // link itself, and the file keeps its place in staging for the next run.
        if !candidate.exists() {
            return None;
        }
    }
    None
}

/// `notes.tar.gz` numbers as `notes.tar-1.gz`, the way the shell helper did:
/// only the final extension is kept off the counter.
fn split_name(name: &str) -> (&str, &str) {
    match name.rsplit_once('.') {
        Some((base, _)) if !base.is_empty() => (base, &name[base.len()..]),
        _ => (name, ""),
    }
}

fn announce(dir: &Path, path: &Path) {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let is_image = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.as_str()));
    let shown = path.to_string_lossy();

    // Announcing is best-effort: the file is already delivered, and a
    // notification outage must not stop a long-running receiver.
    let _ = notify::run(&Notification {
        summary: &format!("Received {name}"),
        body: &format!("Saved to {}", tildify(dir)),
        urgency: "normal",
        image: is_image.then_some(&shown),
        open: Some(&shown),
    });
}

fn tildify(dir: &Path) -> String {
    let shown = dir.to_string_lossy().into_owned();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && shown.starts_with(&home) => {
            format!("~{}", &shown[home.len()..])
        }
        _ => shown,
    }
}

/// Where Taildrop files land when the caller names nowhere.
pub fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_DOWNLOAD_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join("Downloads")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tg-receive-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn only_the_last_extension_is_kept_off_the_counter() {
        assert_eq!(split_name("notes.md"), ("notes", ".md"));
        assert_eq!(split_name("notes.tar.gz"), ("notes.tar", ".gz"));
        assert_eq!(split_name("README"), ("README", ""));
        assert_eq!(split_name(".bashrc"), (".bashrc", ""));
    }

    #[test]
    fn a_taken_name_is_numbered_rather_than_overwritten() {
        let dir = scratch("claim");
        let staging = dir.join(STAGING);
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(dir.join("shot.png"), b"first").unwrap();

        let staged = staging.join("shot.png");
        std::fs::write(&staged, b"second").unwrap();
        let landed = claim_path(&dir, &staged).unwrap();

        assert_eq!(landed, dir.join("shot-1.png"));
        assert_eq!(std::fs::read(dir.join("shot.png")).unwrap(), b"first");
        assert_eq!(std::fs::read(&landed).unwrap(), b"second");
        assert!(
            !staged.exists(),
            "the staged copy is unlinked once it lands"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_free_name_is_taken_as_it_stands() {
        let dir = scratch("free");
        let staging = dir.join(STAGING);
        std::fs::create_dir_all(&staging).unwrap();
        let staged = staging.join("notes.md");
        std::fs::write(&staged, b"body").unwrap();

        assert_eq!(claim_path(&dir, &staged).unwrap(), dir.join("notes.md"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_downloads_directory_is_shortened_for_the_notification() {
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            return;
        }
        assert_eq!(
            tildify(&PathBuf::from(&home).join("Downloads")),
            "~/Downloads"
        );
        assert_eq!(tildify(Path::new("/srv/drop")), "/srv/drop");
    }
}
