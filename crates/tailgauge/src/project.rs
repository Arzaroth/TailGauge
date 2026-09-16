//! What selvedge cannot know: which binary this is, where its releases live,
//! and what it ships beside itself.

use selvedge::{Frontend, Project, Restart, VersionSource};

pub const TAILGAUGE: Project = Project {
    binaries: &["tailgauge"],
    repo: "Arzaroth/TailGauge",
    // So a fork can self-update from its own releases.
    repo_env: "TAILGAUGE_REPO",
    // This crate's version, not selvedge's: `CARGO_PKG_VERSION` evaluated
    // inside selvedge would report the library.
    version: env!("CARGO_PKG_VERSION"),
    frontends: FRONTENDS,
    aliases: ALIASES,
    // A shell script whose replacement is a flag: the binary answers this
    // name with a usage error, so a key binding on it has to stop finding it.
    legacy: &["tailgauge-update"],
    // No MSI: there is no Windows build, and an update always replaces in
    // place.
    msi_marker_key: None,
};

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

pub const FRONTENDS: &[Frontend] = &[
    Frontend {
        id: "plasma",
        label: "KDE Plasma applet",
        payload: "plasma/org.tailgauge.plasmoid",
        artifact: "org.tailgauge.plasmoid",
        version_source: VersionSource::PlasmaMetadata,
        gsettings_schemas: false,
        compiled: false,
        restart: Restart::Cheap("kquitapp6 plasmashell && kstart plasmashell"),
    },
    Frontend {
        id: "gnome",
        label: "GNOME Shell extension",
        payload: "gnome/tailgauge@arzaroth.github.io",
        artifact: "tailgauge@arzaroth.github.io",
        version_source: VersionSource::GnomeMetadata,
        gsettings_schemas: true,
        // TypeScript: the directory beside this one holds the sources, and
        // installing those lands an extension the shell refuses to load.
        compiled: true,
        restart: Restart::Session(
            "log out and back in, then: gnome-extensions enable tailgauge@arzaroth.github.io",
        ),
    },
    Frontend {
        id: "omarchy",
        label: "Omarchy bar widget",
        payload: "omarchy/arzaroth.tailgauge",
        artifact: "arzaroth.tailgauge",
        version_source: VersionSource::ManifestVersion,
        gsettings_schemas: false,
        compiled: false,
        restart: Restart::Cheap("omarchy-shell -q shell rescanPlugins"),
    },
];

/// The contract between this repository and the release it publishes.
///
/// selvedge knows how to fetch an archive and install what is in it. What it
/// cannot know is whether *this* repository builds the assets it will go
/// looking for, so these read the workflow, the manifests and the build
/// script. They were inner tests of the modules that moved out; the machinery
/// went with them and this stayed.
#[cfg(test)]
mod tests {
    use super::*;
    use selvedge::update::{ARCHIVE_SUFFIX, arch_target};
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

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
        assert!(release.contains(&format!("\"$stage/{}\"", TAILGAUGE.primary())));
        for f in TAILGAUGE.frontends {
            assert!(
                release.contains(&format!(
                    "$stage/{}/{}",
                    selvedge::frontend::ARCHIVE_ROOT,
                    f.id
                )),
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
        let slug = TAILGAUGE.repo();
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

    #[test]
    fn every_payload_exists_in_the_repository() {
        // The release workflow copies these directories into the archive by
        // name. Renamed here without the workflow following, an update
        // silently stops refreshing that frontend; renamed in the repository,
        // the archive ships an empty directory. Neither fails at build time.
        let repo = repo_root();
        for f in TAILGAUGE.frontends {
            let dir = repo.join(f.payload);
            assert!(dir.is_dir(), "{} payload missing: {}", f.id, dir.display());
            assert_eq!(
                dir.file_name().and_then(|n| n.to_str()),
                Some(f.artifact),
                "{} payload does not end in the artifact directory its desktop expects",
                f.id
            );
            assert!(
                f.version_in(&dir).is_some(),
                "{} payload carries no readable version; skew would be undetectable",
                f.id
            );
        }
    }

    #[test]
    fn every_payload_declares_the_same_version_as_the_binary() {
        let repo = repo_root();
        for f in TAILGAUGE.frontends {
            assert_eq!(
                f.version_in(&repo.join(f.payload)).as_deref(),
                Some(env!("CARGO_PKG_VERSION")),
                "{} disagrees with the binary, so a release would ship two products",
                f.id
            );
        }
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
        for alias in TAILGAUGE.aliases {
            let sub = alias.strip_prefix("tailgauge-").unwrap();
            assert!(known.contains(&sub), "{alias} has no subcommand");
        }
        assert_eq!(
            TAILGAUGE.aliases.len(),
            known.len(),
            "a subcommand has no alias"
        );
        assert!(
            !TAILGAUGE.aliases.contains(&"tailgauge-update"),
            "updating is --update, so this alias would resolve to nothing"
        );
    }

    /// The two lists are opposites, and a name in both would be written and
    /// removed by the same update.
    #[test]
    fn a_legacy_name_is_not_also_an_alias() {
        for stale in TAILGAUGE.legacy {
            assert!(
                !TAILGAUGE.aliases.contains(stale),
                "{stale} is listed as both a name to keep and a name to take away"
            );
            let sub = stale
                .strip_prefix("tailgauge-")
                .expect("a legacy name is one of ours");
            assert!(
                ![
                    "ctl",
                    "watch",
                    "notify",
                    "send",
                    "receive",
                    "file-select",
                    "copy"
                ]
                .contains(&sub),
                "{stale} names a subcommand the binary has; removing it breaks that name"
            );
        }
    }

    /// An install from 0.4.0 or earlier updates by running its own
    /// `tailgauge-update`, which downloads a `helpers` asset and installs
    /// whatever `bin/` it carries. 0.5.0 stopped publishing that asset, so
    /// every one of those installs upgraded its frontends, 404'd on the
    /// helpers, and left a panel calling a binary nobody had installed.
    #[test]
    fn the_release_still_publishes_the_helpers_asset_older_installs_ask_for() {
        assert!(
            published_suffixes().iter().any(|s| s == "helpers.tar.gz"),
            "the compatibility asset is gone; published: {:?}",
            published_suffixes()
        );

        let release = repo_file(".github/workflows/release.yml");
        assert!(
            release.contains("scripts/compat/tailgauge-update pack/helpers/bin/tailgauge-update"),
            "the helpers asset must carry the bootstrap under bin/, which is \
             the only path the old updater installs from"
        );
    }

    /// The bootstrap is the one thing that turns a 0.4.0 install into a
    /// working one, and it names the archive itself.
    #[test]
    fn the_bootstrap_downloads_an_archive_the_release_publishes() {
        let bootstrap = repo_file("scripts/compat/tailgauge-update");
        for arch in ARCHES {
            let wanted = format!("linux-{arch}.tar.gz");
            assert!(
                published_suffixes().iter().any(|s| s.ends_with(&wanted)),
                "the release publishes no {wanted}"
            );
        }
        assert!(
            bootstrap.contains("tailgauge-$tag-linux-$arch.tar.gz"),
            "the bootstrap does not name the archive the release publishes"
        );
        // It installs the binary under every name the old scripts answered to,
        // or a key binding on tailgauge-ctl breaks on upgrade.
        for alias in TAILGAUGE.aliases {
            let sub = alias.strip_prefix("tailgauge-").expect("prefixed");
            assert!(
                bootstrap.contains(sub),
                "{alias} is not among the names the bootstrap links"
            );
        }
        assert!(
            bootstrap.contains("rm -f \"$bindir/tailgauge-update\""),
            "the bootstrap must remove itself; nothing else will"
        );

        // It is the only way back, so it goes after the binary has been shown
        // to run. A binary for the wrong libc is a file that exists and does
        // nothing, and `set -e` is not on.
        let proves = bootstrap
            .find("--version 2>&1")
            .expect("the bootstrap runs the binary before trusting it");
        let removes = bootstrap
            .find("rm -f \"$bindir/tailgauge-update\"")
            .expect("the bootstrap removes itself");
        assert!(
            proves < removes,
            "the bootstrap removes the way back before proving the binary runs"
        );

        // A 0.4.0 panel polls --json and applies with --apply, so the pending
        // migration has to show up as available or that panel reports the
        // install as current while the binary is missing.
        let json = bootstrap
            .split_once("--json ]]")
            .map(|(_, rest)| rest.split("exit 0").next().unwrap_or(""))
            .expect("the bootstrap answers --json");
        assert!(
            json.contains(r#""available":true"#) && json.contains(r#""updatable":true"#),
            "the --json answer hides the pending migration from an old panel"
        );
    }
}
