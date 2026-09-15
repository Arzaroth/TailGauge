import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {read} from './paths.js';

// The parity rule, enforced rather than remembered: shared/model.ts decides what
// the panel contains, and a frontend decides only how a row looks. These tests
// fail when a layout decision leaks back into one desktop, which is how the
// three would start to disagree.



const plasma = ['plasma/org.tailgauge.plasmoid/contents/ui/FullRep.qml',
                'plasma/org.tailgauge.plasmoid/contents/ui/PanelRowView.qml',
                'plasma/org.tailgauge.plasmoid/contents/ui/CompactRep.qml'];
const gnome = ['gnome/tailgauge@arzaroth.github.io/extension.ts'];
const omarchy = ['omarchy/arzaroth.tailgauge/Panel.qml'];

const plasmaSource = plasma.map(read).join('\n');
const gnomeSource = gnome.map(read).join('\n');
const omarchySource = omarchy.map(read).join('\n');

const frontends = [['plasma', plasmaSource], ['gnome', gnomeSource], ['omarchy', omarchySource]];

// The frontends still resolving the panel for themselves. Omarchy has been
// flipped onto `tailgauge panel --json`, so the rules about gathering a
// snapshot and handing it to resolvePanel describe the two that have not been,
// and will be empty by the end of the port.
const resolving = [['plasma', plasmaSource], ['gnome', gnomeSource]];

// Rows GNOME shows that Plasma puts in the applet context menu instead. Both
// are desktop conventions, not panel content, so they are allowed to differ.
// Everything else the panel shows is written in shared/model.ts and arrives
// finished: there is no translator to hand it to, and has not been since the
// binary took the strings over.
const DESKTOP_ONLY = new Set(['Refresh', 'Settings']);

const plasmaStrings = new Set([...plasmaSource.matchAll(/i18n\("([^"]{4,})"\)/g)].map(m => m[1]));
const gnomeStrings = new Set([...gnomeSource.matchAll(/\b_\('([^']{4,})'\)/g)].map(m => m[1]));

test('no user-visible string is written in both frontends', () => {
    const shared = [...plasmaStrings].filter(s => gnomeStrings.has(s));
    assert.deepEqual(shared, [],
        `these belong in shared/model.ts, not in each frontend: ${shared.join(', ')}`);
});

test('frontend-local strings are desktop conventions only', () => {
    const stray = [...plasmaStrings, ...gnomeStrings].filter(s => !DESKTOP_ONLY.has(s));
    assert.deepEqual(stray, [],
        `resolvePanel should be producing these: ${stray.join(', ')}`);
});

// Omarchy's shell has no translation layer, so its labels are the model's own
// strings rather than a call the test can spot. The rule is checked from the
// other end there: nothing user-visible is written in the file at all.
test('the Omarchy frontend writes no user-visible string', () => {
    // QML takes either quote, and this repository writes both.
    const written = [...omarchySource.matchAll(/\b(?:text|placeholderText|tooltipText|title|meta|label):\s*(["'])((?:(?!\1).){4,}?)\1/g)]
        .map(m => m[2]);
    assert.deepEqual(written, [],
        `resolvePanel should be producing these: ${written.join(', ')}`);
});

test('no frontend re-derives which sections are visible', () => {
    for (const [name, source] of frontends) {
        assert.equal(/showConnections|showExitNodes|showPeers/.test(source), false,
            `${name} computes section visibility; resolvePanel already did`);
    }
});

test('no frontend rebuilds the exit-node or copy-option lists', () => {
    for (const [name, source] of frontends) {
        assert.equal(/displayExitNodes|_displayExitNodes/.test(source), false,
            `${name} assembles its own exit-node list`);
        assert.equal(/copyOptions\s*=\s*\[|peerCopyOptions\s*\(/.test(source), false,
            `${name} assembles its own copy options`);
    }
});

test('no frontend re-derives the status precedence', () => {
    for (const [name, source] of frontends) {
        assert.equal(/actionStatus\s*!==\s*['"]{2}\s*\?/.test(source), false,
            `${name} ranks actionStatus against lastError; panelStatus already did`);
    }
});

// The model writes the strings; a frontend that wraps one in its desktop's
// translator is either translating something already finished, or writing one
// of its own.
test('no frontend hands a panel string to a translator', () => {
    for (const [name, source] of frontends)
        assert.equal(/\bt:\s|Translate\b/.test(source), false,
            `${name} still passes resolvePanel a translator`);
});

test('every frontend still on the model reads the panel through resolvePanel', () => {
    for (const [name, source] of resolving)
        assert.match(source, /Model\.resolvePanel\(/, `${name} does not resolve the panel`);
});

// A flipped frontend draws what the binary sent and nothing it worked out
// itself. The panel arrives; it is not derived.
test('a flipped frontend has no model left to call', () => {
    const service = read('omarchy/arzaroth.tailgauge/Service.qml');
    for (const [what, source] of [['the panel', omarchySource], ['the service', service]]) {
        assert.equal(/\bModel\./.test(source), false, `omarchy's ${what} still calls the model`);
        assert.equal(/Model\.js/.test(source), false, `omarchy's ${what} still imports it`);
    }
    assert.match(service, /"tailgauge", "panel", "--json"/,
        'omarchy does not ask the binary for its panel');
    // Nothing is parsed on this side any more: the answer arrives resolved.
    assert.equal(/parseStatus|parseAccounts|parseExitNodeList/.test(service), false,
        'omarchy still parses a CLI');
});

test('every service hands resolvePanel the same snapshot shape', () => {
    const services = {
        plasma: 'plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml',
        gnome: 'gnome/tailgauge@arzaroth.github.io/provider.ts'
    };
    // The object literal is flat, so its first closing brace ends the field
    // list whatever the file indents with.
    const fields = (src: string): string[] => {
        const body = src.split('snapshot()')[1] ?? '';
        const start = body.indexOf('return {');
        const block = body.slice(start, body.indexOf('}', start));
        return [...block.matchAll(/^\s*(\w+):/gm)].map(m => m[1]).sort();
    };
    const plasmaFields = fields(read(services.plasma));
    assert.ok(plasmaFields.length > 15, 'the Plasma snapshot was not found');
    for (const [name, file] of Object.entries(services))
        assert.deepEqual(fields(read(file)), plasmaFields,
            `the ${name} snapshot disagrees, so its panel can disagree`);
});

// A service that hands resolvePanel a fresh array on every poll reports a
// change that did not happen, and the panel rebuilds every row it holds -
// which is how a search field loses focus mid-word several times a minute.
test('a QML service that still gathers does not report unchanged state as a change', () => {
    const services = {
        plasma: 'plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml'
    };
    for (const [name, file] of Object.entries(services)) {
        const src = read(file);
        assert.match(src, /function _stable\(/, `${name} never compares before it assigns`);
        for (const field of ['selfPeer', 'peers', 'ownExitNodes', 'mullvadRegions', 'accounts'])
            assert.match(src, new RegExp(`\\b${field} = _stable\\(`),
                `${name} reassigns ${field} unconditionally, rebuilding the panel on every poll`);
    }
});

// The poll watchdog exists for a status call that hangs. Armed and never
// disarmed, it instead kills whichever healthy poll is in flight when it fires,
// which on Omarchy surfaced as the panel reporting a tailnet down that never
// went anywhere.
test('a QML service that still gathers disarms its poll watchdog', () => {
    for (const [name, file] of Object.entries({
        plasma: 'plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml'
    })) {
        const src = read(file);
        assert.match(src, /pollWatchdog\.stop\(\)/, `${name} never disarms the watchdog`);
        // One per poll: status, mullvad, accounts, networks.
        assert.equal((src.match(/root\._pollSettled\(kind\)/g) || []).length, 4,
            `${name} does not check every poll in`);
    }
});

// `panel` is a new object whenever anything it derives from moves, the search
// query included. A QML Repeater handed that array rebuilds every delegate it
// holds, which destroyed the field being typed into on every keystroke. Counted
// models keep the delegates and re-read them by index. GNOME arrives at the same
// place from the other side: it rebuilds only when its panel signature changes.
test('no QML frontend rebuilds its rows just to redraw them', () => {
    for (const [name, file] of Object.entries({
        plasma: 'plasma/org.tailgauge.plasmoid/contents/ui/FullRep.qml',
        omarchy: 'omarchy/arzaroth.tailgauge/Panel.qml'
    })) {
        const src = read(file);
        assert.match(src, /model: (full|root)\.panel\.sections\.length/,
            `${name} hands the sections array to a Repeater`);
        assert.match(src, /\.modelData\.rows\.length : 0/,
            `${name} hands the rows array to a Repeater`);
        assert.match(src, /\.children\.length : 0/,
            `${name} hands the children array to a Repeater`);
    }
    // The action and copy-option repeaters inside a row stay array-bound on
    // purpose: a handful of buttons, nothing focusable, nothing to preserve.
});

// Whatever a frontend does to redraw, the field someone is typing into has to
// come out the other side.
test('every frontend keeps its search field across a redraw', () => {
    for (const file of ['plasma/org.tailgauge.plasmoid/contents/ui/PanelRowView.qml',
                        'omarchy/arzaroth.tailgauge/Panel.qml'])
        assert.match(read(file), /function syncRegistration\(\)/,
            `${file} registers rows by component lifetime, which no longer tracks the row`);

    // GNOME rebuilds its menu outright, so it hides the rows a query excludes
    // rather than rebuilding around the entry, and gates a rebuild on the
    // signature - which the query is no longer an input to.
    const gnome = read('gnome/tailgauge@arzaroth.github.io/extension.ts');
    assert.match(gnome, /this\._applySearch\(\)/);
    assert.match(gnome, /signature !== this\._signature/);
});

// The model says what a row is findable by and hands it over as `searchKey`.
// A frontend that builds a haystack of its own is a fourth search behaviour,
// and it is the one the Rust port cannot follow across the process boundary.
test('no frontend assembles its own search haystack', () => {
    for (const [name, source] of frontends) {
        assert.match(source, /\bsearchKey\b/, `${name} never reads searchKey`);
        for (const field of ['DisplayName', 'HostName', 'DNSName', 'UserName', 'City', 'Country'])
            assert.equal(new RegExp(`\\.${field}\\b`).test(source), false,
                `${name} reads ${field} to search on; searchKey already carries it`);
    }
});

// The query changing must not change what resolvePanel is handed, or the panel
// is rebuilt on every keystroke - which is the whole reason the test above
// exists.
test('no frontend feeds its search query back into resolvePanel', () => {
    for (const [name, source] of frontends) {
        const start = source.indexOf('Model.resolvePanel(');
        const options = source.slice(start, source.indexOf('})', start));
        assert.equal(/machineQuery|mullvadQuery/.test(options), false,
            `${name} resolves the panel against the query it is typing`);
    }
});

test('no frontend gates a control on background work', () => {
    for (const [name, source] of frontends) {
        assert.equal(/enabled:\s*!.*busy|setSensitive\(.*busy/.test(source), false,
            `${name} disables a control while busy; resolvePanel decides that`);
    }
});

test('every frontend tells the service when the panel is on screen', () => {
    assert.match(plasmaSource + read('plasma/org.tailgauge.plasmoid/contents/ui/main.qml'), /attentive/);
    assert.match(gnomeSource, /attentive/);
    assert.match(omarchySource, /attentive/);
});

test('the shared model owns the layout vocabulary', () => {
    const model = read('shared/model.ts');
    for (const word of ['sections', 'navigation', 'visible', 'rows', 'empty'])
        assert.match(model, new RegExp(`\\b${word}\\b`));
});

// A provider binary named in a service is a command that will one day be fired
// at the wrong daemon. The argv belongs to the registry, which is the only
// place that knows which CLI is active.
test('no service hardcodes a provider binary in its argv', () => {
    const services = {
        plasma: 'plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml',
        gnome: 'gnome/tailgauge@arzaroth.github.io/provider.ts',
        omarchy: 'omarchy/arzaroth.tailgauge/Service.qml'
    };
    for (const [name, file] of Object.entries(services)) {
        for (const line of read(file).split('\n')) {
            // `tailscale set --operator` stays spelled out: it is a Tailscale
            // concept with no counterpart to generalise toward, and the two QML
            // frontends need it as a shell string for $(id -un).
            if (line.includes('--operator')) continue;
            assert.doesNotMatch(line, /['"](tailscale|netbird)['"]\s*,/,
                `${name} names a provider binary directly: ${line.trim()}`);
        }
    }
});

// The three snapshots agreeing with each other is not enough: they agreed
// while all three were missing the same fields, because an insertion landed in
// a neighbouring object literal. The resolver's own input type is the
// authority on what a snapshot owes it.
test('every service still on the model hands resolvePanel every field it reads', () => {
    const model = read('shared/model.ts');
    const block = model.slice(model.indexOf('export interface PanelState {'));
    const declared = [...block.slice(0, block.indexOf('\n}')).matchAll(/^\s*(\w+)\??:/gm)]
        .map(m => m[1]);
    assert.ok(declared.length > 20, 'PanelState was not found');

    for (const [name, file] of Object.entries({
        plasma: 'plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml',
        gnome: 'gnome/tailgauge@arzaroth.github.io/provider.ts'
    })) {
        const src = read(file);
        const body = src.split('snapshot()')[1] ?? '';
        const start = body.indexOf('return {');
        const fields = new Set([...body.slice(start, body.indexOf('}', start))
            .matchAll(/^\s*(\w+):/gm)].map(m => m[1]));
        const missing = declared.filter(f => !fields.has(f));
        assert.deepEqual(missing, [], `${name} never sends ${missing.join(', ')}`);
    }
});

// A control the model puts on the header has to appear on every desktop, or
// the panels differ in what you can do rather than in how it looks.
test('every frontend draws the header controls the model hands it', () => {
    for (const [name, source] of frontends)
        assert.match(source, /header\.actions/,
            `${name} never reads the header's actions`);
});
