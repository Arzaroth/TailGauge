//! GitHub-release auto-updater.
//!
//! Who owns the copy on disk used to decide whether we may replace it: a
//! store-managed install has its own updater, and racing it leaves the store's
//! registry describing a version that is no longer there. TailGauge is in
//! neither store - not the KDE Store, not extensions.gnome.org - so every copy
//! is ours, and the refusal the shell helper carried is gone.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use self_update::backends::github::ReleaseList;
use serde::Serialize;

use crate::frontend::{self, Frontend};
use crate::notify::{self, Notification};
use crate::state::{self, UpdateStatus};

/// The binary in the release archive, and the names it answers to.
const BINARY: &str = "tailgauge";

/// One symlink per subcommand, under the name the shell helper had. A key
/// binding on `tailgauge-ctl toggle` and the Taildrop systemd unit both keep
/// working, and `main` reads the name it was invoked as.
pub const ALIASES: &[&str] = &[
    "tailgauge-ctl",
    "tailgauge-watch",
    "tailgauge-notify",
    "tailgauge-send",
    "tailgauge-receive",
    "tailgauge-file-select",
    "tailgauge-copy",
    "tailgauge-update",
];

/// Six hours: long enough that opening the panel is free, short enough that a
/// release lands the same day.
const CACHE_TTL_MS: i64 = 6 * 60 * 60 * 1000;

const ARCHIVE_SUFFIX: &str = ".tar.gz";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ---------------------------------------------------------------------------
// what the panel reads
// ---------------------------------------------------------------------------

/// One installed part, in the shape `shared/model.ts` already parses.
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub kind: &'static str,
    pub current: String,
    /// Always `manual` now that neither store is used. The panel still reads
    /// it, so it is reported rather than dropped.
    pub managed: &'static str,
    pub outdated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub available: bool,
    pub updatable: bool,
    pub latest: String,
    pub url: String,
    pub error: String,
    pub targets: Vec<Target>,
}

/// The panel's name for each frontend, which predates the binary and is what
/// `helpersVersion` in the shared model still looks for.
fn target_kind(frontend: &Frontend) -> &'static str {
    match frontend.id {
        "plasma" => "plasmoid",
        "gnome" => "extension",
        _ => "plugin",
    }
}

pub fn report(force: bool) -> Report {
    let cache = state::update_cache_file();
    let (latest, error) = match latest_version(&cache, force) {
        Ok(v) => (v, String::new()),
        Err(e) => (String::new(), format!("{e:#}")),
    };

    let mut targets: Vec<Target> = frontend::installed()
        .into_iter()
        .map(|f| {
            let current = f.installed_version().unwrap_or_default();
            Target {
                kind: target_kind(f),
                outdated: outdated(&latest, &current),
                current,
                managed: "manual",
            }
        })
        .collect();

    // Named `helpers` because that is what it replaced: the shared model reads
    // this entry to report the binary's version beside the widget's.
    targets.push(Target {
        kind: "helpers",
        current: current_version().to_string(),
        managed: "manual",
        outdated: outdated(&latest, current_version()),
    });

    let available = targets.iter().any(|t| t.outdated);
    Report {
        available,
        // Nothing is managed by a store any more, so anything outdated is ours
        // to replace.
        updatable: available || force,
        latest: latest.clone(),
        url: format!("https://github.com/{}/releases/latest", repo_slug()),
        error,
        targets,
    }
}

fn outdated(latest: &str, current: &str) -> bool {
    !latest.is_empty() && !current.is_empty() && version_gt(latest, current)
}

/// The newest release, from the cache when it is fresh enough.
fn latest_version(cache: &Path, force: bool) -> Result<String> {
    if !force
        && let Some(cached) = state::read_update_status(cache)
        && let Some(latest) = cached.latest.filter(|v| !v.is_empty())
        && state::now_ms() - cached.checked_ms < CACHE_TTL_MS
    {
        return Ok(latest);
    }
    Ok(check(cache)?.latest.unwrap_or_default())
}

/// Query GitHub, recompute availability, and persist the cached status.
pub fn check(cache_file: &Path) -> Result<UpdateStatus> {
    let current = current_version().to_string();
    let release = latest_release()?;
    let latest = release.version.clone();
    let status = UpdateStatus {
        available: version_gt(&latest, &current),
        current,
        latest: Some(latest),
        checked_ms: state::now_ms(),
    };
    let _ = state::write_update_status(cache_file, &status);
    Ok(status)
}

// ---------------------------------------------------------------------------
// releases
// ---------------------------------------------------------------------------

/// `owner/repo` to pull releases from, so a fork can self-update from its own.
fn repo_slug() -> String {
    std::env::var("TAILGAUGE_REPO").unwrap_or_else(|_| "Arzaroth/TailGauge".into())
}

fn repo() -> (String, String) {
    let slug = repo_slug();
    match slug.split_once('/') {
        Some((o, r)) => (o.to_string(), r.to_string()),
        None => ("Arzaroth".into(), "TailGauge".into()),
    }
}

/// Substring the release asset name must contain for the running platform.
/// Matches the release workflow's `tailgauge-<tag>-<target>.tar.gz` naming.
fn arch_target() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("linux-x86_64"),
        "aarch64" | "arm64" => Ok("linux-aarch64"),
        other => bail!("unsupported arch: {other}"),
    }
}

/// True if dotted version `a` is greater than `b`. A leading `v` and any
/// pre-release suffix are ignored.
pub fn version_gt(a: &str, b: &str) -> bool {
    fn parts(v: &str) -> [u64; 3] {
        let v = v.trim().trim_start_matches(['v', 'V']);
        let core = v.split(['-', '+']).next().unwrap_or(v);
        let mut out = [0u64; 3];
        for (i, seg) in core.split('.').take(3).enumerate() {
            out[i] = seg.parse().unwrap_or(0);
        }
        out
    }
    parts(a) > parts(b)
}

fn latest_release() -> Result<self_update::update::Release> {
    let (owner, name) = repo();
    let target = arch_target()?;
    let releases = ReleaseList::configure()
        .repo_owner(&owner)
        .repo_name(&name)
        .build()?
        .fetch()
        .context("could not reach GitHub to check for updates")?;
    releases
        .into_iter()
        .find(|r| r.asset_for(target, Some(ARCHIVE_SUFFIX)).is_some())
        .ok_or_else(|| anyhow!("no release with a {target} asset found"))
}

// ---------------------------------------------------------------------------
// applying
// ---------------------------------------------------------------------------

/// Exclusive lock, so an update fired from the panel and one fired from a
/// terminal cannot race on the shared staging directory.
struct UpdateLock(PathBuf);

impl UpdateLock {
    fn acquire(install_dir: &Path) -> Result<Self> {
        let path = install_dir.join(".tg-update.lock");
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => Ok(UpdateLock(path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => bail!(
                "update already in progress ({} exists; remove it if stale)",
                path.display()
            ),
            Err(e) => Err(e).context("failed to acquire update lock"),
        }
    }
}

impl Drop for UpdateLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// What an update did to a non-binary frontend, for the caller to report.
#[derive(Debug, Clone)]
pub struct FrontendOutcome {
    pub label: &'static str,
    pub restart_hint: &'static str,
    pub needs_session_restart: bool,
    /// Set when the install failed; the binary is already replaced by then, so
    /// this is reported rather than propagated.
    pub error: Option<String>,
}

pub struct Applied {
    pub version: String,
    pub frontends: Vec<FrontendOutcome>,
}

pub fn apply() -> Result<Applied> {
    let target = arch_target()?;
    let release = latest_release()?;
    let current = current_version();
    if !version_gt(&release.version, current) {
        return Ok(Applied {
            version: current.to_string(),
            frontends: Vec::new(),
        });
    }

    let asset = release
        .asset_for(target, Some(ARCHIVE_SUFFIX))
        .ok_or_else(|| anyhow!("release {} has no {target} asset", release.version))?;

    let exe = std::env::current_exe().context("cannot resolve current executable")?;
    let install_dir = exe
        .parent()
        .ok_or_else(|| anyhow!("cannot resolve install directory"))?
        .to_path_buf();

    // Held for the whole download/extract/replace, so a second invocation
    // fails fast instead of corrupting the staging directory.
    let _lock = UpdateLock::acquire(&install_dir)?;

    // Staged inside the install directory so the final move is a rename on the
    // same filesystem.
    let tmp = install_dir.join(".tg-update.tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)
        .with_context(|| format!("cannot create staging dir {}", tmp.display()))?;

    // Only what is already installed: this refreshes an existing frontend, it
    // does not decide that a machine should grow a GNOME extension.
    let present = frontend::installed();

    let result = (|| -> Result<Vec<FrontendOutcome>> {
        let archive = tmp.join(&asset.name);
        let file = std::fs::File::create(&archive)
            .with_context(|| format!("cannot create {}", archive.display()))?;
        // GitHub's asset `url` is the API endpoint, which streams the binary
        // only when `Accept: application/octet-stream` is set - otherwise it
        // returns the asset's JSON metadata.
        self_update::Download::from_url(&asset.download_url)
            .set_header(
                http::header::ACCEPT,
                http::HeaderValue::from_static("application/octet-stream"),
            )
            .download_to(file)
            .context("download failed")?;

        self_update::Extract::from_source(&archive)
            .archive(self_update::ArchiveKind::Tar(Some(
                self_update::Compression::Gz,
            )))
            .extract_into(&tmp)
            .context("extract failed")?;

        // `is_file`, not `exists`: `Move::to_dest` renames without checking
        // what it is moving, so a directory of that name in the archive would
        // land on the installed binary's path and become what every alias
        // points at.
        let staged_binary = tmp.join(BINARY);
        if !staged_binary.is_file() {
            bail!("release archive has no {BINARY} - refusing a partial update");
        }

        let dest = install_dir.join(BINARY);
        // Move-with-temp so the running binary is replaced safely: the old
        // inode stays live for this process.
        self_update::Move::from_source(&staged_binary)
            .replace_using_temp(&tmp.join("tailgauge.old"))
            .to_dest(&dest)
            .with_context(|| format!("failed to replace {}", dest.display()))?;
        if let Ok(meta) = std::fs::metadata(&dest) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&dest, perms);
        }

        refresh_aliases(&install_dir);

        Ok(install_frontends_from(&tmp, &present))
    })();

    let _ = std::fs::remove_dir_all(&tmp);
    let frontends = result?;

    // The panel drops the update prompt on the next check rather than after
    // the next TTL.
    let cache = state::update_cache_file();
    let _ = state::write_update_status(
        &cache,
        &UpdateStatus {
            current: release.version.clone(),
            latest: Some(release.version.clone()),
            available: false,
            checked_ms: state::now_ms(),
        },
    );

    Ok(Applied {
        version: release.version,
        frontends,
    })
}

fn install_frontends_from(
    source_root: &Path,
    targets: &[&'static Frontend],
) -> Vec<FrontendOutcome> {
    targets
        .iter()
        .map(|f| FrontendOutcome {
            label: f.label,
            restart_hint: f.restart.hint(),
            needs_session_restart: f.restart.needs_session_restart(),
            error: f.install_from(source_root).err().map(|e| format!("{e:#}")),
        })
        .collect()
}

/// Point every old helper name at the binary, replacing whatever is there - on
/// an upgrade from the shell helpers that is a real script, and leaving it
/// would let a stale copy answer for `tailgauge-ctl` forever.
pub fn refresh_aliases(install_dir: &Path) {
    // Never trade a working helper for a link to nothing.
    if !install_dir.join(BINARY).is_file() {
        return;
    }
    for alias in ALIASES {
        let path = install_dir.join(alias);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink())
            && std::fs::read_link(&path).is_ok_and(|t| t == Path::new(BINARY))
        {
            continue;
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::os::unix::fs::symlink(BINARY, &path);
    }
}

pub fn announce(applied: &Applied) {
    let failed: Vec<&str> = applied
        .frontends
        .iter()
        .filter(|f| f.error.is_some())
        .map(|f| f.label)
        .collect();
    let body = if failed.is_empty() {
        "Restart the shell to load it".to_string()
    } else {
        format!("{} could not be replaced", failed.join(", "))
    };
    let _ = notify::run(&Notification {
        summary: &format!("TailGauge updated to {}", applied.version),
        body: &body,
        urgency: if failed.is_empty() {
            "normal"
        } else {
            "critical"
        },
        image: None,
        open: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tg-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_newer_version_is_newer_however_it_is_spelled() {
        assert!(version_gt("0.5.0", "0.4.9"));
        assert!(version_gt("v1.0.0", "0.31.0"));
        assert!(version_gt("0.10.0", "0.9.0"), "0.10 is not 0.1");
        assert!(!version_gt("0.4.0", "0.4.0"));
        assert!(!version_gt("0.4.0", "0.5.0"));
        assert!(
            !version_gt("0.5.0-rc1", "0.5.0"),
            "a pre-release is the release"
        );
    }

    #[test]
    fn every_old_helper_name_becomes_a_link_to_the_binary() {
        let dir = scratch("aliases");
        std::fs::write(dir.join(BINARY), b"binary").unwrap();
        refresh_aliases(&dir);

        for alias in ALIASES {
            let path = dir.join(alias);
            assert_eq!(
                std::fs::read_link(&path).unwrap(),
                Path::new(BINARY),
                "{alias}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"binary", "{alias} resolves");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_upgrade_from_the_shell_helpers_replaces_the_scripts() {
        // What 0.4.0 leaves behind: eight real bash scripts. Left in place
        // they would answer for their names forever.
        let dir = scratch("stale");
        std::fs::write(dir.join(BINARY), b"new").unwrap();
        std::fs::write(dir.join("tailgauge-ctl"), b"#!/bin/bash\n").unwrap();
        refresh_aliases(&dir);

        let alias = dir.join("tailgauge-ctl");
        assert!(
            std::fs::symlink_metadata(&alias)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&alias).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_binary_leaves_the_helpers_alone() {
        // The half-extracted archive case: replacing a working helper with a
        // link to a file that is not there is worse than doing nothing.
        let dir = scratch("dangling");
        std::fs::write(dir.join("tailgauge-ctl"), b"#!/bin/bash\n").unwrap();
        refresh_aliases(&dir);

        let alias = dir.join("tailgauge-ctl");
        assert!(
            !std::fs::symlink_metadata(&alias)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&alias).unwrap(), b"#!/bin/bash\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn refreshing_an_existing_set_of_aliases_is_a_no_op() {
        let dir = scratch("idempotent");
        std::fs::write(dir.join(BINARY), b"binary").unwrap();
        refresh_aliases(&dir);
        refresh_aliases(&dir);
        assert_eq!(
            std::fs::read_link(dir.join("tailgauge-ctl")).unwrap(),
            Path::new(BINARY)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_alias_names_a_subcommand_the_binary_has() {
        // An alias the parser does not answer to is a helper that vanished.
        let known = [
            "ctl",
            "watch",
            "notify",
            "send",
            "receive",
            "file-select",
            "copy",
            "update",
        ];
        for alias in ALIASES {
            let sub = alias.strip_prefix("tailgauge-").unwrap();
            assert!(known.contains(&sub), "{alias} has no subcommand");
        }
        assert_eq!(ALIASES.len(), known.len(), "a subcommand has no alias");
    }

    #[test]
    fn the_lock_is_exclusive_and_released_on_the_way_out() {
        let dir = scratch("lock");
        {
            let _held = UpdateLock::acquire(&dir).expect("first");
            assert!(
                UpdateLock::acquire(&dir).is_err(),
                "two updates must not run"
            );
        }
        UpdateLock::acquire(&dir).expect("released on drop");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_target_is_only_outdated_against_a_version_we_know() {
        assert!(outdated("0.5.0", "0.4.0"));
        assert!(
            !outdated("", "0.4.0"),
            "GitHub unreachable is not an update"
        );
        assert!(
            !outdated("0.5.0", ""),
            "an unreadable manifest is not outdated"
        );
    }

    #[test]
    fn the_panel_still_finds_the_version_it_reads_beside_the_widget() {
        // `helpersVersion` in shared/model.ts looks for exactly this kind, and
        // the footer goes quiet if it is renamed.
        let report = Report {
            available: false,
            updatable: false,
            latest: String::new(),
            url: String::new(),
            error: String::new(),
            targets: vec![Target {
                kind: "helpers",
                current: current_version().into(),
                managed: "manual",
                outdated: false,
            }],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["targets"][0]["kind"], "helpers");
        for field in [
            "available",
            "updatable",
            "latest",
            "url",
            "error",
            "targets",
        ] {
            assert!(json.get(field).is_some(), "the panel reads {field}");
        }
    }
}
