---
name: upstream-check
description: Routine review of what TailGauge depends on without depending on at runtime - basecamp/omarchy (the plugin the whole panel was ported from, and the shell surfaces the Omarchy widget rides on), tailscale/tailscale and netbirdio/netbird (the CLI output the core parses), the GNOME Shell and Plasma releases our extension and plasmoid declare, the pinned selvedge crate, and peer frontends (tsui, trayscale, KTailctl) worth borrowing fixes from. Use for "did upstream move", "should we backport", "check omarchy", "check tailscale", "check netbird", "new GNOME release", "upstream check", or any periodic upstream drift review.
---

# Upstream check

TailGauge internalized a first-party Omarchy plugin, parses two CLIs it does not
ship, and installs into two desktop shells that each declare which releases
they accept. Nothing tells us when any of them moves except this check.

| Upstream | What we took | What can rot |
| --- | --- | --- |
| [basecamp/omarchy](https://github.com/basecamp/omarchy) (MIT, default branch `quattro`) | the parsers in `crates/tailgauge-core` from their `Model.js` (ported through a TypeScript `shared/model.ts`, now Rust), Taildrop send and receive (`crates/tailgauge/src/send.rs`, `receive.rs`) from their `omarchy-tailscale-*` scripts, and `omarchy/arzaroth.tailgauge` rides on the shell's own QML components | Their widget grows a feature ours lacks or fixes a parse bug still in ours; the shell APIs our widget imports move, and the project is `4.x.alpha` with no stability promise |
| [tailscale/tailscale](https://github.com/tailscale/tailscale) | Nothing copied. `tailgauge-core` parses `status --json`, `exit-node list`, `switch --list --json`, and the binary drives `file get` and `debug watch-ipn` | A field is renamed or dropped, the exit-node table changes columns, a flag moves. Every frontend goes blank at once and CI cannot see it |
| [netbirdio/netbird](https://github.com/netbirdio/netbird) | Nothing copied. `crates/tailgauge-core/src/netbird.rs` parses `status --json` and the human-readable `networks list`, and `providers.rs` drives `up`, `down` and `networks select` / `deselect` | The same as Tailscale, with a faster release train (several minors a month) and a text table for networks |
| [GNOME Shell](https://gitlab.gnome.org/GNOME/gnome-shell) and [Plasma](https://invent.kde.org/plasma/plasma-workspace) | Nothing copied. The extension's `metadata.json` lists every `shell-version` it accepts; the plasmoid declares `X-Plasma-API-Minimum-Version` | A new GNOME major is refused outright until it is listed; a Plasma major can drop QML API the plasmoid imports |
| [Arzaroth/selvedge](https://github.com/Arzaroth/selvedge) | The updater, pinned by tag in the workspace `Cargo.toml` | A fix to fetching or installing lands there and never reaches us until the pin moves |
| Peer frontends: [tsui](https://github.com/neuralinkcorp/tsui), [trayscale](https://github.com/DeedleFake/trayscale), [KTailctl](https://github.com/f-koehler/KTailctl) | The TUI's layout and keys are tsui's (`brain/features/tui.md`); nothing from the others | Nothing breaks. They fix bugs and grow rows we might want, and tsui's are often ours too |

Read `BASELINES.md` in this folder first: it records what each upstream looked
like at the last check. Update it at the end of every run.

## Omarchy

Two questions, one repo.

**1. Did their Tailscale widget grow something ours lacks, or fix something ours
still has wrong?** This is the one that matters most: the parsers in
`crates/tailgauge-core/src/` descend from their `Model.js`, so their parser
fixes are our parser fixes.

```bash
gh api "repos/basecamp/omarchy/commits?path=shell/plugins/panels/tailscale&since=<baseline date>&per_page=100" \
  --jq '.[] | "\(.commit.committer.date[0:10]) \(.sha[0:8]) \(.commit.message|split("\n")[0])"'
```

The installed copy under `/usr/share/omarchy/shell/plugins/panels/tailscale/` is
easier to read than the API, and `omarchy-version` says which release it is.
Read their `Model.js` against our parsers rather than reading the commits
alone - ours is reorganized and in another language, so a hunk that looks
unrelated usually is not:

```bash
gh api repos/basecamp/omarchy/contents/shell/plugins/panels/tailscale/Model.js \
  --jq '.content' | base64 -d >/tmp/upstream-model.js
less /tmp/upstream-model.js   # against crates/tailgauge-core/src/{status,accounts,exit_nodes,peer}.rs
```

Expect a large diff: the panel resolver, the layout vocabulary, the owner
lookup, the machine search, the offline handling and the update banner are ours.
**Signal** is anything inside the parsing half - their `parseStatus`,
`parseAccounts`, `parseExitNodeList` and its fixed-width column slicing,
`mullvadRegionOptions`, `isTaildropTarget`, `hasFileSharing`, `loginPlan`;
ours are `parse_status`, `parse_accounts`, `parse_exit_node_list`,
`mullvad_region_options`, `is_taildrop_target`, `StatusOk::file_sharing` and
`login_plan`. **Noise** is their Panel.qml
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
carries no symlinks, so `scripts/build.sh` copies the payload rather than
linking it.
`omarchy-plugin-validate` mirrors those checks, and running it against
`build/arzaroth.tailgauge` is the cheapest way to find out that the schema
moved:

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

`tailgauge-core` parses CLI output, so a Tailscale release is the one upstream that can
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

`parse_exit_node_list` is the fragile one: it slices fixed-width columns out of a
human-readable table that carries no compatibility promise at all. Check it
against the real thing rather than against the release notes:

```bash
tailscale exit-node list | head -5
cargo test -p tailgauge-core --test specification
```

`crates/tailgauge-core/tests/fixtures/` holds the captured output
`specification.rs` runs against. When the
CLI's shape changes, the fixture is what needs updating first - a test suite
passing against a stale fixture is exactly how this breaks silently.

## NetBird

Releases are frequent and mostly about the management server, so ask per path
rather than reading every release:

```bash
for p in client/cmd/status.go client/cmd/networks.go client/status \
         client/cmd/up.go client/cmd/down.go; do
  echo "### $p"
  gh api "repos/netbirdio/netbird/commits?path=$p&since=<baseline date>&per_page=100" \
    --jq '.[] | "\(.commit.committer.date[0:10]) \(.sha[0:8]) \(.commit.message|split("\n")[0])"'
done
```

**Signal** - a renamed or dropped key in `status --json` (`daemonStatus`,
`fqdn`, `netbirdIp`, and per `peers.details[]` entry `fqdn`, `netbirdIp`,
`publicKey`, `status`, `latency`, `connectionType`, `relayAddress`,
`iceCandidateEndpoint.remote`, `transferReceived`, `transferSent`), the
daemon states `netbird.rs` reads as "needs login", the layout of
`networks list` (the `- ID:` / `Network:` / `Domains:` blocks), and any flag
`providers.rs` passes. **Noise** - the management server, relay, signal, the
proxy and the GUI. Check the live CLI against
`tests/fixtures/netbird-status.json` and `netbird-networks.txt`;
`networks list` needs a connected client, so say so when it could not be
captured rather than calling it unchanged.

## Desktop shells

GNOME refuses an extension whose `metadata.json` does not list the running
major, so a new GNOME release is a disabled extension for everyone who
upgrades, with no error we ever see:

```bash
gh api "repos/GNOME/gnome-shell/tags?per_page=10" --jq '.[].name'
jq -r '."shell-version" | join(" ")' gnome/tailgauge@arzaroth.github.io/metadata.json
gh api "repos/KDE/plasma-workspace/tags?per_page=5" --jq '.[].name'
```

A stable GNOME major (`NN.0`) missing from `shell-version` is a finding, and
the fix is listing it after `tests/gnome/shell/run.sh --gnome NN` passes, never
before. For Plasma, only a new major (7.x) is signal: the plasmoid targets the
6.x API.

## selvedge

```bash
grep selvedge Cargo.toml
gh api repos/Arzaroth/selvedge/tags --jq '.[0:5][].name'
gh api repos/Arzaroth/selvedge/contents/CHANGELOG.md --jq .content | base64 -d | less
```

It is shared with TokenGauge, so most releases are for TokenGauge. Signal is a
fix under fetching a release, verifying it, replacing the binary or installing
payloads; that is a bump here. Anything else can wait for a release that needs
it.

## Peer frontends

Nothing of theirs is copied, so this is the one section that can only ever
produce ideas. Read the non-dependency commits since the baseline:

```bash
for r in neuralinkcorp/tsui DeedleFake/trayscale f-koehler/KTailctl; do
  echo "### $r"
  gh api "repos/$r/commits?since=<baseline date>&per_page=100" \
    --jq '.[] | "\(.commit.committer.date[0:10]) \(.sha[0:8]) \(.commit.message|split("\n")[0])"' \
    | grep -viE 'bump|deps|renovate|dependabot|translat|weblate'
done
```

**Signal** - a field of `tailscale status` they started showing or showing
right (key expiry, `Expired`, `Online`, exit-node state), a parse or edge-case
fix in their status handling, and for tsui any change to the layout or keys our
TUI follows. **Noise** - their toolkit, packaging, trayscale's LocalAPI
plumbing (we drive the CLI, so its races are not ours). A peer row is a
candidate for `panel_spec`, which means all four surfaces, so weigh it the way
an Omarchy widget feature is weighed.

## Report

Answer each section plainly, then list backport candidates worth the work:
what upstream changed, which of our files owns it, whether a user on our side
can actually hit it, and - for an Omarchy widget feature - what it would cost in
the other two frontends. Say "nothing to do" when that is the answer. Do not
open issues or start implementing without being asked: this check ends in a
recommendation.

Finish by rewriting `BASELINES.md` with today's date, the current
`basecamp/omarchy` HEAD sha on `quattro`, the newest Tailscale and NetBird
release tags, the newest GNOME and Plasma tags, the selvedge pin against its
latest tag, and each peer's HEAD:

```bash
gh api repos/basecamp/omarchy/commits/quattro --jq '.sha'
gh api repos/tailscale/tailscale/releases/latest --jq '.tag_name'
gh api repos/netbirdio/netbird/releases/latest --jq '.tag_name'
omarchy-version; tailscale version | head -1; netbird version
for r in neuralinkcorp/tsui DeedleFake/trayscale f-koehler/KTailctl; do
  echo "$r $(gh api "repos/$r/commits?per_page=1" --jq '.[0].sha[0:8]')"
done
```
