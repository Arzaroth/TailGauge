---
name: upstream-check
description: Routine review of the two projects TailGauge depends on without depending on at runtime - basecamp/omarchy (the plugin the whole panel was ported from, and the shell surfaces the Omarchy widget rides on) and tailscale/tailscale (the CLI output the shared model parses). Use for "did upstream move", "should we backport", "check omarchy", "check tailscale", "upstream check", or any periodic upstream drift review.
---

# Upstream check

TailGauge internalized a first-party Omarchy plugin and parses a CLI it does not
ship. Nothing tells us when either moves except this check.

| Upstream | What we took | What can rot |
| --- | --- | --- |
| [basecamp/omarchy](https://github.com/basecamp/omarchy) (MIT, default branch `quattro`) | `shared/model.js` from their `Model.js`, the Taildrop helpers from their `omarchy-tailscale-*` scripts, and `omarchy/arzaroth.tailgauge` rides on the shell's own QML components | Their widget grows a feature ours lacks or fixes a parse bug still in ours; the shell APIs our widget imports move, and the project is `4.x.alpha` with no stability promise |
| [tailscale/tailscale](https://github.com/tailscale/tailscale) | Nothing copied. The model parses `status --json`, `exit-node list`, `switch --list --json`, and the helpers drive `file get` and `debug watch-ipn` | A field is renamed or dropped, the exit-node table changes columns, a flag moves. Every frontend goes blank at once and CI cannot see it |

Read `BASELINES.md` in this folder first: it records what each upstream looked
like at the last check. Update it at the end of every run.

## Omarchy

Two questions, one repo.

**1. Did their Tailscale widget grow something ours lacks, or fix something ours
still has wrong?** This is the one that matters most: `shared/model.js` is their
`Model.js` with our additions, so their parser fixes are our parser fixes.

```bash
gh api "repos/basecamp/omarchy/commits?path=shell/plugins/panels/tailscale&since=<baseline date>&per_page=100" \
  --jq '.[] | "\(.commit.committer.date[0:10]) \(.sha[0:8]) \(.commit.message|split("\n")[0])"'
```

The installed copy under `/usr/share/omarchy/shell/plugins/panels/tailscale/` is
easier to read than the API, and `omarchy-version` says which release it is.
Diff their `Model.js` against ours rather than reading the commits alone -
our copy is reorganized, so a hunk that looks unrelated usually is not:

```bash
gh api repos/basecamp/omarchy/contents/shell/plugins/panels/tailscale/Model.js \
  --jq '.content' | base64 -d >/tmp/upstream-model.js
diff /tmp/upstream-model.js shared/model.js
```

Expect a large diff: the panel resolver, the layout vocabulary, the owner
lookup, the machine search, the offline handling and the update banner are ours.
**Signal** is anything inside the parsing half - `parseStatus`, `parseAccounts`,
`parseExitNodeList` and its fixed-width column slicing, `mullvadRegionOptions`,
`isTaildropTarget`, `hasFileSharing`, `loginPlan`. **Noise** is their Panel.qml
layout, which we deliberately do not follow.

Feature parity is a judgement call, not an obligation. Their widget is one
frontend; ours is three, so a panel feature is three implementations plus a
model change plus the parity tests. Ours is also ahead on several rows already -
that asymmetry is the reason this repository exists.

**2. Did a surface our Omarchy widget rides on move?** These are the paths whose
breakage we would otherwise learn about from a blank bar:

```bash
for p in shell/Ui shell/Commons/Style.qml shell/Commons/Color.qml \
         shell/services/PluginRegistry.qml bin/omarchy-plugin-validate \
         bin/omarchy-tailscale-send bin/omarchy-tailscale-receive; do
  echo "### $p"
  gh api "repos/basecamp/omarchy/commits?path=$p&since=<baseline date>&per_page=100" \
    --jq '.[] | "\(.commit.committer.date[0:10]) \(.sha[0:8]) \(.commit.message|split("\n")[0])"'
done
```

Ask per path. The compare endpoint caps `files` at 300 without saying so, so
watched changes hide behind unwatched ones in a busy month.

`shell/Ui` is the widest of these and the one that actually breaks us:
`Panel`, `KeyboardPanel`, `PanelKeyCatcher`, `PanelHero`, `PanelSectionHeader`,
`PanelSeparator`, `PanelActionButton`, `PanelToolTip`, `CursorSurface`,
`ToggleSwitch`, `TextField` and `BarIconButton` are all load-bearing in
`omarchy/arzaroth.tailgauge/Panel.qml`. A renamed property there is a runtime
error the QML lint in CI cannot see, because those imports do not resolve on a
CI runner at all.

`PluginRegistry.qml` owns the manifest schema and the rule that a plugin folder
carries no symlinks - which is why `scripts/build.sh` copies `shared/model.js`
in rather than linking it. `omarchy-plugin-validate` mirrors those checks, and
running it against `build/arzaroth.tailgauge` is the cheapest way to find out
that the schema moved:

```bash
scripts/build.sh && omarchy-plugin-validate build/arzaroth.tailgauge
```

Deliberately not watched:

- `migrations/` - upstream lands one with nearly every change, and a monitor
  that always fires is one you stop reading.
- Themes, `bin/omarchy-launch-browser` and the rest of `bin/` - we call two of
  those scripts by name and nothing else.

A hit means "read the diff", not "you are broken".

## Tailscale

The model parses CLI output, so a Tailscale release is the one upstream that can
empty every panel at once. Releases are the readable unit:

```bash
gh api repos/tailscale/tailscale/releases --paginate \
  --jq '.[] | select(.published_at > "<baseline date>") | "## \(.tag_name) (\(.published_at[0:10]))\n\(.body)\n"'
```

**Signal** - anything about `tailscale status` JSON fields (`BackendState`,
`Self`, `Peer`, `User`, `CapMap`, `ExitNode`, `Online`, `TailscaleIPs`,
`DNSName`, `UserID`), the `tailscale exit-node list` table, `tailscale switch
--list --json`, `tailscale set --exit-node`, `tailscale file get`, or
`tailscale debug watch-ipn`. **Noise** - everything about the daemon, the
control plane, platform ports, and the GUI clients.

`parseExitNodeList` is the fragile one: it slices fixed-width columns out of a
human-readable table that carries no compatibility promise at all. Check it
against the real thing rather than against the release notes:

```bash
tailscale exit-node list | head -5
node --test test/model.test.mjs
```

`test/fixtures/` holds the captured output the model tests run against. When the
CLI's shape changes, the fixture is what needs updating first - a test suite
passing against a stale fixture is exactly how this breaks silently.

## Report

Answer both questions plainly, then list backport candidates worth the work:
what upstream changed, which of our files owns it, whether a user on our side
can actually hit it, and - for an Omarchy widget feature - what it would cost in
the other two frontends. Say "nothing to do" when that is the answer. Do not
open issues or start implementing without being asked: this check ends in a
recommendation.

Finish by rewriting `BASELINES.md` with today's date, the current
`basecamp/omarchy` HEAD sha on `quattro`, and the newest Tailscale release tag:

```bash
gh api repos/basecamp/omarchy/commits/quattro --jq '.sha'
gh api repos/tailscale/tailscale/releases/latest --jq '.tag_name'
omarchy-version; tailscale version | head -1
```
