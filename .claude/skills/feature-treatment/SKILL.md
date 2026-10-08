---
name: feature-treatment
description: Ship a finished TailGauge feature branch the full way - rebase onto master, run a max-effort multi-agent code review, fix everything confirmed, bring the README and CHANGELOG in line, pass the gate, open the PR and work CodeRabbit to a rebase-merge, cut a release, clean up the worktree. Use when the user says "feature treatment", "treat this branch", "review and merge this feature", or names a worktree/branch to ship.
---

# Feature treatment

The pipeline a finished feature branch goes through before it lands: **rebase ->
max review -> fix -> docs -> gate -> PR -> merge -> release -> clean up**. The
point is that nothing merges without an adversarial review and a green gate.

The branch to treat comes from the argument (a branch or worktree name). If none
is given, find it with `wt list`: the feature branch is the non-`master` one, in
a worktree under `~/repos/TailGauge.worktrees/<branch>` (branch names keep their
slash, e.g. `feature/tui`). Confirm which one if ambiguous. The shell's working
directory resets between calls, so every git and gh command below runs with a
`cd <worktree> &&` prefix or `git -C <worktree>`, never bare.

## 1. Rebase onto master

```bash
cd ~/repos/TailGauge.worktrees/'<branch>' && git fetch origin && git rebase origin/master
```

Resolve conflicts if any (the **resolving-merge-conflicts** skill). The branch
must sit directly on `master` so the review and the merge see only this
feature's diff. Keep the layers: mechanical, then behaviour, then tests and docs,
each commit green on its own and prefixed `[<branch>]`.

## 2. Max-effort review (parallel finders)

Review diff: `git diff origin/master...HEAD -- . ':(exclude)Cargo.lock' ':(exclude)pnpm-lock.yaml'`.

Spawn **independent finder subagents in parallel** (one message, several Agent
calls), each over the same diff with a different lens. Scale the count to the
feature (4-6 is typical):

- **Correctness** - line by line: inverted conditions, off-by-one, `unwrap` on
  input that can be absent, swallowed errors, ordering, the JSON the binary
  sends vs what `gnome/.../panel.ts` and the QML read, a parser fed a capture
  shape it has never seen (empty tailnet, logged out, daemon down, NetBird only).
- **Security** - argv built from panel input (every CLI call must be an argv,
  never a shell string), Taildrop paths and file names, temp files and
  permissions, anything the systemd unit or `scripts/*.sh` run with
  user-controlled values. The updater itself lives in selvedge: here the
  surface is `crates/tailgauge/src/project.rs` and a change to the selvedge tag.
- **Parity / integration** - does a frontend now decide something the binary
  should (a visible string, section visibility, a search haystack, a provider
  binary name)? `crates/tailgauge-core/tests/parity.rs` catches some of it, not
  all. Do all three frontends and `tailgauge tui` draw the new rows? Is the
  `panel.ts` contract updated with the Rust struct? Store-installed frontends
  update apart from the binary, so a change to the panel JSON or the `--ui`
  input must still work with a frontend one release older or newer. Do the
  manifests, the release asset names and `crates/tailgauge/src/project.rs` still
  agree? A new subcommand needs its alias in both `scripts/install.sh` and
  `ALIASES` in `project.rs`; only the first is tested. Is there a CHANGELOG
  `[Unreleased]` entry for every user-visible change?
- **Reuse / simplify / altitude** - does new code re-implement something in
  `tailgauge-core` or selvedge? copy-paste across the three frontends or across
  providers? a special case where the provider registry already generalizes?
- **Pitfalls / tests** - GJS and QML footguns (a Plasma import that silently
  blanks the plasmoid, `console.log` filtered out in Plasma, a Shell or GI API
  newer than the lowest `shell-version` in the extension's metadata, which still
  typechecks because `@girs/gnome-shell` is pinned to the newest),
  async/process footguns in the binary, and whether the tests cover the new
  branches: a `specification.rs` case against a real capture, an e2e run against
  the fake `tailscale` / `netbird` on an isolated PATH, the QML harness fixtures
  in `tests/qml`, the node tests in `tests/gnome`. The node tests never load
  `extension.ts`, so a change to it gets a run of `tests/gnome/shell/run.sh` at
  `--gnome 45` and `--gnome 50`.

Each finder returns findings as JSON `{file, line, severity, summary,
failure_scenario}`, verified (quote the line), most severe first. Tell them NOT
to fix anything. Optionally run one **sweep** finder with the merged list that
hunts only for gaps.

## 3. Fix what's confirmed

Re-verify each claim before fixing: finders surface plausible-but-wrong items
too. Fix every confirmed correctness and security issue and the worthwhile
quality ones, as new layers on top (`[<branch>] fix(<area>): ...`), not folded
into earlier commits once the branch has been pushed. Anything real but out of
scope: tell the user, or open a GitHub issue if they agree.

## 4. Docs

The docs ship in the same PR, not as a follow-up. Apply the **docs-update**
skill scoped to this branch; the essentials:

- `CHANGELOG.md` `[Unreleased]`: every user-visible change, in the house style
  (bold one-sentence headline, then a paragraph on why or how).
- `README.md`: any section can be the one a change makes wrong, from
  Requirements (a new provider CLI) and Install (installer flags) through
  Distribution (updater flags, release assets) to Development.
- `omarchy/arzaroth.tailgauge/README.md` documents the Omarchy widget on its
  own: its panel, interactions and settings.
- A new setting lands in six places: `gnome/.../schemas` and a row in
  `prefs.ts`, `plasma/.../config/main.xml` and a control in
  `ConfigGeneral.qml`, `barWidget.defaults` and `barWidget.schema` in the
  Omarchy `manifest.json`, and the README Settings section.
- No em-dashes; relative links; cite paths rather than restating code.

## 5. Gate (must be green before the PR)

Run the gate in section 4 of the **release** skill
(`.claude/skills/release/SKILL.md`) in the worktree: it is every step of
`.github/workflows/ci.yml`. Say which steps were skipped for a missing tool; CI
runs them on the PR. Check the file count stays under the limit:
`git diff --name-only origin/master...HEAD | wc -l`.

## 6. PR and merge

```bash
cd ~/repos/TailGauge.worktrees/'<branch>' && git push -u --force-with-lease origin '<branch>'
gh pr create --base master --head '<branch>' --title "<what it does, in plain words>" --body "..."
```

`--force-with-lease` because the rebase in step 1 may have rewritten a branch
that was already pushed. PR titles in this repo are plain sentences, not
conventional-commit subjects (e.g. "The panel, in a terminal"). Then hand the PR
to the **babysit-pr** skill: CodeRabbit reviews every PR here, and its rounds are
worked like any other. The merge is a **rebase merge** so the layers land on
`master` as they are:

```bash
gh pr merge <n> --rebase --delete-branch
```

## 7. Release

Run the **release** skill from the main checkout on an updated `master`
(`git pull --ff-only`). Pick **minor** for a user-facing feature, **patch** for
fix-only.

## 8. Clean up

```bash
wt remove '<branch>'
```

## Done when

The feature is on `master`, the README and CHANGELOG describe it, the gate and
CI were green, the release has its assets, and the worktree is gone. Report the
version and a one-line summary of what the review caught.
