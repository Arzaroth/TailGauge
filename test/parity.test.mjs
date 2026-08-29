import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {fileURLToPath} from 'node:url';

// The parity rule, enforced rather than remembered: shared/model.js decides what
// the panel contains, and a frontend decides only how a row looks. These tests
// fail when a layout decision leaks back into one desktop, which is how the two
// would start to disagree.

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');

const plasma = ['plasma/org.tailgauge.plasmoid/contents/ui/FullRep.qml',
                'plasma/org.tailgauge.plasmoid/contents/ui/PanelRowView.qml',
                'plasma/org.tailgauge.plasmoid/contents/ui/CompactRep.qml'];
const gnome = ['gnome/tailgauge@arzaroth.github.io/extension.js'];

const plasmaSource = plasma.map(read).join('\n');
const gnomeSource = gnome.map(read).join('\n');

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

test('no frontend re-derives which sections are visible', () => {
    for (const [name, source] of [['plasma', plasmaSource], ['gnome', gnomeSource]]) {
        assert.equal(/showConnections|showExitNodes|showPeers/.test(source), false,
            `${name} computes section visibility; resolvePanel already did`);
    }
});

test('no frontend rebuilds the exit-node or copy-option lists', () => {
    for (const [name, source] of [['plasma', plasmaSource], ['gnome', gnomeSource]]) {
        assert.equal(/displayExitNodes|_displayExitNodes/.test(source), false,
            `${name} assembles its own exit-node list`);
        assert.equal(/copyOptions\s*=\s*\[|peerCopyOptions\s*\(/.test(source), false,
            `${name} assembles its own copy options`);
    }
});

test('no frontend re-derives the status precedence', () => {
    for (const [name, source] of [['plasma', plasmaSource], ['gnome', gnomeSource]]) {
        assert.equal(/actionStatus\s*!==\s*['"]{2}\s*\?/.test(source), false,
            `${name} ranks actionStatus against lastError; panelStatus already did`);
    }
});

test('both frontends read the panel through resolvePanel', () => {
    assert.match(plasmaSource, /Model\.resolvePanel\(/);
    assert.match(gnomeSource, /Model\.resolvePanel\(/);
});

test('both services hand resolvePanel the same snapshot shape', () => {
    const service = read('plasma/org.tailgauge.plasmoid/contents/ui/TailscaleService.qml');
    const gjs = read('gnome/tailgauge@arzaroth.github.io/tailscale.js');
    const fields = src => {
        const body = src.split('snapshot()')[1] ?? '';
        const block = body.slice(0, body.indexOf('\n    }'));
        return [...block.matchAll(/^\s*(\w+):/gm)].map(m => m[1]).sort();
    };
    const plasmaFields = fields(service);
    const gnomeFields = fields(gjs);
    assert.ok(plasmaFields.length > 15, 'the Plasma snapshot was not found');
    assert.deepEqual(plasmaFields, gnomeFields,
        'the two snapshots disagree, so the two panels can disagree');
});

test('the shared model owns the layout vocabulary', () => {
    const model = read('shared/model.js');
    for (const word of ['sections', 'navigation', 'visible', 'rows', 'empty'])
        assert.match(model, new RegExp(`\\b${word}\\b`));
});
