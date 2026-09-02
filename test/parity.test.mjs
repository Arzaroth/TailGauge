import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {fileURLToPath} from 'node:url';

// The parity rule, enforced rather than remembered: shared/model.js decides what
// the panel contains, and a frontend decides only how a row looks. These tests
// fail when a layout decision leaks back into one desktop, which is how the
// three would start to disagree.

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');

const plasma = ['plasma/org.tailgauge.plasmoid/contents/ui/FullRep.qml',
                'plasma/org.tailgauge.plasmoid/contents/ui/PanelRowView.qml',
                'plasma/org.tailgauge.plasmoid/contents/ui/CompactRep.qml'];
const gnome = ['gnome/tailgauge@arzaroth.github.io/extension.js'];
const omarchy = ['omarchy/arzaroth.tailgauge/Panel.qml'];

const plasmaSource = plasma.map(read).join('\n');
const gnomeSource = gnome.map(read).join('\n');
const omarchySource = omarchy.map(read).join('\n');

const frontends = [['plasma', plasmaSource], ['gnome', gnomeSource], ['omarchy', omarchySource]];

// Rows GNOME shows that Plasma puts in the applet context menu instead. Both
// are desktop conventions, not panel content, so they are allowed to differ.
const DESKTOP_ONLY = new Set(['Refresh', 'Settings']);

const plasmaStrings = new Set([...plasmaSource.matchAll(/i18n\("([^"]{4,})"\)/g)].map(m => m[1]));
const gnomeStrings = new Set([...gnomeSource.matchAll(/\b_\('([^']{4,})'\)/g)].map(m => m[1]));

test('no user-visible string is written in both frontends', () => {
    const shared = [...plasmaStrings].filter(s => gnomeStrings.has(s));
    assert.deepEqual(shared, [],
        `these belong in shared/model.js, not in each frontend: ${shared.join(', ')}`);
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

test('every frontend reads the panel through resolvePanel', () => {
    for (const [name, source] of frontends)
        assert.match(source, /Model\.resolvePanel\(/, `${name} does not resolve the panel`);
});

test('every service hands resolvePanel the same snapshot shape', () => {
    const services = {
        plasma: 'plasma/org.tailgauge.plasmoid/contents/ui/TailscaleService.qml',
        gnome: 'gnome/tailgauge@arzaroth.github.io/tailscale.js',
        omarchy: 'omarchy/arzaroth.tailgauge/Service.qml'
    };
    // The object literal is flat, so its first closing brace ends the field
    // list whatever the file indents with.
    const fields = src => {
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
test('neither QML service reports unchanged state as a change', () => {
    const services = {
        plasma: 'plasma/org.tailgauge.plasmoid/contents/ui/TailscaleService.qml',
        omarchy: 'omarchy/arzaroth.tailgauge/Service.qml'
    };
    for (const [name, file] of Object.entries(services)) {
        const src = read(file);
        assert.match(src, /function _stable\(/, `${name} never compares before it assigns`);
        for (const field of ['selfPeer', 'peers', 'tailnetExitNodes', 'mullvadRegions', 'accounts'])
            assert.match(src, new RegExp(`\\b${field} = _stable\\(`),
                `${name} reassigns ${field} unconditionally, rebuilding the panel on every poll`);
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
    const model = read('shared/model.js');
    for (const word of ['sections', 'navigation', 'visible', 'rows', 'empty'])
        assert.match(model, new RegExp(`\\b${word}\\b`));
});
