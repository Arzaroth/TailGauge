import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {fileURLToPath} from 'node:url';

import Gio from './mocks/Gio.js';
import GLib from './mocks/GLib.js';
import {ProviderService} from '../../build/tailgauge@arzaroth.github.io/provider.js';

// The GNOME service, driven with a recorded panel. Nothing is spawned: the
// Gio mock hands back a command and a test answers it, so what is covered is
// what the extension does with an answer - the argv it builds, the JSON it
// reads back, and what it hands the binary on the next call.

const repo = path.join(path.dirname(fileURLToPath(import.meta.url)), '../..');
const PANEL = fs.readFileSync(path.join(repo, 'tests/qml/fixtures/panel.json'), 'utf8');

/// A service with its first call already answered, unless told otherwise.
function service({settings = {}, answerFirst = true} = {}) {
    Gio.__reset();
    GLib.__reset();
    const config = new Gio.Settings(settings);
    const svc = new ProviderService(config, '9.9.9');
    if (answerFirst)
        Gio.__find('panel')?.answer(PANEL, '', 0);
    return {svc, config};
}

/// The `--ui` blob of the most recent panel call.
function askedWith() {
    const proc = Gio.__find('panel');
    assert.ok(proc, 'no panel call was made');
    return JSON.parse(proc.real[proc.real.length - 1]);
}

test('the service asks for a panel as soon as it exists', () => {
    const {svc} = service({answerFirst: false});
    const proc = Gio.__find('panel');
    assert.ok(proc, 'nothing was asked');
    assert.deepEqual(proc.real.slice(0, 3), ['tailgauge', 'panel', '--json']);
    assert.equal(svc.panel.status.text, 'Checking…', 'and says so until it is answered');
    assert.equal(svc.installed, false);
});

test('what it asks with is the state only this side knows', () => {
    service({settings: {'active-provider': 'netbird'}, answerFirst: false});
    const ui = askedWith();
    assert.equal(ui.activeProviderId, 'netbird');
    assert.equal(ui.version, '9.9.9');
    assert.equal(ui.active, undefined, 'no optimistic override until something is clicked');
    assert.equal(ui.busy, false);
});

test('an answer lands, and the derived properties follow it', () => {
    const {svc} = service();
    assert.equal(svc.panel.header.title, 'workstation');
    assert.equal(svc.installed, true);
    assert.equal(svc.active, true);
    assert.equal(svc.statusText, '');
    assert.equal(svc.panel.sections.length, 7);
    assert.ok(svc.panel.navigation.length > 1);
});

test('output that is not a panel is reported, and the last good one stays', () => {
    const {svc} = service();
    svc.refresh();
    Gio.__find('panel').answer('not json at all', '', 0);
    assert.match(svc.lastError, /could not be read/);
    assert.equal(svc.panel.header.title, 'workstation', 'the panel on screen is not thrown away');

    svc.refresh();
    Gio.__find('panel').answer('', 'tailgauge: boom', 1);
    assert.equal(svc.lastError, 'tailgauge: boom', 'a failure says what the binary said');
});

test('a click shows before the daemon has agreed to it', () => {
    const {svc} = service();
    assert.equal(svc.active, true);

    svc.down();
    assert.equal(svc.active, false, 'the switch moves on the click, not a round trip later');
    assert.equal(svc.panel.header.dimmed, true);
    assert.equal(askedWith().active, false, 'and the override is carried to the binary');

    const ctl = Gio.__find('ctl');
    assert.ok(ctl, 'nothing was run');
    assert.deepEqual(ctl.real, ['tailgauge', 'ctl', 'down'], 'no provider chosen, so none named');
});

test('the optimistic override clears once the daemon agrees', () => {
    const {svc} = service();
    svc.down();
    assert.equal(svc._desired, 0);

    const off = JSON.parse(PANEL);
    off.header.toggleChecked = false;
    Gio.__find('panel').answer(JSON.stringify(off), '', 0);
    assert.equal(svc._desired, -1, 'the daemon caught up, so stop overriding it');
    assert.equal(svc.active, false);
});

test('a patched panel keeps the sections array it was handed', () => {
    // The menu rebuilds on a signature over the rows; a click that replaced
    // the array would rebuild every row, and take the search entry with it.
    const {svc} = service();
    const before = svc.panel.sections;
    svc.down();
    assert.equal(svc.panel.sections, before, 'same array, so nothing downstream rebuilds');
});

test('every command names the provider being shown', () => {
    const {svc} = service({settings: {'active-provider': 'netbird'}});
    svc.down();
    assert.deepEqual(Gio.__find('ctl').real,
        ['tailgauge', 'ctl', '--provider', 'netbird', 'down']);
});

test('turning it on says so while it is happening', () => {
    const {svc} = service();
    svc._desired = -1;
    svc.loginOrUp();
    assert.equal(svc.active, true);
    assert.equal(svc.actionStatus, 'Turning it on…');
    assert.deepEqual(Gio.__find('ctl').real.slice(-1), ['up']);
});

test('a row hands its own payload back rather than an address', () => {
    const {svc} = service();
    const peer = {id: 'mullvad-region:France\nParis', Mullvad: true, DNSName: 'fr.mullvad.ts.net'};
    svc.setExitNode(peer);

    const ctl = Gio.__find('ctl');
    assert.deepEqual(ctl.real.slice(-3, -1), ['exit-node', '--peer']);
    assert.deepEqual(JSON.parse(ctl.real[ctl.real.length - 1]), peer);
    assert.equal(svc.settingExitNodeId, peer.id, 'and the row it belongs to says it is busy');
});

test('joining and leaving a network are the same command with a flag', () => {
    const {svc} = service();
    svc.selectNetwork({id: 'office', selected: false});
    const joining = Gio.__find('ctl');
    assert.deepEqual(joining.real.slice(-2), ['select-network', 'office']);

    // Answered, because a second one while the first is in flight is refused -
    // which is the point of the test below this.
    joining.answer('', '', 0);
    Gio.__reset();
    svc.selectNetwork({id: 'office', selected: true});
    assert.deepEqual(Gio.__find('ctl').real.slice(-3), ['select-network', 'office', '--leave']);
});

test('a command that failed says why, and clears what it was waiting on', () => {
    const {svc} = service();
    svc.switchAccount('bbbb');
    assert.equal(svc.switchingAccountId, 'bbbb');

    Gio.__find('ctl').answer('', 'tailscale: nope', 1);
    assert.equal(svc.switchingAccountId, '', 'the row stops saying it is busy');
    assert.equal(svc.lastError, 'tailscale: nope');
    assert.equal(svc.actionStatus, '');
});

test('a command that worked clears the error it might have set', () => {
    const {svc} = service();
    svc.lastError = 'something earlier';
    svc.switchAccount('bbbb');
    Gio.__find('ctl').answer('', '', 0);
    assert.equal(svc.lastError, '');
});

test('switching provider blanks the panel rather than showing the old one', () => {
    const {svc, config} = service();
    assert.equal(svc.panel.header.title, 'workstation');

    svc.switchProvider({id: 'netbird'});
    assert.equal(svc.activeProviderId, 'netbird');
    assert.equal(config.values['active-provider'], 'netbird', 'and it is remembered');
    assert.equal(svc.panel.header.title, 'TailGauge', 'the old provider’s panel is not NetBird’s');
    assert.equal(askedWith().activeProviderId, 'netbird');

    // Switching to the one already shown does nothing at all.
    Gio.__reset();
    svc.switchProvider({id: 'netbird'});
    assert.equal(Gio.__find('panel'), null);
    svc.switchProvider(null);
    assert.equal(Gio.__find('panel'), null);
});

test('a chosen region is remembered, capped and deduplicated', () => {
    const {svc, config} = service({settings: {'recent-mullvad-regions': ['Germany\nBerlin']}});
    svc.rememberMullvadRegion({id: 'mullvad-region:France\nParis'});
    assert.deepEqual(config.values['recent-mullvad-regions'],
        ['France\nParis', 'Germany\nBerlin']);

    // A row that is not a region leaves the list alone.
    svc.rememberMullvadRegion({id: 'peer:laptop'});
    assert.deepEqual(config.values['recent-mullvad-regions'],
        ['France\nParis', 'Germany\nBerlin']);
});

test('the update check is cached unless forced, and refreshes the panel', () => {
    const {svc} = service();
    svc.checkUpdate();
    assert.deepEqual(Gio.__find('--check-update').real, ['tailgauge', '--check-update']);

    Gio.__find('--check-update').answer('{}', '', 0);
    Gio.__reset();
    svc.checkUpdate(true);
    assert.deepEqual(Gio.__find('--check-update').real,
        ['tailgauge', '--check-update', '--force']);
});

test('applying an update says so, and stops saying it either way', () => {
    const {svc} = service();
    svc.applyUpdate();
    assert.equal(svc.updating, true);
    assert.equal(svc.actionStatus, 'Updating TailGauge…');
    assert.deepEqual(Gio.__find('--update').real, ['tailgauge', '--update']);

    // A second click while one is running is not a second update.
    const before = Gio.__spawned().length;
    svc.applyUpdate();
    assert.equal(Gio.__spawned().length, before);

    Gio.__find('--update').answer('', 'it broke', 1);
    assert.equal(svc.updating, false);
    assert.equal(svc.actionStatus, '');
    assert.equal(svc.lastError, 'it broke');
});

test('only one of each kind of command runs at a time', () => {
    const {svc} = service();
    svc.switchAccount('aaaa');
    const first = Gio.__spawned().length;
    svc.switchAccount('bbbb');
    assert.equal(Gio.__spawned().length, first, 'the second click waits for the first');
    assert.equal(svc.switchingAccountId, 'aaaa');
});

test('busy is what has a command in flight', () => {
    const {svc} = service();
    assert.equal(svc.busy, false);
    svc.switchAccount('aaaa');
    assert.equal(svc.busy, true);
    Gio.__find('ctl').answer('', '', 0);
    assert.equal(svc.busy, false);
});

test('the poll runs faster while the menu is open', () => {
    const {svc} = service();
    const armedAt = ms => GLib.__armed().some(t => t.interval === ms);
    // The configured interval while idle; three seconds with the menu open.
    assert.ok(armedAt(30000), `idle poll missing from ${JSON.stringify(GLib.__armed())}`);

    svc.attentive = true;
    assert.ok(armedAt(3000), `attentive poll missing from ${JSON.stringify(GLib.__armed())}`);
    assert.ok(Gio.__find('panel'), 'and it asks at once rather than waiting for the tick');

    svc.attentive = false;
    assert.ok(armedAt(30000), 'and it slows down again when the menu closes');
});

test('a watcher that reports a change refreshes; a broken one backs off', () => {
    const {svc} = service();
    const watch = Gio.__find('watch');
    assert.ok(watch, 'nothing is watching');

    Gio.__reset();
    watch.answer('', '', 0);
    assert.ok(Gio.__find('panel'), 'a change on the bus is worth redrawing for');
    assert.ok(GLib.__armed().some(t => t.interval === 250), 'and it re-arms promptly');

    GLib.__reset();
    Gio.__reset();
    svc._watch();
    Gio.__find('watch').answer('', '', 7);
    assert.ok(GLib.__armed().some(t => t.interval === 30000),
        'a watcher that is neither a change nor a timeout is backed off');
});

test('destroy cancels what is in flight and disarms every clock', () => {
    const {svc} = service();
    svc.switchAccount('aaaa');
    const inflight = Gio.__find('ctl');

    svc.destroy();
    assert.equal(inflight.cancellable.cancelled, true);
    assert.deepEqual(GLib.__armed(), [], 'nothing is left ticking');

    // An answer arriving after destroy changes nothing.
    const before = svc.panel.header.title;
    inflight.answer('', 'too late', 1);
    assert.equal(svc.panel.header.title, before);
});
