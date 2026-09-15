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

use crate::frontend::{self, Frontend};
use crate::state::{self, UpdateStatus};

/// The binary in the release archive, and the name every alias points at.
const BINARY: &str = "tailgauge";

/// One symlink per subcommand, under the name the shell helper had. A key
/// binding on `tailgauge-ctl toggle` and the Taildrop systemd unit both keep
/// working, and `main` reads the name it was invoked as. Updating is a flag
/// rather than a subcommand, so `tailgauge-update` has nothing to point at.
pub const ALIASES: &[&str] = &[
    "tailgauge-ctl",
    "tailgauge-watch",
    "tailgauge-notify",
    "tailgauge-send",
    "tailgauge-receive",
    "tailgauge-file-select",
    "tailgauge-copy",
];

/// Six hours: long enough that a panel polling this costs nothing, short
/// enough that a release lands the same day. The check itself is always live;
/// this is the caller's answer to how stale a banner may be.
pub const CACHE_TTL_MS: i64 = 6 * 60 * 60 * 1000;

/// Distinguishes the binary archive from the other assets a release carries.
const ARCHIVE_SUFFIX: &str = ".tar.gz";

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ---------------------------------------------------------------------------
// checking
// ---------------------------------------------------------------------------

/// Query GitHub, recompute availability, and persist the cached status. The
/// `notified` guard is preserved across calls.
pub fn check(cache_file: &Path) -> Result<UpdateStatus> {
    let current = current_version().to_string();
    let mut status = state::read_update_status(cache_file).unwrap_or_default();
    status.current = current.clone();
    status.checked_ms = state::now_ms();

    let release = latest_release()?;
    let latest = release.version.clone();
    status.available = version_gt(&latest, &current);
    status.latest = Some(latest);

    state::write_update_status(cache_file, &status)?;
    Ok(status)
}

/// The cached answer while it is fresh, and a live [`check`] otherwise. A panel
/// polls this, so the round trip to GitHub is meant to be the exception.
pub fn check_cached(cache_file: &Path, force: bool) -> Result<UpdateStatus> {
    if !force
        && let Some(cached) = state::read_update_status(cache_file)
        && cached.latest.as_deref().is_some_and(|v| !v.is_empty())
        && state::now_ms().saturating_sub(cached.checked_ms) < CACHE_TTL_MS
    {
        return Ok(cached);
    }
    check(cache_file)
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

/// The newest release carrying an asset for the running platform.
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

fn release_named(version: &str) -> Result<self_update::update::Release> {
    let (owner, name) = repo();
    let releases = ReleaseList::configure()
        .repo_owner(&owner)
        .repo_name(&name)
        .build()?
        .fetch()
        .context("could not reach GitHub to fetch releases")?;
    let wanted = version.trim_start_matches('v');
    releases
        .into_iter()
        .find(|r| r.version.trim_start_matches('v') == wanted)
        .ok_or_else(|| anyhow!("no release v{wanted} to install frontends from"))
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
    pub id: &'static str,
    pub label: &'static str,
    /// The version now on disk, read back from the installed copy rather than
    /// assumed from the release.
    pub version: Option<String>,
    pub restart_hint: &'static str,
    pub needs_session_restart: bool,
    /// Set when the install failed; the binary is already replaced by then, so
    /// this is reported rather than propagated.
    pub error: Option<String>,
}

/// Result of a successful [`apply`]: the version installed, plus what happened
/// to each non-binary frontend that was already present.
pub struct Applied {
    pub version: String,
    pub frontends: Vec<FrontendOutcome>,
}

/// Download the platform archive and replace the installed binary. Returns the
/// version installed - unchanged when already current, so a same-version run
/// never clobbers.
pub fn apply(cache_file: &Path) -> Result<Applied> {
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

    let install_dir = install_dir()?;

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
        fetch_into(&tmp, &asset.name, &asset.download_url)?;

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

        // An archive predating the frontend payloads carries none, and every
        // install would fail with the same "payload not found". Say nothing
        // rather than reporting a failure per frontend for an old release.
        Ok(if present.iter().any(|f| f.payload_in(&tmp).is_some()) {
            install_frontends_from(&tmp, &present)
        } else {
            Vec::new()
        })
    })();

    let _ = std::fs::remove_dir_all(&tmp);
    let frontends = result?;

    // Refresh the cached status so the panel drops the update banner.
    let mut status = state::read_update_status(cache_file).unwrap_or_default();
    status.current = release.version.clone();
    status.latest = Some(release.version.clone());
    status.available = false;
    status.notified = None;
    status.checked_ms = state::now_ms();
    let _ = state::write_update_status(cache_file, &status);

    Ok(Applied {
        version: release.version,
        frontends,
    })
}

/// Download the release matching `version` and install one frontend from it,
/// whether or not it is already present. This is the "switched desktops" path:
/// the payload always comes from the release the running binary belongs to, so
/// the frontend cannot land out of step with it.
pub fn install_frontends(
    targets: &[&'static Frontend],
    version: &str,
) -> Result<Vec<FrontendOutcome>> {
    let release = release_named(version)?;
    let asset = release
        .asset_for(arch_target()?, Some(ARCHIVE_SUFFIX))
        .ok_or_else(|| anyhow!("release {} has no asset for this platform", release.version))?;

    let install_dir = install_dir()?;
    // One lock, one download, one extraction for the whole set: installing
    // three frontends must not fetch the archive three times.
    let _lock = UpdateLock::acquire(&install_dir)?;

    let tmp = install_dir.join(".tg-frontend.tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)
        .with_context(|| format!("cannot create staging dir {}", tmp.display()))?;

    let result = (|| -> Result<Vec<FrontendOutcome>> {
        fetch_into(&tmp, &asset.name, &asset.download_url)?;
        if !targets.iter().any(|t| t.payload_in(&tmp).is_some()) {
            bail!(
                "release v{} ships no frontend payloads",
                version.trim_start_matches('v')
            );
        }
        // Per-frontend failures are collected rather than propagated, so one
        // unwritable destination does not skip the rest of the set.
        Ok(install_frontends_from(&tmp, targets))
    })();

    let _ = std::fs::remove_dir_all(&tmp);
    result
}

fn install_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot resolve current executable")?;
    Ok(exe
        .parent()
        .ok_or_else(|| anyhow!("cannot resolve install directory"))?
        .to_path_buf())
}

fn fetch_into(tmp: &Path, name: &str, url: &str) -> Result<()> {
    let archive = tmp.join(name);
    let file = std::fs::File::create(&archive)
        .with_context(|| format!("cannot create {}", archive.display()))?;
    // GitHub's asset `url` is the API endpoint, which streams the binary only
    // when `Accept: application/octet-stream` is set - otherwise it returns the
    // asset's JSON metadata.
    self_update::Download::from_url(url)
        .set_header(
            http::header::ACCEPT,
            http::HeaderValue::from_static("application/octet-stream"),
        )
        .show_progress(true)
        .download_to(file)
        .context("download failed")?;

    self_update::Extract::from_source(&archive)
        .archive(self_update::ArchiveKind::Tar(Some(
            self_update::Compression::Gz,
        )))
        .extract_into(tmp)
        .context("extract failed")
}

fn install_frontends_from(
    source_root: &Path,
    targets: &[&'static Frontend],
) -> Vec<FrontendOutcome> {
    targets
        .iter()
        .map(|f| match f.install_from(source_root) {
            Ok(_) => FrontendOutcome {
                id: f.id,
                label: f.label,
                version: f.installed_version(),
                restart_hint: f.restart.hint(),
                needs_session_restart: f.restart.needs_session_restart(),
                error: None,
            },
            Err(e) => FrontendOutcome {
                id: f.id,
                label: f.label,
                version: None,
                restart_hint: f.restart.hint(),
                needs_session_restart: f.restart.needs_session_restart(),
                error: Some(format!("{e:#}")),
            },
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
        // What 0.4.0 leaves behind: real bash scripts. Left in place they would
        // answer for their names forever.
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
        // Updating is a flag, so it is deliberately absent from the set.
        let known = [
            "ctl",
            "watch",
            "notify",
            "send",
            "receive",
            "file-select",
            "copy",
        ];
        for alias in ALIASES {
            let sub = alias.strip_prefix("tailgauge-").unwrap();
            assert!(known.contains(&sub), "{alias} has no subcommand");
        }
        assert_eq!(ALIASES.len(), known.len(), "a subcommand has no alias");
        assert!(
            !ALIASES.contains(&"tailgauge-update"),
            "updating is --update, so this alias would resolve to nothing"
        );
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
    fn a_fresh_cache_answers_without_reaching_github() {
        // The panel polls this. A check that always went out would hit the
        // unauthenticated rate limit on a machine that restarts its shell.
        let dir = scratch("cache");
        let cache = dir.join("update.json");
        state::write_update_status(
            &cache,
            &UpdateStatus {
                current: current_version().into(),
                latest: Some("9.9.9".into()),
                available: true,
                checked_ms: state::now_ms(),
                notified: None,
            },
        )
        .unwrap();

        let status = check_cached(&cache, false).expect("served from cache");
        assert_eq!(status.latest.as_deref(), Some("9.9.9"));
        assert!(status.available);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stale_or_empty_cache_is_not_served() {
        let dir = scratch("stale-cache");
        let cache = dir.join("update.json");

        // Older than the TTL.
        state::write_update_status(
            &cache,
            &UpdateStatus {
                latest: Some("9.9.9".into()),
                checked_ms: state::now_ms() - CACHE_TTL_MS - 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            !cache_is_fresh(&cache),
            "a cache past the TTL must be refetched"
        );

        // Checked just now, but carrying no answer.
        state::write_update_status(
            &cache,
            &UpdateStatus {
                latest: None,
                checked_ms: state::now_ms(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            !cache_is_fresh(&cache),
            "a check that failed is not an answer"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The guard `check_cached` applies, without the network call behind it.
    fn cache_is_fresh(cache: &Path) -> bool {
        state::read_update_status(cache).is_some_and(|c| {
            c.latest.as_deref().is_some_and(|v| !v.is_empty())
                && state::now_ms().saturating_sub(c.checked_ms) < CACHE_TTL_MS
        })
    }
}

/// Distribution is a contract spread across four files: the release workflow
/// names the assets, this module downloads them by name, and three manifests
/// declare a version. Nothing at runtime notices when they disagree - the
/// updater just 404s - so it is checked here instead.
#[cfg(test)]
mod distribution {
    use super::*;

    fn repo_file(rel: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .join(rel);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn manifest(rel: &str) -> serde_json::Value {
        serde_json::from_str(&repo_file(rel)).expect("manifest parses")
    }

    const ARCHES: [&str; 2] = ["x86_64", "aarch64"];

    /// Every `dist/tailgauge-$safe-<suffix>` the Package step writes, with the
    /// architecture loop expanded.
    fn published_suffixes() -> Vec<String> {
        let release = repo_file(".github/workflows/release.yml");
        let mut out = Vec::new();
        for (index, _) in release.match_indices("dist/tailgauge-$safe-") {
            let rest = &release[index + "dist/tailgauge-$safe-".len()..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .unwrap_or(rest.len());
            let suffix = &rest[..end];
            if suffix.contains("$arch") {
                for arch in ARCHES {
                    out.push(suffix.replace("$arch", arch));
                }
            } else {
                out.push(suffix.to_string());
            }
        }
        out.sort();
        out.dedup();
        out
    }

    #[test]
    fn the_release_publishes_the_archive_this_binary_downloads() {
        let published = published_suffixes();
        let wanted = format!("{}{ARCHIVE_SUFFIX}", arch_target().expect("known arch"));
        assert!(
            published.iter().any(|s| s == &wanted),
            "apply() asks for {wanted}, and the release publishes {published:?}"
        );
        for arch in ARCHES {
            assert!(
                published
                    .iter()
                    .any(|s| s == &format!("linux-{arch}.tar.gz")),
                "no archive for {arch}: {published:?}"
            );
        }
    }

    #[test]
    fn every_packaged_asset_is_actually_uploaded() {
        let release = repo_file(".github/workflows/release.yml");
        let block = release.split("files: |").nth(1).expect("no files block");
        let globs: Vec<&str> = block
            .lines()
            .map(str::trim)
            .skip_while(|l| l.is_empty())
            .take_while(|l| l.starts_with("dist/"))
            .collect();
        assert!(!globs.is_empty(), "the release uploads nothing");

        for suffix in published_suffixes() {
            let name = format!("dist/tailgauge-v0.0.0-{suffix}");
            let matched = globs.iter().any(|glob| {
                let (head, tail) = glob.split_once('*').expect("a glob");
                name.starts_with(head) && name.ends_with(tail)
            });
            assert!(
                matched,
                "Package builds {name} but no glob matches: {globs:?}"
            );
        }
    }

    #[test]
    fn the_binary_archive_carries_the_binary_and_the_payload_layout() {
        // apply() extracts the archive and looks for exactly these paths; a
        // Package step that stopped writing one of them would download fine
        // and then refuse the update.
        let release = repo_file(".github/workflows/release.yml");
        assert!(release.contains(&format!("\"$stage/{BINARY}\"")));
        for f in frontend::FRONTENDS {
            assert!(
                release.contains(&format!("$stage/{}/{}", frontend::ARCHIVE_ROOT, f.id)),
                "the archive has nowhere to put the {} payload",
                f.id
            );
        }
    }

    #[test]
    fn the_plasmoid_ships_as_a_plasmoid_zip() {
        // kpackagetool6 and the KDE Store both take a zip of the package
        // contents; a tarball installs from neither.
        let published = published_suffixes();
        assert!(
            published.iter().any(|s| s == "plasmoid.plasmoid"),
            "published assets are {published:?}"
        );
        assert!(
            repo_file(".github/workflows/release.yml")
                .contains("cd build/org.tailgauge.plasmoid && zip")
        );
    }

    #[test]
    fn the_omarchy_plugin_ships_under_the_id_its_manifest_declares() {
        // The registry reads the manifest the archive carries, so a tarball
        // that unpacks under any other name installs a plugin the updater
        // cannot find again.
        let plugin = manifest("omarchy/arzaroth.tailgauge/manifest.json");
        let id = plugin["id"].as_str().expect("an id");
        assert!(
            repo_file(".github/workflows/release.yml").contains(&format!(
                "tar -czf \"dist/tailgauge-$safe-omarchy-plugin.tar.gz\" -C build {id}"
            ))
        );
        assert!(
            !id.starts_with("omarchy."),
            "omarchy.* is reserved for first-party plugins"
        );
    }

    #[test]
    fn the_extension_carries_the_integer_version_ego_expects() {
        let extension = manifest("gnome/tailgauge@arzaroth.github.io/metadata.json");
        assert!(
            extension["version"].is_i64(),
            "EGO wants an integer version"
        );
    }

    #[test]
    fn the_updater_points_at_the_repository_the_manifests_name() {
        let slug = repo_slug();
        let plasmoid = manifest("plasma/org.tailgauge.plasmoid/metadata.json");
        let extension = manifest("gnome/tailgauge@arzaroth.github.io/metadata.json");
        for url in [
            plasmoid["KPlugin"]["Website"].as_str().expect("a website"),
            extension["url"].as_str().expect("a url"),
        ] {
            assert!(url.contains(&slug), "{url} does not point at {slug}");
        }
    }

    #[test]
    fn the_release_refuses_a_tag_the_manifests_disagree_with() {
        let release = repo_file(".github/workflows/release.yml");
        assert!(release.contains("tag $REF_NAME does not match the manifests"));
        assert!(
            release.contains("binary=$(sed"),
            "the tag check does not read the binary's own version"
        );
    }
}
