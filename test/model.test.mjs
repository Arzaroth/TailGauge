import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {fileURLToPath} from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const built = path.join(root, 'build', 'tailgauge@arzaroth.github.io', 'model.js');

if (!fs.existsSync(built))
    throw new Error('run scripts/build.sh before the tests: the ES module build is missing');

const M = await import(built);
const fixture = name => fs.readFileSync(path.join(here, 'fixtures', name), 'utf8');

const status = M.parseStatus(fixture('status.json'));
const accounts = M.parseAccounts(fixture('accounts.json'));
const mullvadNodes = M.parseExitNodeList(fixture('exit-nodes.txt'));
const mullvadRegions = M.mullvadRegionOptions(mullvadNodes);

function state(overrides = {}) {
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
        tailnetExitNodes: status.exitNodes,
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

const section = (panel, id) => panel.sections.find(s => s.id === id);

// Past the threshold the machines section grows a search field, which the
// fixture's four peers are deliberately too few to trigger.
const manyPeers = Array.from({length: 12}, (_, i) => ({
    id: `peer-${i}`,
    HostName: `box-${i}`,
    DisplayName: `box-${i}`,
    DNSName: `box-${i}.example.ts.net`,
    UserID: status.selfUserId,
    TaildropTarget: 1,
    TailscaleIPs: [`100.64.1.${i}`],
    TailscaleIPv6: [],
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
    assert.deepEqual(status.selfPeer.TailscaleIPv6, ['fd7a:115c:a1e0::1']);
});

test('parseStatus keeps every non-Mullvad peer, online ones first', () => {
    assert.deepEqual(status.peers.map(p => p.HostName), ['laptop', 'phone', 'router', 'offline-box']);
    assert.equal(status.peers.find(p => p.HostName === 'offline-box').Online, false);
});

test('parseStatus separates Tailscale IPv4 from IPv6', () => {
    const laptop = status.peers.find(p => p.HostName === 'laptop');
    assert.deepEqual(laptop.TailscaleIPs, ['100.64.0.2']);
    assert.deepEqual(laptop.TailscaleIPv6, ['fd7a:115c:a1e0::2']);
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
    let recent = [];
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
    assert.equal(M.peerSubtitle({TailscaleIPs: ['100.64.0.9'], DNSName: 'box.example.ts.net', UserName: 'bob@example.com'}),
        '100.64.0.9 · bob@example.com');
    // No User map, an older daemon: the DNS name still holds the second slot.
    assert.equal(M.peerSubtitle({TailscaleIPs: ['100.64.0.9'], DNSName: 'box.example.ts.net'}),
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
    assert.deepEqual(panel.sections.map(s => s.id), ['update', 'self', 'connections', 'exitNodes', 'machines']);
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
    const idle = section(M.resolvePanel(state({tailnetExitNodes: [{...status.exitNodes[0], ExitNode: false}]}), {}), 'exitNodes').rows;
    assert.equal(idle[0].hint, 'Connect');
});

test('the picker filters its regions and reports an empty result', () => {
    const open = q => section(M.resolvePanel(state(), {mullvadQuery: q, mullvadPickerOpen: true}), 'exitNodes')
        .rows.find(r => r.kind === 'mullvadPicker');
    assert.deepEqual(open('par').children.map(c => c.label), ['Paris']);
    assert.deepEqual(open('france').children.map(c => c.label), ['Marseille', 'Paris']);
    const none = open('zzz').children;
    assert.equal(none.length, 1);
    assert.equal(none[0].kind, 'empty');
    assert.equal(none[0].navigable, false);
});

test('machine rows carry their subtitle, icon, copy options and actions', () => {
    const rows = section(M.resolvePanel(state(), {}), 'machines').rows;
    const laptop = rows.find(r => r.label === 'laptop');
    assert.equal(laptop.sublabel, '100.64.0.2 · Alice');
    assert.equal(laptop.icon, 'computer-symbolic');
    assert.deepEqual(laptop.copyOptions.map(o => o.kind), ['name', 'dns', 'ipv6', 'ip']);
    assert.deepEqual(laptop.actions.map(a => a.id), ['send', 'copy']);
});

test('the send action appears only for a Taildrop target', () => {
    const rows = section(M.resolvePanel(state(), {}), 'machines').rows;
    assert.deepEqual(rows.find(r => r.label === 'router').actions.map(a => a.id), ['copy']);
    assert.deepEqual(rows.find(r => r.label === 'phone').actions.map(a => a.id), ['copy']);
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
    const names = query => M.filterMachines(status.peers, query).map(p => p.HostName);
    assert.equal(M.filterMachines(status.peers, '').length, status.peers.length);
    assert.deepEqual(names('ANDROID'), ['phone']);
    assert.deepEqual(names('100.64.0.5'), ['offline-box']);
    assert.deepEqual(names('example.ts.net'), status.peers.map(p => p.HostName));
    assert.deepEqual(M.filterMachines(null, 'anything'), []);
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
    const labels = query => section(M.resolvePanel(state({peers: manyPeers}), {machineQuery: query}), 'machines')
        .rows.filter(r => r.kind === 'peer').map(r => r.label);
    assert.deepEqual(labels('100.64.1.7'), ['box-7']);
    assert.deepEqual(labels('BOX-11'), ['box-11']);
    assert.deepEqual(labels('windows'), manyPeers.filter(p => p.OS === 'windows').map(p => p.DisplayName));
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

    const missing = M.resolvePanel(state({installed: false}), {}).header;
    assert.equal(missing.title, 'Tailscale');
    assert.equal(missing.toggleVisible, false);
});

test('the hero phrase rotates and wraps in both directions', () => {
    const phrase = i => M.resolvePanel(state(), {phraseIndex: i}).header.meta;
    assert.equal(phrase(0), M.ACTIVE_PHRASES[0]);
    assert.equal(phrase(M.ACTIVE_PHRASES.length), M.ACTIVE_PHRASES[0]);
    assert.equal(phrase(-1), M.ACTIVE_PHRASES[M.ACTIVE_PHRASES.length - 1]);
});

test('status precedence: missing CLI, then progress, then error', () => {
    assert.match(M.resolvePanel(state({installed: false}), {}).status.text, /not installed/);
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
    const panel = M.resolvePanel(state({installed: false, peers: [], accounts: [], tailnetExitNodes: [], mullvadRegions: []}), {});
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
    const visit = row => {
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
    assert.equal(switching.find(r => r.label === 'personal').busy, true);
    assert.equal(switching.find(r => r.label === 'work').busy, false);

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
