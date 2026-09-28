---
name: release
description: Cut a TailGauge release, the whole shebang - docs sweep (CHANGELOG, README), bump the version in all five places it is declared, run the local gate, commit, annotated tag, push, then watch the Release workflow until the GitHub release carries every asset. Use when the user says "cut a release", "release", "ship 0.x.y", or "the whole release shebang".
---

# Release shebang

Everything between "the code is on `master`" and "the GitHub release has its
assets". There is no release task: the bump is by hand, and the value of this
skill is not forgetting one of the places a version lives. A tag that disagrees
with any manifest refuses to publish (`.github/workflows/release.yml`, "Check the
tag matches the manifests"), so a forgotten file costs a re-tag.

## 1. Pre-flight

- Working tree clean, on `master`, up to date with `origin/master`.
- Previous version: `git describe --tags --abbrev=0` (tags are `vX.Y.Z`).
- Pick the next semver: **minor** for a new way to use the thing (a feature, a
  new frontend or subcommand), **patch** for fix-only.

## 2. Docs sweep (against `git log v<last>..HEAD` and `git diff v<last>..HEAD`)

- **CHANGELOG.md `[Unreleased]`** must cover every user-visible change since the
  last tag. Entries are normally written on the feature branch, so this is a
  completeness check. House style: a bold one-sentence headline that says what
  changed for the user, then a paragraph on why or how. Categories in use:
  `Added`, `Changed`, `Fixed`, and occasionally `Distribution` / `Shared`.
- **README.md**: Features, Install, Outside the panel, Settings, Differences
  between the frontends, Release assets, Development. Update whatever the
  release makes wrong (a new subcommand, flag, setting, asset, or CI step).
- If the sweep changes anything, commit it on `master` first as
  `[master] docs: ...`, separate from the release commit.

## 3. Bump

Move `## [Unreleased]`'s content under a new `## [x.y.z]` heading (no date, that
is the house format) and leave `## [Unreleased]` empty above it. Then:

| File | Field |
| --- | --- |
| `Cargo.toml` | `[workspace.package] version` |
| `Cargo.lock` | follows: run `cargo check --workspace` after the Cargo.toml edit |
| `plasma/org.tailgauge.plasmoid/metadata.json` | `KPlugin.Version` |
| `gnome/tailgauge@arzaroth.github.io/metadata.json` | `version-name`, **and `version` +1** (the integer EGO orders uploads by; no version-string search finds it) |
| `omarchy/arzaroth.tailgauge/manifest.json` | `version` |
| `README.md` | the `tailgauge --update` sample output under "Updating" (`Current version` and `Already up to date`) |
| `.github/workflows/release.yml` | the `workflow_dispatch` tag description example |

Check nothing still carries the old version:

```bash
git grep -n -F '<old x.y.z>' -- ':!CHANGELOG.md'
```

## 4. Gate (local mirror of `.github/workflows/ci.yml`)

```bash
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
pnpm install --frozen-lockfile && pnpm build
pnpm typecheck
node --import ./tests/gnome/register.js --test 'tests/gnome/**/*.test.js'
tests/qml/run.sh                                  # needs the Qt 6 `qml` runtime
shellcheck --severity=warning scripts/*.sh scripts/compat/*
```

If `qml`, `qmllint` or `shellcheck` is not installed locally, say which step was
skipped rather than implying it passed; CI runs them on the push.

## 5. Commit, tag, push

```bash
git commit -am "[master] chore(release): x.y.z" -m "<why minor or patch, in a sentence or two>" \
  -m "The GNOME extension's integer version goes to <N>. It is what EGO orders uploads by, and no version-string search can find it."
git tag -a vx.y.z -m vx.y.z
git push origin master vx.y.z
```

The commit subject carries the `[master]` branch prefix, and the body ends with
the attribution trailer.

## 6. Watch the release

```bash
gh run list --workflow release.yml --limit 1
gh run watch <run-id> --exit-status
gh release view vx.y.z --json assets --jq '.assets[].name'
```

Expect six assets: `-plasmoid.plasmoid`, `-gnome-shell-extension.zip`,
`-omarchy-plugin.tar.gz`, `-linux-x86_64.tar.gz`, `-linux-aarch64.tar.gz`, and
`-helpers.tar.gz` (the bootstrap installs from 0.4.0 or earlier still ask for).
The release notes are the `## [x.y.z]` CHANGELOG section; if the run warns that
it fell back to auto-generated notes, the heading did not match.

If the workflow fails after the tag is pushed, fix on `master`, then either
re-run it against the existing tag (`gh workflow run release.yml -f tag=vx.y.z`)
when the fix is outside the tagged tree, or cut the next patch when it is inside.
Never move a pushed tag.

## 7. Hand off

The stores are manual and not this skill's to do: the `.plasmoid` goes to
store.kde.org and the extension zip to extensions.gnome.org. Tell the user they
are ready, with the release URL.
