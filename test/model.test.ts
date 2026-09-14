import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {root, testDir} from './paths.js';

import type * as ModelTypes from '../shared/model.js';

const built = path.join(root, 'build', 'tailgauge@arzaroth.github.io', 'model.js');

if (!fs.existsSync(built))
    throw new Error('run scripts/build.sh before the tests: the ES module build is missing');

// The tests run against the artifact the build ships, typed by the source it
// was compiled from.
const M = await import(built) as typeof ModelTypes;
const fixture = (name: string): string => fs.readFileSync(path.join(testDir, 'fixtures', name), 'utf8');

const status = parsedStatus(fixture('status.json'));
const accounts = M.parseAccounts(fixture('accounts.json'));
const mullvadNodes = M.parseExitNodeList(fixture('exit-nodes.txt'));
const mullvadRegions = M.mullvadRegionOptions(mullvadNodes);

function parsedStatus(raw: string): ModelTypes.StatusOk {
    const parsed = M.parseStatus(raw);
    assert.ok(parsed.ok && !parsed.unavailable, 'the fixture should parse as a running tailnet');
    return parsed;
}

// A find() that fails the test rather than handing the assertions an undefined.
function only<T>(values: T[], predicate: (value: T) => boolean, what: string): T {
    const found = values.find(predicate);
    assert.ok(found, `no ${what} in ${JSON.stringify(values.map(v => (v as {id?: string}).id))}`);
    return found;
}

function state(overrides: Partial<ModelTypes.PanelState> = {}): ModelTypes.PanelState {
    return {
        installed: true,
        running: status.running,
        active: status.running,
        needsLogin: status.needsLogin,
        busy: false,
        selfName: status.selfName,
        selfIp: status.selfIp,
        selfUserId: status.selfUserId,
        selfPeer: status.selfPeer,
        fileSharing: status.fileSharing,
        peers: status.peers,
        ownExitNodes: status.exitNodes,
        mullvadRegions,
        accounts: accounts.accounts,
        selectedAccountId: accounts.selectedAccountId,
        switchingAccountId: '',
        settingExitNodeId: '',
        accountsAccessDenied: false,
        actionStatus: '',
        lastError: '',
        ...overrides,
    };
}

const section = (panel: ModelTypes.Panel, id: string): ModelTypes.PanelSection =>
    only(panel.sections, s => s.id === id, `section ${id}`);

// Past the threshold the machines section grows a search field, which the
// fixture's four peers are deliberately too few to trigger.
const manyPeers: ModelTypes.Peer[] = Array.from({length: 12}, (_, i) => ({
    id: `peer-${i}`,
    HostName: `box-${i}`,
    DisplayName: `box-${i}`,
    DNSName: `box-${i}.example.ts.net`,
    UserID: status.selfUserId,
    TaildropTarget: 1,
    IPv4: [`100.64.1.${i}`],
    IPv6: [],
    Online: true,
    OS: i % 2 === 0 ? 'linux' : 'windows',
    Tags: [],
    ExitNodeOption: false,
    ExitNode: false,
    Mullvad: false,
}));

// ---- parsing --------------------------------------------------------------

test('parseStatus reads the running tailnet', () => {
    assert.equal(status.ok, true);
    assert.equal(status.unavailable, false);
    assert.equal(status.running, true);
    assert.equal(status.selfName, 'workstation');
    assert.equal(status.selfIp, '100.64.0.1');
    assert.equal(status.selfUserId, '1001');
    assert.equal(status.fileSharing, true);
    assert.equal(status.selfPeer.DNSName, 'workstation.example.ts.net');
    assert.equal(status.selfPeer.UserName, 'Alice');
    assert.deepEqual(status.selfPeer.IPv6, ['fd7a:115c:a1e0::1']);
});

test('parseStatus keeps every non-Mullvad peer, online ones first', () => {
    assert.deepEqual(status.peers.map(p => p.HostName), ['laptop', 'phone', 'router', 'offline-box']);
    assert.equal(only(status.peers, p => p.HostName === 'offline-box', 'offline-box peer').Online, false);
});

test('parseStatus separates Tailscale IPv4 from IPv6', () => {
    const laptop = only(status.peers, p => p.HostName === 'laptop', 'laptop peer');
    assert.deepEqual(laptop.IPv4, ['100.64.0.2']);
    assert.deepEqual(laptop.IPv6, ['fd7a:115c:a1e0::2']);
});

test('parseStatus collects exit node options', () => {
    assert.deepEqual(status.exitNodes.map(n => n.HostName), ['router']);
});

test('parseStatus survives empty and malformed input', () => {
    assert.deepEqual(M.parseStatus(''), {ok: true, unavailable: true, message: 'Disconnected'});
    assert.equal(M.parseStatus('{not json').ok, false);
});

test('parseAccounts finds the selected profile', () => {
    assert.equal(accounts.accounts.length, 2);
    assert.equal(accounts.selectedAccountId, 'aaaa');
    assert.equal(accounts.selectedAccountLabel, 'work');
    assert.deepEqual(M.parseAccounts('nope'), {accounts: [], selectedAccountId: '', selectedAccountLabel: ''});
});

test('parseExitNodeList reads the fixed-width table and drops non-Mullvad hosts', () => {
    assert.equal(mullvadNodes.length, 6);
    assert.equal(mullvadNodes.every(n => n.Mullvad), true);
    assert.deepEqual(mullvadNodes.filter(n => n.ExitNode).map(n => n.City), ['Paris']);
    assert.equal(M.parseExitNodeList('no table here').length, 0);
});

test('mullvadRegionOptions drops the "Any" city and dedupes', () => {
    assert.deepEqual(mullvadRegions.map(r => r.DisplayName),
        ['Vienna, Austria', 'Brussels, Belgium', 'Marseille, France', 'Paris, France', 'Berlin, Germany']);
});

test('recent Mullvad regions dedupe and put the active one first', () => {
    let recent: string[] = [];
    for (const region of ['France\nParis', 'Germany\nBerlin', 'France\nParis'])
        recent = M.pushRecentMullvad(recent, region, 5);
    assert.deepEqual(recent, ['France\nParis', 'Germany\nBerlin']);
    assert.deepEqual(M.recentMullvadNodes(mullvadRegions, recent, 5).map(r => r.City), ['Paris', 'Berlin']);
});

test('taildrop targets follow the daemon grading', () => {
    const byName = Object.fromEntries(status.peers.map(p => [p.HostName, p]));
    assert.equal(M.isTaildropTarget(byName.laptop, '1001'), true);
    assert.equal(M.isTaildropTarget(byName.router, '1001'), false);
    assert.equal(M.isTaildropTarget(byName.phone, '1001'), false);
});

test('the machine subtitle spends the second slot on the owner', () => {
    assert.equal(M.peerSubtitle({IPv4: ['100.64.0.9'], DNSName: 'box.example.ts.net', UserName: 'bob@example.com'}),
        '100.64.0.9 · bob@example.com');
    // No User map, an older daemon: the DNS name still holds the second slot.
    assert.equal(M.peerSubtitle({IPv4: ['100.64.0.9'], DNSName: 'box.example.ts.net'}),
        '100.64.0.9 · box.example.ts.net');
});

test('peers carry the owner the status map names', () => {
    const byName = Object.fromEntries(status.peers.map(p => [p.HostName, p]));
    assert.equal(byName.laptop.UserName, 'Alice');
    assert.equal(byName.phone.UserName, 'Bob');
    assert.equal(M.userLabel({ID: 3003, LoginName: 'tagged-devices'}), 'tagged-devices');
    assert.equal(M.peerOwner({UserID: 4004}, {1001: 'alice@example.com'}), '');
    assert.equal(M.userLabel({ID: 7}), '7');
});

test('shell quoting survives an apostrophe', () => {
    assert.equal(M.shellQuote("a'b"), "'a'\\''b'");
    assert.equal(M.shellCommand(['tailscale', 'set', '--exit-node=']), "'tailscale' 'set' '--exit-node='");
});

// ---- the parity rule ------------------------------------------------------
//
// resolvePanel is the single source of what the panel contains. These lock in
// the contract both frontends read, so a change that would show up on one
// desktop and not the other fails here first.

test('sections come back in a fixed order', () => {
    const panel = M.resolvePanel(state(), {});
    assert.deepEqual(panel.sections.map(s => s.id),
        ['update', 'providers', 'self', 'connections', 'exitNodes', 'networks', 'machines']);
});

test('this device carries the same copy options a machine row does', () => {
    const self = section(M.resolvePanel(state(), {}), 'self');
    assert.equal(self.visible, true);
    assert.equal(self.rows.length, 1);
    const row = self.rows[0];
    assert.equal(row.id, 'self');
    assert.equal(row.label, 'workstation');
    assert.equal(row.sublabel, '100.64.0.1 · Alice');
    assert.deepEqual(row.copyOptions.map(o => o.kind), ['name', 'dns', 'ipv6', 'ip']);
    assert.deepEqual(row.actions.map(a => a.id), ['copy']);
    assert.equal(M.panelRowHasAction(row, 'send'), false);
});

test('this device disappears with the tailnet it belongs to', () => {
    assert.equal(section(M.resolvePanel(state({active: false, running: false}), {}), 'self').visible, false);
    assert.equal(section(M.resolvePanel(state({installed: false}), {}), 'self').visible, false);
    assert.equal(section(M.resolvePanel(state({selfPeer: null}), {}), 'self').visible, false);
});

test('connections appear only with a choice to make', () => {
    assert.equal(section(M.resolvePanel(state(), {}), 'connections').visible, true);
    assert.equal(section(M.resolvePanel(state({accounts: []}), {}), 'connections').visible, false);
    assert.equal(section(M.resolvePanel(state({accounts: [], accountsAccessDenied: true}), {}), 'connections').visible, true);
});

test('the operator prompt leads the connections section', () => {
    const rows = section(M.resolvePanel(state({accountsAccessDenied: true}), {}), 'connections').rows;
    assert.equal(rows[0].kind, 'auth');
    assert.equal(rows[0].action, 'authorize');
});

test('the selected account is the current row', () => {
    const rows = section(M.resolvePanel(state(), {}), 'connections').rows;
    const selected = rows.filter(r => r.current);
    assert.equal(selected.length, 1);
    assert.equal(selected[0].label, 'work');
    assert.equal(selected[0].bold, true);
});

test('exit nodes list the tailnet, then recents, then the picker', () => {
    const rows = section(M.resolvePanel(state(), {recentRegions: ['France\nParis']}), 'exitNodes').rows;
    assert.deepEqual(rows.map(r => r.kind), ['exitNode', 'exitNode', 'mullvadPicker']);
    assert.equal(rows[0].label, 'router');
    assert.equal(rows[1].label, 'Paris, France');
    assert.equal(rows[2].action, 'togglePicker');
});

test('exit nodes hide when Tailscale is down', () => {
    assert.equal(section(M.resolvePanel(state({active: false, running: false}), {}), 'exitNodes').visible, false);
});

test('the active exit node is current and carries the disconnect hint', () => {
    const rows = section(M.resolvePanel(state(), {}), 'exitNodes').rows;
    assert.equal(rows[0].current, true);
    assert.equal(rows[0].hint, 'Disconnect');
    const idle = section(M.resolvePanel(state({ownExitNodes: [{...status.exitNodes[0], ExitNode: false}]}), {}), 'exitNodes').rows;
    assert.equal(idle[0].hint, 'Connect');
});

test('the picker filters its regions and reports an empty result', () => {
    const open = (q: string) => only(
        section(M.resolvePanel(state(), {mullvadQuery: q, mullvadPickerOpen: true}), 'exitNodes').rows,
        r => r.kind === 'mullvadPicker', 'Mullvad picker row');
    assert.deepEqual(open('par').children.map(c => c.label), ['Paris']);
    assert.deepEqual(open('france').children.map(c => c.label), ['Marseille', 'Paris']);
    const none = open('zzz').children;
    assert.equal(none.length, 1);
    assert.equal(none[0].kind, 'empty');
    assert.equal(none[0].navigable, false);
});

test('machine rows carry their subtitle, icon, copy options and actions', () => {
    const rows = section(M.resolvePanel(state(), {}), 'machines').rows;
    const laptop = only(rows, r => r.label === 'laptop', 'laptop row');
    assert.equal(laptop.sublabel, '100.64.0.2 · Alice');
    assert.equal(laptop.icon, 'computer-symbolic');
    assert.deepEqual(laptop.copyOptions.map(o => o.kind), ['name', 'dns', 'ipv6', 'ip']);
    assert.deepEqual(laptop.actions.map(a => a.id), ['send', 'copy']);
});

test('the send action appears only for a Taildrop target', () => {
    const rows = section(M.resolvePanel(state(), {}), 'machines').rows;
    assert.deepEqual(only(rows, r => r.label === 'router', 'router row').actions.map(a => a.id), ['copy']);
    assert.deepEqual(only(rows, r => r.label === 'phone', 'phone row').actions.map(a => a.id), ['copy']);
    const noSharing = section(M.resolvePanel(state({fileSharing: false}), {}), 'machines').rows;
    assert.equal(noSharing.every(r => !r.actions.some(a => a.id === 'send')), true);
});

test('offline machines are listed last, marked, and cannot be sent to', () => {
    const rows = section(M.resolvePanel(state(), {}), 'machines').rows;
    assert.deepEqual(rows.map(r => r.label), ['laptop', 'phone', 'router', 'offline-box']);

    const offline = rows[rows.length - 1];
    assert.equal(offline.sublabel, 'Offline · 100.64.0.5 · Alice');
    assert.deepEqual(offline.actions.map(a => a.id), ['copy']);
    assert.deepEqual(offline.copyOptions.map(o => o.kind), ['name', 'dns', 'ip']);
});

test('the machines section states its own empty case', () => {
    const empty = section(M.resolvePanel(state({peers: []}), {}), 'machines');
    assert.equal(empty.visible, true);
    assert.equal(empty.rows.length, 0);
    assert.equal(empty.empty, 'No machines found on this tailnet.');
});

test('filterMachines matches every field a row shows, and the OS', () => {
    const names = (query: string) => M.filterMachines(status.peers, query).map(p => p.HostName);
    assert.equal(M.filterMachines(status.peers, '').length, status.peers.length);
    assert.deepEqual(names('ANDROID'), ['phone']);
    assert.deepEqual(names('100.64.0.5'), ['offline-box']);
    assert.deepEqual(names('example.ts.net'), status.peers.map(p => p.HostName));
    assert.deepEqual(M.filterMachines(null, 'anything'), []);
});

// Reported as "the search is case sensitive", which it never was: the panel's
// key catcher was swallowing lowercase h, j, k, l and x before they reached the
// field, so the same query typed in capitals arrived intact and the one typed
// normally did not. The filter is pinned here so a future change cannot make
// the complaint true.
test('both searches ignore case, in the query and in what they match', () => {
    const names = (query: string) => M.filterMachines(status.peers, query).map(p => p.HostName);
    assert.deepEqual(names('android'), names('ANDROID'));
    assert.deepEqual(names('android'), names('AnDrOiD'));
    assert.ok(names('android').length > 0, 'the fixture no longer carries an Android peer');

    const cities = (query: string) => M.filterMullvadRegions(mullvadRegions, query).map(r => r.City);
    const anyCity = mullvadRegions[0].City!;
    assert.deepEqual(cities(anyCity.toUpperCase()), cities(anyCity.toLowerCase()));
    assert.ok(cities(anyCity.toUpperCase()).length > 0);
});

test('the machines search appears only for a list long enough to need it', () => {
    assert.equal(section(M.resolvePanel(state(), {}), 'machines').rows.some(r => r.kind === 'machineSearch'), false);

    const long = section(M.resolvePanel(state({peers: manyPeers}), {}), 'machines');
    assert.equal(long.rows[0].kind, 'machineSearch');
    assert.equal(long.rows[0].navigable, false);
    assert.equal(long.rows[0].searchPlaceholder, 'Search machines');

    // Once it is on screen it stays, however few machines the query leaves.
    const few = section(M.resolvePanel(state(), {machineQuery: 'laptop'}), 'machines');
    assert.deepEqual(few.rows.map(r => r.kind), ['machineSearch', 'peer']);
});

test('the machines search filters the rows it leaves behind', () => {
    const labels = (query: string) => section(M.resolvePanel(state({peers: manyPeers}), {machineQuery: query}), 'machines')
        .rows.filter(r => r.kind === 'peer').map(r => r.label);
    assert.deepEqual(labels('100.64.1.7'), ['box-7']);
    assert.deepEqual(labels('BOX-11'), ['box-11']);
    assert.deepEqual(labels('windows'), manyPeers.filter(p => p.OS === 'windows').map(p => p.DisplayName));
});

test('the machines search matches the owner it shows', () => {
    const labels = (query: string) => section(M.resolvePanel(state(), {machineQuery: query}), 'machines')
        .rows.filter(r => r.kind === 'peer').map(r => r.label);
    assert.deepEqual(labels('bob'), ['phone']);
    assert.deepEqual(labels('alice'), ['laptop', 'router', 'offline-box']);
});

test('a search that matches nothing says so instead of looking broken', () => {
    const rows = section(M.resolvePanel(state({peers: manyPeers}), {machineQuery: 'nowhere'}), 'machines').rows;
    assert.deepEqual(rows.map(r => r.kind), ['machineSearch', 'empty']);
    assert.equal(rows[1].label, 'No machines match.');
    assert.equal(rows[1].navigable, false);
});

test('neither the search field nor its empty case is a cursor stop', () => {
    const nav = M.resolvePanel(state({peers: manyPeers}), {machineQuery: 'nowhere'}).navigation.map(n => n.rowId);
    assert.equal(nav.includes('machines:search'), false);
    assert.equal(nav.includes('machines:empty'), false);
});

test('the header reflects every connection state', () => {
    const on = M.resolvePanel(state(), {}).header;
    assert.equal(on.title, 'workstation');
    assert.equal(on.toggleChecked, true);
    assert.equal(on.toggleHint, 'Turn Tailscale off');
    assert.equal(on.crossed, false);

    const off = M.resolvePanel(state({active: false, running: false}), {}).header;
    assert.equal(off.toggleHint, 'Turn Tailscale on');
    assert.equal(off.crossed, true);
    assert.equal(off.dimmed, true);

    const login = M.resolvePanel(state({active: false, running: false, needsLogin: true}), {}).header;
    assert.equal(login.toggleHint, 'Authorize this device');
    assert.equal(login.warning, true);
    assert.equal(login.crossed, false);

    // With nothing installed the panel has no provider to name, so it falls
    // back to the product rather than to whichever provider came first.
    const missing = M.resolvePanel(state({installed: false}), {}).header;
    assert.equal(missing.title, 'TailGauge');
    assert.equal(missing.toggleVisible, false);
});

test('the hero phrase rotates and wraps in both directions', () => {
    const phrase = (i: number) => M.resolvePanel(state(), {phraseIndex: i}).header.meta;
    assert.equal(phrase(0), M.ACTIVE_PHRASES[0]);
    assert.equal(phrase(M.ACTIVE_PHRASES.length), M.ACTIVE_PHRASES[0]);
    assert.equal(phrase(-1), M.ACTIVE_PHRASES[M.ACTIVE_PHRASES.length - 1]);
});

test('status precedence: missing CLI, then progress, then error', () => {
    assert.match(M.resolvePanel(state({installed: false}), {}).status.text, /No supported VPN CLI/);
    const both = M.resolvePanel(state({actionStatus: 'Working', lastError: 'boom'}), {}).status;
    assert.equal(both.text, 'Working');
    assert.equal(both.tone, 'dim');
    const failed = M.resolvePanel(state({lastError: 'boom'}), {}).status;
    assert.equal(failed.text, 'boom');
    assert.equal(failed.tone, 'error');
    assert.equal(M.resolvePanel(state(), {}).status.text, '');
});

test('navigation visits the header then every visible row, in draw order', () => {
    const panel = M.resolvePanel(state(), {recentRegions: ['France\nParis']});
    const ids = panel.navigation.map(n => n.rowId);
    assert.equal(ids[0], 'header');
    const expected = ['header'];
    for (const s of panel.sections) {
        if (!s.visible)
            continue;
        for (const row of s.rows) {
            if (row.navigable)
                expected.push(row.id);
        }
    }
    assert.deepEqual(ids, expected);
});

test('an expanded picker puts its regions in the traversal, a closed one does not', () => {
    const closed = M.resolvePanel(state(), {}).navigation.map(n => n.rowId);
    const open = M.resolvePanel(state(), {mullvadPickerOpen: true}).navigation.map(n => n.rowId);
    assert.equal(closed.some(id => id.startsWith('region:')), false);
    assert.equal(open.filter(id => id.startsWith('region:')).length, mullvadRegions.length);
    assert.equal(open.indexOf('mullvad:add') + 1, open.findIndex(id => id.startsWith('region:')));
});

test('hidden sections contribute no cursor stops', () => {
    const panel = M.resolvePanel(state({installed: false, peers: [], accounts: [], ownExitNodes: [], mullvadRegions: []}), {});
    assert.deepEqual(panel.navigation.map(n => n.rowId), ['header']);
});

test('every cursor stop resolves back to its row', () => {
    const panel = M.resolvePanel(state(), {mullvadPickerOpen: true, recentRegions: ['France\nParis']});
    assert.equal(M.panelRowAt(panel, 0), null, 'the header is not a row');
    for (let i = 1; i < panel.navigation.length; i++) {
        const row = M.panelRowAt(panel, i);
        assert.ok(row, `stop ${i} resolves`);
        assert.equal(row.id, panel.navigation[i].rowId);
        assert.equal(M.panelNavIndexOf(panel, row.id), i);
    }
    assert.equal(M.panelRowAt(panel, panel.navigation.length), null);
    assert.equal(M.panelRowAt(panel, -1), null);
});

test('every row is fully formed, so neither frontend has to fill a gap', () => {
    const panel = M.resolvePanel(state(), {mullvadPickerOpen: true, recentRegions: ['France\nParis']});
    const keys = ['id', 'kind', 'label', 'sublabel', 'icon', 'glyph', 'action', 'current', 'busy',
                  'bold', 'navigable', 'hint', 'actions', 'copyOptions', 'children', 'expanded',
                  'searchPlaceholder', 'payload'];
    const visit = (row: ModelTypes.PanelRow): void => {
        for (const key of keys)
            assert.ok(key in row, `${row.id} is missing ${key}`);
        assert.equal(typeof row.label, 'string');
        assert.equal(Array.isArray(row.actions), true);
        assert.equal(Array.isArray(row.copyOptions), true);
        assert.equal(Array.isArray(row.children), true);
        row.children.forEach(visit);
    };
    for (const s of panel.sections)
        s.rows.forEach(visit);
});

test('the translator reaches every user-visible string', () => {
    const panel = M.resolvePanel(state({installed: false}), {t: s => `«${s}»`});
    assert.match(panel.status.text, /^«.*»$/);
    assert.match(panel.header.toggleHint, /^«.*»$/);
    for (const s of panel.sections) {
        // The update banner carries no header, so it has no title to translate.
        if (s.title === '')
            continue;
        assert.match(s.title, /^«.*»$/);
    }
    const machines = section(M.resolvePanel(state({peers: []}), {t: s => `«${s}»`}), 'machines');
    assert.match(machines.empty, /^«.*»$/);
});

test('a busy row reports which command it is waiting on', () => {
    const switching = section(M.resolvePanel(state({switchingAccountId: 'bbbb'}), {}), 'connections').rows;
    assert.equal(only(switching, r => r.label === 'personal', 'personal account row').busy, true);
    assert.equal(only(switching, r => r.label === 'work', 'work account row').busy, false);

    const settingId = status.exitNodes[0].id;
    const setting = section(M.resolvePanel(state({settingExitNodeId: settingId}), {}), 'exitNodes').rows;
    assert.equal(setting[0].busy, true);
});

// ---- the update banner ----------------------------------------------------

test('the update section is hidden until there is an update', () => {
    const panel = M.resolvePanel(state(), {});
    const update = section(panel, 'update');
    assert.equal(update.visible, false);
    assert.equal(update.rows.length, 0);
    assert.equal(update.title, '', 'one banner needs no section header');
    assert.equal(panel.navigation.some(n => n.rowId === 'update'), false);
});

test('an installable update offers to install itself', () => {
    const panel = M.resolvePanel(state({update: {available: true, updatable: true, latest: '1.1.0'}}), {});
    const row = section(panel, 'update').rows[0];
    assert.equal(section(panel, 'update').visible, true);
    assert.equal(row.kind, 'update');
    assert.equal(row.label, 'TailGauge 1.1.0 is available');
    assert.equal(row.sublabel, 'Install it now');
    assert.equal(row.action, 'update');
    assert.equal(panel.navigation[1].rowId, 'update', 'the banner leads the traversal');
});

test('a store-managed update points at the store instead', () => {
    const row = section(M.resolvePanel(state({update: {available: true, updatable: false, latest: '1.1.0'}}), {}), 'update').rows[0];
    assert.equal(row.sublabel, 'Update it where you installed it from');
    assert.equal(row.action, 'openUrl');
});

test('an update in flight marks the row busy', () => {
    const row = section(M.resolvePanel(state({
        update: {available: true, updatable: true, latest: '1.1.0'}, updating: true,
    }), {}), 'update').rows[0];
    assert.equal(row.busy, true);
});

test('the version substitutes into the translated template', () => {
    const row = section(M.resolvePanel(state({update: {available: true, updatable: true, latest: '2.3.4'}}),
        {t: s => `«${s}»`}), 'update').rows[0];
    assert.equal(row.label, '«TailGauge %1 is available»'.replace('%1', '2.3.4'));
    assert.equal(M.formatText('a %1 b', 'X'), 'a X b');
});

// ---- the footer reports what is installed ---------------------------------

test('the footer carries the version the frontend passed', () => {
    assert.equal(M.resolvePanel(state({version: '1.2.3'}), {}).footer, 'TailGauge v1.2.3');
    assert.equal(M.resolvePanel(state({version: '1.2.3'}), {t: s => `«${s}»`}).footer,
        '«TailGauge v%1»'.replace('%1', '1.2.3'));
});

test('a frontend that knows no version gets no footer', () => {
    assert.equal(M.resolvePanel(state(), {}).footer, '');
    assert.equal(M.resolvePanel(state({version: ''}), {}).footer, '');
});

test('helpers left behind by a half-applied update show next to the widget', () => {
    const targets = [{kind: 'plugin', current: '1.2.3'}, {kind: 'helpers', current: '1.2.2'}];
    assert.equal(M.resolvePanel(state({version: '1.2.3', update: {targets}}), {}).footer,
        'TailGauge v1.2.3 · helpers v1.2.2');
});

test('helpers on the widget version are not worth a second number', () => {
    const targets = [{kind: 'helpers', current: '1.2.3'}];
    assert.equal(M.resolvePanel(state({version: '1.2.3', update: {targets}}), {}).footer,
        'TailGauge v1.2.3');
    assert.equal(M.resolvePanel(state({version: '1.2.3', update: {targets: []}}), {}).footer,
        'TailGauge v1.2.3');
});

// ---- Taildrop needs the helpers, not just the capability -------------------

test('the send action disappears when the helpers are not installed', () => {
    const withHelpers = section(M.resolvePanel(state(), {}), 'machines').rows;
    assert.equal(withHelpers.some(r => r.actions.some(a => a.id === 'send')), true);

    const without = section(M.resolvePanel(state({helpers: false}), {}), 'machines').rows;
    assert.equal(without.some(r => r.actions.some(a => a.id === 'send')), false,
        'a store-installed widget has no tailgauge-send to call');
    assert.equal(without.every(r => r.actions.some(a => a.id === 'copy')), true,
        'copying still works without the helpers');
});

test('canSendFiles agrees with the resolved actions', () => {
    const peer = status.peers.find(p => p.HostName === 'laptop');
    assert.equal(M.canSendFiles(state(), peer), true);
    assert.equal(M.canSendFiles(state({helpers: false}), peer), false);
    assert.equal(M.canSendFiles(state({fileSharing: false}), peer), false);
    assert.equal(M.canSendFiles(state({running: false}), peer), false);
});

// ---- the toggle must never be gated on background work --------------------

test('the switch stays enabled while a background poll runs', () => {
    // A status poll every few seconds would otherwise leave the switch dead
    // most of the time. Upstream shows `busy`; it never enforces it.
    const idle = M.resolvePanel(state(), {}).header;
    const polling = M.resolvePanel(state({busy: true}), {}).header;
    assert.equal(idle.toggleEnabled, true);
    assert.equal(polling.toggleEnabled, true, 'busy must not disable the toggle');
    assert.equal(polling.busy, true, 'but it is still reported, for a spinner');
    assert.equal(idle.busy, false);
});

test('the switch is disabled only when there is no CLI to drive', () => {
    assert.equal(M.resolvePanel(state({installed: false}), {}).header.toggleEnabled, false);
    assert.equal(M.resolvePanel(state({installed: false}), {}).header.toggleVisible, false);
});

// ---------------------------------------------------------------------------
// Providers
// ---------------------------------------------------------------------------

const detected = (...ids: string[]) =>
    M.providerDescriptors().map(p => ({id: p.id, installed: ids.includes(p.id)}));

test('the registry names every provider and the CLI that proves it', () => {
    const ids = M.providerDescriptors().map(p => p.id);
    assert.deepEqual(ids, ['tailscale', 'netbird']);
    assert.deepEqual(M.providerCliNames(), ['tailscale', 'netbird']);
    assert.equal(M.providerById('netbird')!.label, 'NetBird');
    assert.equal(M.providerById('nope'), null);
});

test('a frontend that has not been taught to probe still reports one provider', () => {
    assert.equal(M.activeProvider({installed: true})!.id, 'tailscale');
    assert.equal(M.activeProvider({installed: false}), null);
    assert.equal(M.activeProvider({}), null);
});

test('detection scopes the panel to what is actually installed', () => {
    assert.equal(M.activeProvider({providers: detected('netbird')})!.id, 'netbird');
    assert.equal(M.activeProvider({providers: detected()}), null);
    assert.deepEqual(
        M.installedProviders({providers: detected('tailscale', 'netbird')}).map(p => p.id),
        ['tailscale', 'netbird']);
});

test('with several installed the registry order decides, until a choice is made', () => {
    const both = detected('tailscale', 'netbird');
    assert.equal(M.activeProvider({providers: both})!.id, 'tailscale');
    assert.equal(M.activeProvider({providers: both, activeProviderId: 'netbird'})!.id, 'netbird');
    // Report order must not change the answer.
    assert.equal(M.activeProvider({providers: both.slice().reverse()})!.id, 'tailscale');
});

test('a choice naming a provider that is gone falls back instead of blanking', () => {
    const only = {providers: detected('netbird'), activeProviderId: 'tailscale'};
    assert.equal(M.activeProvider(only)!.id, 'netbird');
});

test('capabilities are read from the active provider, not its name', () => {
    const ts = {providers: detected('tailscale')};
    const nb = {providers: detected('netbird')};
    assert.equal(M.providerSupports(ts, 'mullvad'), true);
    assert.equal(M.providerSupports(nb, 'mullvad'), false);
    assert.equal(M.providerSupports(nb, 'networks'), true);
    assert.equal(M.providerSupports({providers: detected()}, 'exitNodes'), false);
});

test('a provider without a feature never shows the section that needs it', () => {
    const nb = M.resolvePanel(state({providers: detected('netbird')}), {});
    assert.equal(section(nb, 'exitNodes').visible, false,
        'NetBird has no exit nodes, even with tailnet exit nodes in the snapshot');
    assert.equal(section(nb, 'connections').visible, false, 'NetBird has no account switching');

    const ts = M.resolvePanel(state({providers: detected('tailscale')}), {});
    assert.equal(section(ts, 'exitNodes').visible, true);
    assert.equal(section(ts, 'connections').visible, true);
});

test('every provider the registry calls drivable has what driving needs', () => {
    for (const provider of M.providerDescriptors()) {
        if (!provider.supported) continue;
        assert.ok(provider.commands.status.length > 0, `${provider.id} has no status command`);
        assert.ok(provider.commands.up.length > 0, `${provider.id} has no up command`);
        assert.ok(provider.commands.down.length > 0, `${provider.id} has no down command`);
        assert.equal(provider.commands.status[0], provider.cli,
            `${provider.id} polls a binary other than the one detection probes`);
        // A capability with no command behind it is a row that would do nothing.
        if (provider.capabilities.exitNodes) assert.ok(provider.commands.exitNodeList);
        if (provider.capabilities.accounts) assert.ok(provider.commands.accounts);
        if (provider.capabilities.networks)
            assert.ok(provider.commands.networks && provider.commands.selectNetwork);
    }
});

test('nothing installed is never ready, whatever was chosen', () => {
    assert.equal(M.providerReady({providers: detected()}), false);
    assert.equal(M.providerReady({providers: detected(), activeProviderId: 'tailscale'}), false);
    assert.equal(M.providerReady({}), false);
    const panel = M.resolvePanel(state({providers: detected(), installed: false}), {});
    assert.equal(panel.header.toggleEnabled, false);
    assert.match(panel.status.text, /No supported VPN CLI/);
});

test('with both installed the drivable one wins the auto-choice', () => {
    const both = state({providers: detected('tailscale', 'netbird')});
    assert.equal(M.activeProvider(both)!.id, 'tailscale');
    assert.equal(M.providerReady(both), true);
    assert.equal(M.resolvePanel(both, {}).header.title, status.selfName);
});

test('the probe reads which stdout, not its exit code', () => {
    const probe = (out: string) =>
        Object.fromEntries(M.parseProviderProbe(out).map(p => [p.id, p.installed]));
    assert.deepEqual(probe('/usr/bin/tailscale\n/usr/bin/netbird\n'),
        {tailscale: true, netbird: true});
    assert.deepEqual(probe('/usr/bin/netbird\n'), {tailscale: false, netbird: true});
    assert.deepEqual(probe(''), {tailscale: false, netbird: false});
    // A miss reported on stderr must not read as a hit if the streams ever merge.
    assert.deepEqual(probe('which: no netbird in (/usr/bin)\n/usr/bin/tailscale\n'),
        {tailscale: true, netbird: false});
    // Only the basename decides, so a user-local install still counts.
    assert.deepEqual(probe('/home/me/.local/bin/tailscale\n'), {tailscale: true, netbird: false});
});

test('sending files is a provider capability, not just a helper check', () => {
    const peer = only(status.peers, p => M.canSendFiles(state(), p), 'Taildrop target');
    assert.equal(M.canSendFiles(state({providers: detected('netbird')}), peer), false);
    assert.equal(M.canSendFiles(state({providers: detected('tailscale')}), peer), true);
});

test('the panel names the provider it is driving', () => {
    assert.equal(M.providerLabel({providers: detected('netbird')}), 'NetBird');
    assert.equal(M.providerLabel({providers: detected()}), 'TailGauge');
    const nb = M.resolvePanel(state({providers: detected('netbird'), active: false, running: false}), {});
    assert.equal(nb.header.meta, 'NetBird is disconnected');
    assert.equal(nb.header.toggleHint, 'Turn NetBird on');
});

test('the argv comes from the registry, not the caller', () => {
    const ts = M.providerCommands({providers: detected('tailscale')})!;
    assert.deepEqual(ts.status, ['tailscale', 'status', '--json']);
    assert.deepEqual(ts.switchAccount!('work'), ['tailscale', 'switch', 'work']);
    assert.deepEqual(ts.setExitNode!('100.64.0.3'), ['tailscale', 'set', '--exit-node=100.64.0.3']);
    assert.deepEqual(ts.setExitNode!(''), ['tailscale', 'set', '--exit-node=']);

    const nb = M.providerCommands({providers: detected('netbird')})!;
    assert.deepEqual(nb.status, ['netbird', 'status', '--json']);
    assert.deepEqual(nb.selectNetwork!('net1', true), ['netbird', 'networks', 'select', 'net1']);
    assert.deepEqual(nb.selectNetwork!('net1', false), ['netbird', 'networks', 'deselect', 'net1']);
    // NetBird has neither, and the registry says so rather than guessing.
    assert.equal(nb.exitNodeList, undefined);
    assert.equal(nb.accounts, undefined);

    assert.equal(M.providerCommands({providers: detected()}), null);
});

test('loginPlan turns on whatever provider it was handed', () => {
    const up = ['netbird', 'up'];
    assert.deepEqual(M.loginPlan(false, '', up), {authUrl: '', command: up});
    // A pending authorization is a URL to open, whoever the provider is.
    assert.deepEqual(M.loginPlan(true, 'https://login.example/x', up),
        {authUrl: 'https://login.example/x', command: []});
    // No provider, no command to run.
    assert.deepEqual(M.loginPlan(false, '', undefined), {authUrl: '', command: []});
});

// ---------------------------------------------------------------------------
// NetBird
// ---------------------------------------------------------------------------
//
// netbird-status-idle.json is a real `netbird status --json` capture with the
// event log emptied. netbird-status.json is that same envelope with peers and
// a connected daemon filled in, since connecting one would have meant logging
// a real machine into a real network.

const nbStatus = M.parseNetbirdStatus(fixture('netbird-status.json'));
const nbIdle = M.parseNetbirdStatus(fixture('netbird-status-idle.json'));

test('parseNetbirdStatus reads a connected mesh', () => {
    assert.ok(nbStatus.ok && !nbStatus.unavailable);
    assert.equal(nbStatus.running, true);
    assert.equal(nbStatus.needsLogin, false);
    assert.equal(nbStatus.daemonState, 'Connected');
    assert.equal(nbStatus.selfName, 'workstation');
    assert.equal(nbStatus.selfIp, '100.85.0.1', 'the CIDR suffix is not part of the address');
    assert.equal(nbStatus.selfDnsName, 'workstation.netbird.selfhosted');
});

test('parseNetbirdStatus orders peers online first, like the Tailscale one', () => {
    assert.ok(nbStatus.ok && !nbStatus.unavailable);
    assert.deepEqual(nbStatus.peers.map(p => p.HostName), ['laptop', 'nas', 'offline-box']);
    const nas = only(nbStatus.peers, p => p.HostName === 'nas', 'nas peer');
    assert.deepEqual(nas.IPv4, ['100.85.0.2']);
    assert.equal(nas.Online, true);
    assert.equal(nas.ConnectionType, 'P2P');
    assert.equal(nas.LatencyMs, 1.5, 'nanoseconds are reported as milliseconds');
    const off = only(nbStatus.peers, p => p.HostName === 'offline-box', 'offline peer');
    assert.equal(off.Online, false, 'only "Connected" is reachable now');
    assert.equal(off.LatencyMs, -1, 'unmeasured is -1, not 0');
});

test('parseNetbirdStatus offers nothing it cannot do', () => {
    assert.ok(nbStatus.ok && !nbStatus.unavailable);
    assert.deepEqual(nbStatus.exitNodes, [], 'NetBird has no exit nodes');
    assert.equal(nbStatus.fileSharing, false, 'nor Taildrop');
    assert.equal(nbStatus.authUrl, '', 'the login URL arrives on the `up` stream');
    assert.equal(nbStatus.peers.every(p => !p.Mullvad && !p.ExitNodeOption), true);
});

test('parseNetbirdStatus reads the real idle capture', () => {
    assert.ok(nbIdle.ok && !nbIdle.unavailable);
    assert.equal(nbIdle.running, false);
    assert.equal(nbIdle.daemonState, 'Idle');
    assert.deepEqual(nbIdle.peers, [], 'a null details list is no peers, not a crash');
    assert.equal(nbIdle.selfIp, '');
});

test('parseNetbirdStatus grades every login-shaped daemon state', () => {
    const state = (s: string) => {
        const parsed = M.parseNetbirdStatus(JSON.stringify({daemonStatus: s, peers: {details: null}}));
        assert.ok(parsed.ok && !parsed.unavailable);
        return parsed;
    };
    for (const s of ['NeedsLogin', 'SessionExpired', 'LoginFailed'])
        assert.equal(state(s).needsLogin, true, `${s} needs a login`);
    assert.equal(state('Connecting').needsLogin, false);
    assert.equal(state('Connecting').running, false);
    assert.equal(state('Connected').running, true);
});

test('parseNetbirdStatus survives empty and malformed input', () => {
    assert.deepEqual(M.parseNetbirdStatus(''), {ok: true, unavailable: true, message: 'Disconnected'});
    assert.equal(M.parseNetbirdStatus('{not json').ok, false);
    assert.equal(M.parseNetbirdStatus('[]').ok, false);
});

// netbird-networks.txt follows the block format `netbird networks list` prints.
// It is written to that format rather than captured: listing needs a connected
// daemon, which this machine is not.
const nbNetworks = M.parseNetbirdNetworks(fixture('netbird-networks.txt'));

test('parseNetbirdNetworks reads the block format', () => {
    assert.equal(nbNetworks.ok, true);
    assert.deepEqual(nbNetworks.networks.map(n => n.id), ['prod-vpc', 'office-lan', 'dns-only']);
    const prod = only(nbNetworks.networks, n => n.id === 'prod-vpc', 'prod network');
    assert.equal(prod.range, '10.10.0.0/16');
    assert.equal(prod.selected, true);
    const office = only(nbNetworks.networks, n => n.id === 'office-lan', 'office network');
    assert.equal(office.selected, false);
    assert.deepEqual(office.domains, ['office.internal', 'printers.internal']);
    // A "-" is an absent value, not a one-item list.
    assert.deepEqual(prod.domains, []);
    assert.equal(only(nbNetworks.networks, n => n.id === 'dns-only', 'dns network').range, '',
        'a "-" range is absent, not a dash to show');
});

test('parseNetbirdNetworks tells empty apart from broken', () => {
    assert.deepEqual(M.parseNetbirdNetworks(''), {ok: true, networks: [], message: ''});
    assert.deepEqual(M.parseNetbirdNetworks('No networks available.'), {ok: true, networks: [], message: ''});
    const failed = M.parseNetbirdNetworks('Error: failed to list network: not connected');
    assert.equal(failed.ok, false);
    assert.match(failed.message, /not connected/);
});

test('the networks section belongs to the provider that has networks', () => {
    const nbState = {
        providers: detected('netbird'), active: true, running: true,
        networks: nbNetworks.networks,
    };
    const nb = M.resolvePanel(nbState, {});
    const networks = section(nb, 'networks');
    assert.equal(networks.visible, true);
    assert.deepEqual(networks.rows.map(r => r.label), ['prod-vpc', 'office-lan', 'dns-only']);
    assert.equal(networks.rows[0].current, true, 'the selected one is marked');
    assert.equal(networks.rows[0].sublabel, '10.10.0.0/16');
    assert.equal(networks.rows.every(r => r.action === 'selectNetwork'), true);
    // Domains stand in when there is no range.
    assert.equal(only(networks.rows, r => r.label === 'dns-only', 'dns row').sublabel, 'apps.internal');

    // Tailscale has no networks, so the same snapshot shows none.
    const ts = M.resolvePanel({...nbState, providers: detected('tailscale')}, {});
    assert.equal(section(ts, 'networks').visible, false);
});

test('a network being joined reports busy on its own row', () => {
    const panel = M.resolvePanel({
        providers: detected('netbird'), active: true, running: true,
        networks: nbNetworks.networks, selectingNetworkId: 'office-lan',
    }, {});
    const rows = section(panel, 'networks').rows;
    assert.equal(only(rows, r => r.label === 'office-lan', 'office row').busy, true);
    assert.equal(only(rows, r => r.label === 'prod-vpc', 'prod row').busy, false);
});

test('the provider switcher appears only when there is a choice', () => {
    const one = M.resolvePanel(state({providers: detected('tailscale')}), {});
    assert.equal(section(one, 'providers').visible, false, 'one provider is not a choice');
    assert.equal(section(M.resolvePanel(state({providers: detected()}), {}), 'providers').visible, false);

    const both = M.resolvePanel(state({providers: detected('tailscale', 'netbird')}), {});
    const providers = section(both, 'providers');
    assert.equal(providers.visible, true);
    assert.deepEqual(providers.rows.map(r => r.label), ['Tailscale', 'NetBird']);
    assert.deepEqual(providers.rows.map(r => r.current), [true, false]);
    assert.equal(providers.rows.every(r => r.action === 'switchProvider'), true);
});

test('the switcher follows the choice, and the whole panel with it', () => {
    const both = detected('tailscale', 'netbird');
    const picked = M.resolvePanel(state({providers: both, activeProviderId: 'netbird'}), {});
    assert.deepEqual(section(picked, 'providers').rows.map(r => r.current), [false, true]);
    // The title is the device, which a refresh replaces; the provider shows
    // in what the switch says it will do.
    assert.equal(picked.header.toggleHint, 'Turn NetBird off');
    assert.equal(section(picked, 'exitNodes').visible, false);
    assert.equal(section(picked, 'connections').visible, false);
});

test('the switcher never offers a provider it cannot drive', () => {
    const drivable = M.drivableProviders({providers: detected('tailscale', 'netbird')});
    assert.deepEqual(drivable.map(p => p.id), M.providerDescriptors()
        .filter(p => p.supported).map(p => p.id));
    assert.deepEqual(M.drivableProviders({providers: detected()}), []);
});

// ---------------------------------------------------------------------------
// The bar icon and its tooltip
// ---------------------------------------------------------------------------

const summary = (id: string, over: Partial<ModelTypes.ProviderSummary> = {}) => ({
    ...M.summarizeProvider(id, null), ...over,
});

test('summarizeProvider reads a status result, or survives not having one', () => {
    const ts = M.summarizeProvider('tailscale', status);
    assert.equal(ts.label, 'Tailscale');
    assert.equal(ts.running, true);
    assert.equal(ts.selfName, 'workstation');
    assert.equal(ts.selfIp, '100.64.0.1');

    const nb = M.summarizeProvider('netbird', nbIdle);
    assert.equal(nb.label, 'NetBird');
    assert.equal(nb.running, false);

    for (const bad of [null, {ok: false}, {ok: true, unavailable: true}]) {
        const s = M.summarizeProvider('tailscale', bad);
        assert.equal(s.running, false, 'an unusable status is not a connection');
        assert.equal(s.id, 'tailscale', 'but it is still that provider');
    }
});

test('the bar icon describes the machine, not the panel view', () => {
    const both = detected('tailscale', 'netbird');
    const summaries = [summary('tailscale', {running: true, selfName: 'workstation', selfIp: '100.64.0.1'}),
                       summary('netbird', {state: 'Idle'})];
    // Viewing idle NetBird while Tailscale is up must not read as disconnected.
    const viewingNetbird = M.resolvePanel(
        {providers: both, activeProviderId: 'netbird', summaries, active: false}, {});
    assert.equal(viewingNetbird.bar.connected, true, 'Tailscale is still up');
    assert.equal(viewingNetbird.bar.crossed, false);
    // The panel hero still describes what you are looking at.
    assert.equal(viewingNetbird.header.toggleChecked, false, 'NetBird itself is off');

    const viewingTailscale = M.resolvePanel(
        {providers: both, activeProviderId: 'tailscale', summaries, active: true}, {});
    // The tooltip reorders to put the viewed provider first; the icon does not move.
    for (const field of ['connected', 'warning', 'crossed'] as const)
        assert.equal(viewingTailscale.bar[field], viewingNetbird.bar[field],
            `switching the view changed bar.${field}`);
});

test('the bar goes dark only when every provider is down', () => {
    const both = detected('tailscale', 'netbird');
    const down = M.resolvePanel({providers: both,
        summaries: [summary('tailscale'), summary('netbird')]}, {});
    assert.equal(down.bar.connected, false);
    assert.equal(down.bar.crossed, true);

    const pending = M.resolvePanel({providers: both,
        summaries: [summary('tailscale', {needsLogin: true}), summary('netbird')]}, {});
    assert.equal(pending.bar.warning, true);
    assert.equal(pending.bar.crossed, false, 'needing a login is not being crossed out');
});

test('the tooltip says what each installed provider is doing, active first', () => {
    const both = detected('tailscale', 'netbird');
    const summaries = [summary('tailscale', {running: true, selfName: 'workstation', selfIp: '100.64.0.1'}),
                       summary('netbird', {state: 'Idle'})];
    const trimmed = (p: ModelTypes.Panel) => p.bar.tooltip.map(l => l.replace(/\u00a0+$/, ''));
    assert.deepEqual(trimmed(M.resolvePanel({providers: both, summaries}, {})),
        ['Tailscale  connected · workstation · 100.64.0.1', 'NetBird    Idle']);
    // Every line is the same width, so the bar centres them identically.
    const widths = new Set(M.resolvePanel({providers: both, summaries}, {}).bar.tooltip.map(l => l.length));
    assert.equal(widths.size, 1, 'lines of different widths centre differently');
    // The one being viewed leads, whichever it is.
    assert.deepEqual(trimmed(M.resolvePanel({providers: both, activeProviderId: 'netbird', summaries}, {})),
        ['NetBird    Idle', 'Tailscale  connected · workstation · 100.64.0.1']);
    assert.deepEqual(trimmed(M.resolvePanel({providers: both,
        summaries: [summary('tailscale', {needsLogin: true}), summary('netbird')]}, {})),
        ['Tailscale  needs login', 'NetBird    disconnected']);
});

test('the tooltip falls back rather than lying about what it knows', () => {
    assert.deepEqual(M.resolvePanel({providers: detected()}, {}).bar.tooltip,
        ['No supported VPN CLI on PATH. Looked for Tailscale, NetBird.']);
    // Installed but never polled.
    assert.deepEqual(M.resolvePanel({providers: detected('netbird')}, {}).bar.tooltip,
        ['NetBird  checking\u2026']);
});

test('with no summaries the bar still follows the one provider we know about', () => {
    assert.equal(M.resolvePanel(state(), {}).bar.connected, true, 'legacy snapshot, Tailscale up');
    assert.equal(M.resolvePanel(state({active: false}), {}).bar.connected, false);
});

// ---------------------------------------------------------------------------
// Peer detail
// ---------------------------------------------------------------------------

const NOW = Date.parse('2026-09-14T06:00:00Z');

test('both providers report what the expanded row shows', () => {
    const nas = only((nbStatus as ModelTypes.StatusOk).peers, p => p.HostName === 'nas', 'nas');
    assert.equal(nas.ConnectionType, 'P2P');
    assert.equal(nas.RxBytes, 4096);
    assert.equal(nas.TxBytes, 2048);

    // Tailscale says "direct" by having an address rather than by a field.
    const direct = M.peerFromStatus('x', {CurAddr: '1.2.3.4:41641', RxBytes: 10, TxBytes: 20}, {});
    assert.equal(direct.ConnectionType, 'P2P');
    assert.equal(direct.Endpoint, '1.2.3.4:41641');
    const relayed = M.peerFromStatus('x', {CurAddr: '', Relay: 'ams'}, {});
    assert.equal(relayed.ConnectionType, 'Relayed');
    assert.equal(relayed.Relay, 'ams');
    const unknown = M.peerFromStatus('x', {CurAddr: '', Relay: ''}, {});
    assert.equal(unknown.ConnectionType, '', 'no reading is not a guess');
});

test('peerDetailRows shows only what was actually reported', () => {
    const rows = M.peerDetailRows({
        ConnectionType: 'P2P', LatencyMs: 20, Endpoint: '[2001:db8::1]:51820',
        RxBytes: 424, TxBytes: 472, LastHandshake: '2026-09-14T05:59:00Z',
        Routes: ['192.168.42.0/24'],
    }, (x: string) => x, NOW);
    const by = Object.fromEntries(rows.map(r => [r.id, r.sublabel]));
    assert.equal(by['detail:connection'], 'Direct peer-to-peer · 20 ms');
    assert.equal(by['detail:endpoint'], '[2001:db8::1]:51820');
    assert.equal(by['detail:handshake'], '1 minute ago');
    assert.equal(by['detail:transfer'], '↓ 424 B   ↑ 472 B');
    assert.equal(by['detail:routes'], '192.168.42.0/24');
    assert.equal(rows.every(r => r.kind === 'detail' && !r.navigable), true);

    // A peer that reported nothing gets no rows, so no arrow.
    assert.deepEqual(M.peerDetailRows({}, (x: string) => x, NOW), []);
    assert.deepEqual(M.peerDetailRows(null, (x: string) => x, NOW), []);
});

test('a relayed peer names its relay', () => {
    const t = (x: string) => x;
    assert.equal(M.connectionSummary({ConnectionType: 'Relayed', Relay: 'ams'}, t), 'Relayed via ams');
    assert.equal(M.connectionSummary({ConnectionType: '', Relay: 'ams'}, t), 'Relayed via ams');
    assert.equal(M.connectionSummary({ConnectionType: 'P2P'}, t), 'Direct peer-to-peer');
    assert.equal(M.connectionSummary({}, t), '');
});

test('bytes and elapsed time read like a human wrote them', () => {
    assert.equal(M.formatBytes(0), '0 B');
    assert.equal(M.formatBytes(424), '424 B');
    assert.equal(M.formatBytes(1536), '1.5 KB');
    assert.equal(M.formatBytes(15 * 1024 * 1024), '15 MB');
    assert.equal(M.formatSince('2026-09-14T05:59:30Z', NOW), 'just now');
    assert.equal(M.formatSince('2026-09-14T05:59:00Z', NOW), '1 minute ago');
    assert.equal(M.formatSince('2026-09-14T03:00:00Z', NOW), '3 hours ago');
    assert.equal(M.formatSince('', NOW), '');
    assert.equal(M.formatSince('not a date', NOW), '');
});

test('NetBird writes "never" as a zero date, not an absent field', () => {
    const parsed = M.parseNetbirdStatus(JSON.stringify({
        daemonStatus: 'Connected',
        peers: {details: [
            {fqdn: 'a.example', status: 'Connected', lastWireguardHandshake: '0001-01-01T00:00:00Z'},
            {fqdn: 'b.example', status: 'Connected', lastWireguardHandshake: '2026-09-14T05:59:00Z'},
        ]},
    }));
    assert.ok(parsed.ok && !parsed.unavailable);
    assert.equal(only(parsed.peers, p => p.HostName === 'a', 'peer a').LastHandshake, '',
        'a zero date is no handshake, not 2001 years ago');
    assert.notEqual(only(parsed.peers, p => p.HostName === 'b', 'peer b').LastHandshake, '');
});

test('a machine row carries its detail behind a disclosure arrow', () => {
    const peers = (nbStatus as ModelTypes.StatusOk).peers;
    const nbState = {providers: detected('netbird'), active: true, running: true, peers};
    const collapsed = section(M.resolvePanel(nbState, {nowMs: NOW}), 'machines');
    const nas = only(collapsed.rows, r => r.label === 'nas', 'nas row');
    assert.ok(nas.children.length > 0, 'the detail is carried, not fetched on expand');
    assert.equal(nas.expanded, false);
    assert.equal(only(nas.actions, a => a.id === 'detail', 'detail action').label, 'Show details');

    const open = section(M.resolvePanel(nbState, {nowMs: NOW, expandedPeerId: nas.payload.id}), 'machines');
    const openNas = only(open.rows, r => r.label === 'nas', 'nas row');
    assert.equal(openNas.expanded, true);
    assert.equal(only(openNas.actions, a => a.id === 'detail', 'detail action').label, 'Hide details');
    // Only the one asked for.
    assert.equal(only(open.rows, r => r.label === 'laptop', 'laptop row').expanded, false);
});

test('a zero date is "never", from either provider', () => {
    // Tailscale fills the handshake only once a session is up; an idle peer
    // carries the year-1 timestamp, which read as 739872 days ago.
    const idle = M.peerFromStatus('x', {
        LastHandshake: '0001-01-01T00:00:00Z',
        LastSeen: '2026-09-14T05:00:00Z',
        Relay: 'par',
    }, {});
    assert.equal(idle.LastHandshake, '');
    assert.equal(idle.LastSeen, '2026-09-14T05:00:00Z');

    const rows = M.peerDetailRows(idle, (x: string) => x, NOW);
    const by = Object.fromEntries(rows.map(r => [r.id, r.sublabel]));
    assert.equal(by['detail:handshake'], undefined, 'no handshake, no row');
    assert.equal(by['detail:seen'], '1 hour ago', 'last seen stands in for it');
    assert.equal(by['detail:connection'], 'Relayed via par');
});

test('a live handshake wins over last seen', () => {
    const live = M.peerFromStatus('x', {
        CurAddr: '1.2.3.4:41641',
        LastHandshake: '2026-09-14T05:59:00Z',
        LastSeen: '2026-09-14T05:00:00Z',
    }, {});
    const by = Object.fromEntries(
        M.peerDetailRows(live, (x: string) => x, NOW).map(r => [r.id, r.sublabel]));
    assert.equal(by['detail:handshake'], '1 minute ago');
    assert.equal(by['detail:seen'], undefined, 'not both');
});

test('the hero carries the active provider mark', () => {
    const ts = M.resolvePanel(state({providers: detected('tailscale')}), {}).header;
    const nb = M.resolvePanel(state({providers: detected('netbird')}), {}).header;
    assert.notEqual(ts.glyph, nb.glyph, 'each provider is drawn as itself');
    assert.equal(ts.glyph, M.providerById('tailscale')!.glyph);
    assert.equal(nb.icon, M.providerById('netbird')!.icon);
    // Nothing installed still draws something rather than an empty box.
    const none = M.resolvePanel(state({providers: detected(), installed: false}), {}).header;
    assert.notEqual(none.glyph, '');
});

test('every provider brings its own mark', () => {
    for (const p of M.providerDescriptors()) {
        assert.ok(p.glyph.length > 0, `${p.id} has no glyph`);
        assert.ok(p.icon.length > 0, `${p.id} has no icon name`);
    }
});

test('a public key is a copy option, from either provider', () => {
    const ts = M.peerFromStatus('x', {PublicKey: 'nodekey:abc', TailscaleIPs: ['100.64.0.9']}, {});
    assert.equal(ts.PublicKey, 'nodekey:abc');
    assert.equal(only(M.peerCopyOptions(ts), o => o.kind === 'key', 'key option').label, 'nodekey:abc');
    // A peer that reports no key offers no such option.
    assert.equal(M.peerCopyOptions(M.peerFromStatus('x', {}, {})).some(o => o.kind === 'key'), false);
});

test('an idle peer still opens onto something', () => {
    // Tailscale reports no session details for a peer with no session, which
    // left araki's row showing one line.
    const idle = M.peerFromStatus('x', {
        Relay: 'lhr', Created: '2026-09-13T06:00:00Z',
        LastHandshake: '0001-01-01T00:00:00Z', LastSeen: '0001-01-01T00:00:00Z',
    }, {});
    const by = Object.fromEntries(
        M.peerDetailRows(idle, (x: string) => x, NOW).map(r => [r.id, r.sublabel]));
    assert.equal(by['detail:connection'], 'Relayed via lhr');
    assert.equal(by['detail:added'], '1 day ago');

    // A peer with real detail does not need the filler.
    const busy = M.peerFromStatus('x', {
        CurAddr: '1.2.3.4:41641', RxBytes: 10, TxBytes: 20,
        LastHandshake: '2026-09-14T05:59:00Z', Created: '2026-09-13T06:00:00Z',
        PrimaryRoutes: ['10.0.0.0/8'],
    }, {});
    assert.equal(Object.keys(Object.fromEntries(
        M.peerDetailRows(busy, (x: string) => x, NOW).map(r => [r.id, r.sublabel])))
        .includes('detail:added'), false);
});

test('the hero says which provider to draw, not just what to print', () => {
    assert.equal(M.resolvePanel(state({providers: detected('tailscale')}), {}).header.providerId, 'tailscale');
    assert.equal(M.resolvePanel(state({providers: detected('netbird')}), {}).header.providerId, 'netbird');
    assert.equal(M.resolvePanel(state({providers: detected(), installed: false}), {}).header.providerId, '');
});
