import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {root, testDir} from './paths.js';

import type * as ModelTypes from '../shared/model.js';

// The port's safety net. `shared/model.ts` is still the copy all three
// frontends run on; the Rust in crates/tailgauge-core is not trusted until it
// agrees with it, fixture for fixture. Every function ported gets a row here
// before it gets a caller.
//
// When this fails, the Rust is wrong: the TypeScript is the specification
// until the last frontend is flipped.

// The compiled ES module, which no frontend ships any more: GNOME and Omarchy
// read their panel from the binary, so this is built for the tests and for
// Plasma until it is flipped too.
const built = path.join(root, 'build', '.ts', 'model', 'model.js');
if (!fs.existsSync(built))
    throw new Error('run scripts/build.sh before the tests: the ES module build is missing');
const M = await import(built) as typeof ModelTypes;

const binary = ['debug', 'release']
    .map(profile => path.join(root, 'target', profile, 'tailgauge'))
    .find(fs.existsSync);
if (!binary)
    throw new Error('run `cargo build` before the tests: target/debug/tailgauge is missing');

const fixture = (name: string): string =>
    fs.readFileSync(path.join(testDir, 'fixtures', name), 'utf8');

/// Ask the binary what the Rust made of a fixture.
const rust = (op: string, input: string): unknown =>
    JSON.parse(execFileSync(binary, ['internal-model', op], {
        input, encoding: 'utf8', timeout: 20000,
    }));

// A fixture the TypeScript and the Rust must read the same way. Grown from
// real tailnet states as the port goes; `inline` carries the cases a captured
// state does not happen to contain.
type Case = {what: string; input: string};

const statusCases: Case[] = [
    {what: 'the captured tailnet', input: fixture('status.json')},
    {what: 'nothing at all', input: ''},
    {what: 'whitespace', input: '   \n  '},
    {what: 'output that is not JSON', input: 'tailscale: command not found'},
    {what: 'an empty document', input: '{}'},
    {what: 'a daemon that wants a login', input: JSON.stringify({
        BackendState: 'NeedsLogin',
        AuthURL: 'https://login.tailscale.com/a/1234abcd',
    })},
    // The machine's own addresses live at the top level on some daemon
    // versions and inside Self on others.
    {what: 'addresses only at the top level', input: JSON.stringify({
        BackendState: 'Running',
        TailscaleIPs: ['100.64.0.1', 'fd7a:115c:a1e0::1', '192.168.1.5'],
        Self: {HostName: 'box', DNSName: 'box.example.ts.net.'},
    })},
    // Both halves of the sort, and the tie the two languages break
    // differently: localeCompare folds case and puts lowercase first, while
    // comparing bytes does neither.
    {what: 'machine names that differ only in case', input: JSON.stringify({
        BackendState: 'Running',
        Peer: {
            a: {HostName: 'Box-2', Online: true},
            b: {HostName: 'box-10', Online: true},
            c: {HostName: 'box-1', Online: true},
            d: {HostName: 'apple', Online: false},
            e: {HostName: 'Zebra', Online: true},
        },
    })},
    {what: 'two machines whose names differ only in case', input: JSON.stringify({
        BackendState: 'Running',
        Peer: {
            a: {HostName: 'Box', Online: true},
            b: {HostName: 'box', Online: true},
            c: {HostName: 'BOX', Online: true},
        },
    })},
    {what: 'a machine that calls itself localhost', input: JSON.stringify({
        BackendState: 'Running',
        Peer: {a: {HostName: 'localhost', DNSName: 'router.example.ts.net.', Online: true}},
    })},
    {what: 'a Mullvad peer, which is not a machine', input: JSON.stringify({
        BackendState: 'Running',
        Peer: {
            a: {HostName: 'de-ber-wg-001.mullvad.ts.net', Online: true, ExitNodeOption: true},
            b: {DNSName: 'fr-par-wg-101.mullvad.ts.net.', Online: true, ExitNodeOption: true},
            c: {HostName: 'laptop', Online: true},
        },
    })},
    {what: 'an exit node that is offline', input: JSON.stringify({
        BackendState: 'Running',
        Peer: {
            a: {HostName: 'up', Online: true, ExitNodeOption: true},
            b: {HostName: 'down', Online: false, ExitNodeOption: true},
        },
    })},
    {what: "Go's zero time, which means never", input: JSON.stringify({
        BackendState: 'Running',
        Peer: {a: {
            HostName: 'box', Online: true,
            LastHandshake: '0001-01-01T00:00:00Z',
            LastSeen: '2026-09-14T10:00:00Z',
            Created: '0001-01-01T00:00:00Z',
        }},
    })},
    {what: 'both shapes of the file-sharing capability', input: JSON.stringify({
        BackendState: 'Running',
        Self: {HostName: 'box', CapMap: {'https://tailscale.com/cap/file-sharing': null}},
    })},
    {what: 'the capability as a plain list', input: JSON.stringify({
        BackendState: 'Running',
        Self: {HostName: 'box', Capabilities: ['https://tailscale.com/cap/file-sharing']},
    })},
    {what: 'an owner resolved through the user table', input: JSON.stringify({
        BackendState: 'Running',
        User: {'1': {DisplayName: 'Alice'}, '2': {LoginName: 'bob@example.com'}, '3': {ID: 3}},
        Peer: {
            a: {HostName: 'a', UserID: '1', Online: true},
            b: {HostName: 'b', UserID: '2', Online: true},
            c: {HostName: 'c', UserID: '3', Online: true},
            d: {HostName: 'd', UserID: '9', Online: true},
            e: {HostName: 'e', Online: true},
        },
    })},
    {what: 'a relayed peer beside a direct one', input: JSON.stringify({
        BackendState: 'Running',
        Peer: {
            a: {HostName: 'direct', Online: true, CurAddr: '10.0.0.1:41641', Relay: 'fra'},
            b: {HostName: 'relayed', Online: true, Relay: 'fra'},
            c: {HostName: 'neither', Online: true},
        },
    })},
];

for (const {what, input} of statusCases) {
    test(`parseStatus agrees on ${what}`, () => {
        assert.deepEqual(rust('parse-status', input), JSON.parse(JSON.stringify(M.parseStatus(input))),
            'the Rust parser disagrees with shared/model.ts, which is still the specification');
    });
}

const accountCases: Case[] = [
    {what: 'the captured profile list', input: fixture('accounts.json')},
    {what: 'nothing at all', input: ''},
    {what: 'output that is not JSON', input: 'Error: not logged in'},
    {what: 'an object where a list belongs', input: '{}'},
    {what: 'an empty list', input: '[]'},
    // The CLI has spelled these both ways across versions, and both are still
    // out there on machines that have not updated.
    {what: 'the capitalised spelling', input: JSON.stringify(
        [{ID: '7', Name: 'Work', Tailnet: 'work.ts.net', LoginName: 'a@b', Selected: true}])},
    {what: 'an account with nothing but an id', input: JSON.stringify([{id: '9', selected: true}])},
    {what: 'no account selected', input: JSON.stringify([{id: '1', nickname: 'work'}])},
];

for (const {what, input} of accountCases) {
    test(`parseAccounts agrees on ${what}`, () => {
        assert.deepEqual(rust('parse-accounts', input), JSON.parse(JSON.stringify(M.parseAccounts(input))));
    });
}

const table = (rows: string) =>
    'IP             HOSTNAME                      COUNTRY        CITY           STATUS\n' + rows;

const exitNodeCases: Case[] = [
    {what: 'the captured table', input: fixture('exit-nodes.txt')},
    {what: 'nothing at all', input: ''},
    {what: 'a table with no header', input: 'some error\n'},
    {what: 'a header and nothing under it', input: table('')},
    {what: 'a city with a space in it', input: table(
        '100.100.0.3    us-nyc-wg-201.mullvad.ts.net  USA            New York       -\n')},
    {what: 'a row that is not a Mullvad server', input: table(
        '100.100.0.4    router.example.ts.net         -              -              -\n')},
    {what: 'two servers in one city', input: table(
        '100.100.0.1    de-ber-wg-001.mullvad.ts.net  Germany        Berlin         -\n' +
        '100.100.0.5    de-ber-wg-002.mullvad.ts.net  Germany        Berlin         selected\n')},
    {what: 'a comment line and a blank one', input: table(
        '\n# a note\n100.100.0.2    fr-par-wg-101.mullvad.ts.net  France         Paris          -\n')},
    {what: 'a city the CLI calls Any', input: table(
        '100.100.0.6    se-got-wg-001.mullvad.ts.net  Sweden         Any            -\n')},
    {what: 'a row cut short', input: table('100.100.0.7\n')},
    {what: 'CRLF line endings', input: table(
        '100.100.0.1    de-ber-wg-001.mullvad.ts.net  Germany        Berlin         -\r\n').replace(/(?<!\r)\n/g, '\r\n')},
];

for (const {what, input} of exitNodeCases) {
    test(`parseExitNodeList agrees on ${what}`, () => {
        assert.deepEqual(rust('parse-exit-node-list', input),
            JSON.parse(JSON.stringify(M.parseExitNodeList(input))));
    });
    test(`mullvadRegionOptions agrees on ${what}`, () => {
        assert.deepEqual(rust('mullvad-region-options', input),
            JSON.parse(JSON.stringify(M.mullvadRegionOptions(M.parseExitNodeList(input)))));
    });
}

const netbirdStatusCases: Case[] = [
    {what: 'the captured daemon', input: fixture('netbird-status.json')},
    {what: 'an idle daemon', input: fixture('netbird-status-idle.json')},
    {what: 'nothing at all', input: ''},
    {what: 'output that is not JSON', input: 'netbird: command not found'},
    {what: 'a list where an object belongs', input: '[]'},
    {what: 'a session that expired', input: JSON.stringify({daemonStatus: 'SessionExpired'})},
    {what: 'a login that failed', input: JSON.stringify({daemonStatus: 'LoginFailed'})},
    {what: 'an address carrying its prefix length', input: JSON.stringify({
        daemonStatus: 'Connected', netbirdIp: '100.92.0.3/16', fqdn: 'me.netbird.cloud.',
    })},
    {what: 'a latency in nanoseconds', input: JSON.stringify({
        daemonStatus: 'Connected',
        peers: {details: [
            {fqdn: 'a.netbird.cloud', status: 'Connected', latency: 25500000},
            {fqdn: 'b.netbird.cloud', status: 'Disconnected'},
        ]},
    })},
    {what: 'a peer with no name at all', input: JSON.stringify({
        daemonStatus: 'Connected',
        peers: {details: [{netbirdIp: '100.92.0.9/16', status: 'Connected', publicKey: 'abc='}]},
    })},
    {what: 'a peer reporting its transfer and its endpoint', input: JSON.stringify({
        daemonStatus: 'Connected',
        peers: {details: [{
            fqdn: 'box.netbird.cloud', status: 'Connected',
            connectionType: 'P2P', relayAddress: 'rel.example:443',
            iceCandidateEndpoint: {remote: '10.0.0.2:51820'},
            transferReceived: 4096, transferSent: 2048,
            lastWireguardHandshake: '0001-01-01T00:00:00Z',
            networks: ['10.0.0.0/24'],
        }]},
    })},
];

for (const {what, input} of netbirdStatusCases) {
    test(`parseNetbirdStatus agrees on ${what}`, () => {
        assert.deepEqual(rust('parse-netbird-status', input),
            JSON.parse(JSON.stringify(M.parseNetbirdStatus(input))));
    });
}

const netbirdNetworkCases: Case[] = [
    {what: 'the captured network list', input: fixture('netbird-networks.txt')},
    {what: 'nothing at all', input: ''},
    {what: 'a daemon with no networks', input: 'No networks available.'},
    {what: 'an error rather than a list', input: 'Error: daemon not running\nsecond line'},
    {what: 'a block with a dash for every absent field', input:
        'Available Networks:\n\n- ID: office\n  Network: -\n  Domains: -\n  Status: Not selected\n'},
    {what: 'a resolved-address line', input:
        'Available Networks:\n- ID: lab\n  Domains: lab.example.com, other.example.com\n' +
        '  [10.0.0.5]: resolved\n  Status: Selected\n'},
    {what: 'a field the panel does not show', input:
        'Available Networks:\n- ID: lab\n  Something Else: whatever\n  Status: Selected\n'},
];

for (const {what, input} of netbirdNetworkCases) {
    test(`parseNetbirdNetworks agrees on ${what}`, () => {
        assert.deepEqual(rust('parse-netbird-networks', input),
            JSON.parse(JSON.stringify(M.parseNetbirdNetworks(input))));
    });
}

const probeCases: Case[] = [
    {what: 'both CLIs resolved', input: '/usr/bin/tailscale\n/usr/bin/netbird\n'},
    {what: 'one resolved and one missed', input:
        '/usr/bin/tailscale\nwhich: no netbird in (/usr/bin:/bin)\n'},
    {what: 'nothing resolved', input: 'which: no tailscale in (/usr/bin)\n'},
    {what: 'nothing at all', input: ''},
    {what: 'a path with surrounding space', input: '  /opt/bin/netbird  \n'},
    {what: 'a relative path, which which never prints', input: 'bin/tailscale\n'},
    {what: 'the same binary twice', input: '/usr/bin/tailscale\n/usr/local/bin/tailscale\n'},
];

for (const {what, input} of probeCases) {
    test(`parseProviderProbe agrees on ${what}`, () => {
        assert.deepEqual(rust('parse-provider-probe', input),
            JSON.parse(JSON.stringify(M.parseProviderProbe(input))));
    });
}

// The bar is the first thing resolvePanel builds, and the first piece of the
// panel to cross into Rust. It takes a whole PanelState rather than raw output.
const detected = (...ids: string[]) =>
    [{id: 'tailscale', installed: ids.includes('tailscale')},
     {id: 'netbird', installed: ids.includes('netbird')}];

// Every summary a frontend puts in a PanelState comes out of
// Model.summarizeProvider, which sets all seven fields. A fixture that leaves
// one out is not an input the model can receive, and comparing on it would
// pin the two languages' readings of `undefined` rather than any behaviour.
const summary = (id: string, over: Record<string, unknown> = {}) => ({
    id,
    label: id === 'netbird' ? 'NetBird' : 'Tailscale',
    running: false, needsLogin: false, selfName: '', selfIp: '', state: '',
    ...over,
});

const barCases: {what: string; state: Record<string, unknown>}[] = [
    {what: 'a machine with no VPN CLI', state: {}},
    {what: 'a frontend that probes nothing but reports installed', state: {installed: true}},
    {what: 'one provider, nothing reported yet', state: {providers: detected('tailscale')}},
    {what: 'one provider up', state: {
        providers: detected('tailscale'),
        summaries: [summary('tailscale', {
            running: true, selfName: 'workstation', selfIp: '100.64.0.1', state: 'Running',
        })],
    }},
    {what: 'a provider up with no address to show', state: {
        providers: detected('tailscale'),
        summaries: [summary('tailscale', {running: true, selfName: 'workstation', state: 'Running'})],
    }},
    // The icon describes the machine, not the view: NetBird is up while the
    // panel is showing Tailscale.
    {what: 'the provider that is up is not the one being shown', state: {
        providers: detected('tailscale', 'netbird'),
        summaries: [summary('tailscale'), summary('netbird', {running: true, selfName: 'me'})],
    }},
    {what: 'a provider wanting a login', state: {
        providers: detected('tailscale'),
        summaries: [summary('tailscale', {needsLogin: true, state: 'NeedsLogin'})],
    }},
    {what: 'a daemon in some other state', state: {
        providers: detected('tailscale'),
        summaries: [summary('tailscale', {state: 'Starting'})],
    }},
    {what: 'the shown provider leading the tooltip', state: {
        providers: detected('tailscale', 'netbird'),
        activeProviderId: 'netbird',
        summaries: [
            summary('tailscale', {running: true, state: 'Running'}),
            summary('netbird', {running: true, selfIp: '100.92.0.3', state: 'Connected'}),
        ],
    }},
    {what: 'no summaries, falling back to the active flag', state: {
        providers: detected('tailscale'), active: true,
    }},
    {what: 'no summaries, falling back to needsLogin', state: {
        providers: detected('tailscale'), needsLogin: true,
    }},
    {what: 'a summary for a provider that is not installed', state: {
        providers: detected('tailscale'),
        summaries: [summary('netbird', {running: true})],
    }},
];

for (const {what, state: barState} of barCases) {
    test(`barState agrees on ${what}`, () => {
        assert.deepEqual(rust('bar-state', JSON.stringify(barState)),
            JSON.parse(JSON.stringify(M.barState(barState as never))));
    });
}

// The formatters take a JSON array of cases and answer in order, so a whole
// table crosses the process boundary in a single spawn. JSON rather than a
// separator: every separator worth choosing turns up inside a shell argument
// or a CLI's complaint sooner or later.
const eachCase = (op: string, cases: unknown[]): string[] =>
    rust(op, JSON.stringify(cases)) as string[];

const NOW = Date.parse('2026-09-14T06:00:00Z');

test('formatBytes agrees on every scale', () => {
    const values = [0, -5, 1, 424, 1023, 1024, 1536, 1468006, 15728640,
                    1073741824, 1099511627776, 9007199254740991];
    assert.deepEqual(eachCase('format-bytes', values), values.map(v => M.formatBytes(v)));
});

test('formatSince agrees on every bracket', () => {
    const times = [
        '', '   ', 'not a date',
        '2026-09-14T06:00:00Z', '2026-09-14T05:59:50Z', '2026-09-14T05:59:00Z',
        '2026-09-14T05:30:00Z', '2026-09-14T05:00:00Z', '2026-09-14T03:00:00Z',
        '2026-09-13T06:00:00Z', '2026-09-01T06:00:00Z',
        '2026-09-14T07:00:00Z',
        '2026-09-14T05:59:00+00:00', '2026-09-14T07:59:00+02:00',
    ];
    assert.deepEqual(eachCase('format-since', times.map(t => [t, NOW])),
        times.map(t => M.formatSince(t, NOW)));
});

test('elideStatus agrees on what it keeps and what it cuts', () => {
    const texts = ['', 'short', '  one   two \t three  ', 'x'.repeat(139),
                   'x'.repeat(140), 'x'.repeat(200), 'é'.repeat(200)];
    assert.deepEqual(eachCase('elide-status', texts), texts.map(t => M.elideStatus(t)));
});

test('firstUrl agrees on where a login link starts and stops', () => {
    const cases: [string, string][] = [
        ['To authenticate, visit:\n\n\thttps://login.tailscale.com/a/1 \n', ''],
        ['visit https://login.tailscale.com/a/1 to log in', ''],
        ['http://insecure.example/x', ''],
        ['nothing here', 'fallback'],
        ['', ''],
    ];
    assert.deepEqual(eachCase('first-url', cases),
        cases.map(([text, fallback]) => M.firstUrl(text, fallback)));
});

test('shellCommand agrees on what survives a shell', () => {
    const argvs = [
        ['tailscale', 'status', '--json'],
        ['tailscale', 'set', "--exit-node=it's"],
        ['tailgauge', 'copy', 'a b\tc'],
        [''],
        [],
    ];
    assert.deepEqual(eachCase('shell-command', argvs), argvs.map(a => M.shellCommand(a)));
});

// What a machine row shows. The Peer comes back out of parseStatus, so these
// cases are built from a status document rather than hand-written: a Peer the
// parser would never produce is not an input the panel can receive.
const peersFrom = (status: Record<string, unknown>): ModelTypes.Peer[] => {
    const parsed = M.parseStatus(JSON.stringify(status));
    assert.ok(parsed.ok && !parsed.unavailable, 'the fixture should parse');
    return [(parsed as ModelTypes.StatusOk).selfPeer, ...(parsed as ModelTypes.StatusOk).peers];
};

const peerCases: ModelTypes.Peer[] = [
    ...peersFrom(JSON.parse(fixture('status.json'))),
    ...peersFrom({
        BackendState: 'Running',
        User: {'1': {DisplayName: 'Alice'}},
        Peer: {
            direct: {
                HostName: 'direct', DNSName: 'direct.example.ts.net.', Online: true, OS: 'linux',
                UserID: '1', TailscaleIPs: ['100.64.0.2', 'fd7a:115c:a1e0::2'],
                CurAddr: '[2001:db8::1]:51820', Relay: 'ams',
                RxBytes: 424, TxBytes: 472, LastHandshake: '2026-09-14T05:59:00Z',
                PrimaryRoutes: ['192.168.42.0/24'], PublicKey: 'nodekey:abc',
                TaildropTarget: 1,
            },
            relayed: {HostName: 'relayed', Online: true, OS: 'windows', Relay: 'fra'},
            idle: {
                HostName: 'idle', Online: false, OS: 'android',
                LastSeen: '2026-09-13T06:00:00Z', Created: '2026-01-01T00:00:00Z',
            },
            bare: {HostName: 'bare', Online: true},
            macos: {HostName: 'mac', Online: true, OS: 'macos'},
            ios: {HostName: 'phone', Online: true, OS: 'ios'},
            unknown: {HostName: 'thing', Online: true, OS: 'plan9'},
        },
    }),
    // A Mullvad node, which comes out of the exit-node table rather than the
    // status document and carries a different set of fields.
    ...M.parseExitNodeList(fixture('exit-nodes.txt')),
];

test('a machine row agrees on everything it shows', () => {
    assert.deepEqual(
        rust('peer-row', JSON.stringify(peerCases.map(p => [p, NOW]))),
        peerCases.map(peer => JSON.parse(JSON.stringify({
            address: M.peerAddress(peer),
            exitNodeTarget: M.exitNodeTarget(peer),
            copyOptions: M.peerCopyOptions(peer),
            subtitle: M.peerSubtitle(peer),
            rowSubtitle: M.peerRowSubtitle(peer),
            connection: M.connectionSummary(peer),
            osIcon: M.osIcon(peer.OS),
            osIconName: M.osIconName(peer.OS),
            details: M.peerDetailRows(peer, NOW),
        }))));
});

const chromeCases: [Record<string, unknown>, number][] = [
    [{}, 0],
    [{installed: true}, 0],
    [{providers: detected('tailscale'), version: '1.2.3'}, 0],
    [{providers: detected('tailscale'), active: true, selfName: 'workstation'}, 0],
    // Every phrase, and the wrap at both ends.
    ...[0, 1, 9, 10, 11, -1, -10, -11].map((i): [Record<string, unknown>, number] =>
        [{providers: detected('tailscale'), active: true, selfName: 'box'}, i]),
    [{providers: detected('tailscale'), needsLogin: true}, 0],
    [{providers: detected('tailscale'), busy: true, active: true}, 0],
    // Precedence: progress beats a stale error, and both beat the idle line.
    [{providers: detected('tailscale'), actionStatus: 'Connecting…', lastError: 'old'}, 0],
    [{providers: detected('tailscale'), lastError: 'Something broke'}, 0],
    [{providers: detected('netbird'), activeProviderId: 'netbird', active: true}, 0],
    // The footer's skew line.
    [{providers: detected('tailscale'), version: '1.2.3', update: {current: '1.2.2'}}, 0],
    [{providers: detected('tailscale'), version: '1.2.3', update: {current: '1.2.3'}}, 0],
    [{providers: detected('tailscale'), update: {current: '1.2.2'}}, 0],
];

test('the header, status and footer agree', () => {
    assert.deepEqual(
        rust('panel-chrome', JSON.stringify(chromeCases)),
        chromeCases.map(([s, phrase]) => JSON.parse(JSON.stringify({
            header: M.resolvePanel(s as never, {phraseIndex: phrase}).header,
            status: M.resolvePanel(s as never, {}).status,
            footer: M.resolvePanel(s as never, {}).footer,
            toggleHint: M.toggleHint(s as never),
        }))));
});

// The whole panel. This is the case the port exists for: every section, the
// bar, the header, the status precedence and the traversal order, resolved
// from one state and compared field for field.
const statusOf = (raw: string) => {
    const parsed = M.parseStatus(raw) as ModelTypes.StatusOk;
    assert.ok(parsed.ok && !parsed.unavailable, 'the fixture should parse');
    return parsed;
};

const tailnet = statusOf(fixture('status.json'));
const mullvad = M.mullvadRegionOptions(M.parseExitNodeList(fixture('exit-nodes.txt')));
const accountsFixture = M.parseAccounts(fixture('accounts.json'));
const netbirdNetworks = M.parseNetbirdNetworks(fixture('netbird-networks.txt')).networks;

const live = (over: Record<string, unknown> = {}) => ({
    providers: detected('tailscale'),
    installed: true, running: true, active: true, helpers: true,
    selfName: tailnet.selfName, selfIp: tailnet.selfIp, selfUserId: tailnet.selfUserId,
    selfPeer: tailnet.selfPeer, fileSharing: tailnet.fileSharing,
    peers: tailnet.peers, ownExitNodes: tailnet.exitNodes,
    version: '0.4.0',
    ...over,
});

const manyPeers = Array.from({length: 12}, (_, i) => ({
    ...tailnet.peers[0], id: `peer-${i}`, HostName: `box-${i}`, DisplayName: `box-${i}`,
    DNSName: `box-${i}.example.ts.net`, IPv4: [`100.64.1.${i}`],
    OS: i % 2 === 0 ? 'linux' : 'windows',
}));

const panelCases: [Record<string, unknown>, Record<string, unknown>][] = [
    [{}, {}],
    [{installed: true}, {}],
    [{providers: detected('tailscale')}, {}],
    [live(), {nowMs: NOW}],
    [live({active: false}), {nowMs: NOW}],
    [live({peers: manyPeers}), {nowMs: NOW}],
    [live({peers: []}), {nowMs: NOW}],
    // Either side of the threshold that grows the search field, and on it.
    ...[7, 8, 9].map((n): [Record<string, unknown>, Record<string, unknown>] =>
        [live({peers: manyPeers.slice(0, n)}), {nowMs: NOW}]),
    // Exit nodes, the picker, and the shortlist of recent regions.
    [live({mullvadRegions: mullvad}), {nowMs: NOW}],
    [live({mullvadRegions: mullvad}), {nowMs: NOW, mullvadPickerOpen: true}],
    [live({mullvadRegions: mullvad}),
     {nowMs: NOW, recentRegions: ['France\nParis', 'Germany\nBerlin']}],
    [live({mullvadRegions: [], ownExitNodes: []}), {nowMs: NOW}],
    // An expanded machine puts its details into the traversal.
    [live(), {nowMs: NOW, expandedPeerId: tailnet.peers[0].id}],
    [live(), {nowMs: NOW, expandedPeerId: 'nothing-here'}],
    // Accounts, and the operator refusal.
    [live({accounts: accountsFixture.accounts}), {nowMs: NOW}],
    [live({accounts: accountsFixture.accounts, switchingAccountId: accountsFixture.accounts[0]?.id}),
     {nowMs: NOW}],
    [live({accountsAccessDenied: true, busy: true}), {nowMs: NOW}],
    // NetBird: networks instead of exit nodes, and no Taildrop.
    [{...live(), providers: detected('netbird'), activeProviderId: 'netbird',
      networks: netbirdNetworks}, {nowMs: NOW}],
    [{...live(), providers: detected('tailscale', 'netbird'), networks: netbirdNetworks},
     {nowMs: NOW}],
    // The banner, the status precedence and the footer's skew line.
    [live({update: {available: true, latest: '9.9.9', current: '0.4.0'}}), {nowMs: NOW}],
    [live({update: {available: true, latest: '9.9.9', current: '0.4.0'}, updating: true}), {nowMs: NOW}],
    [live({actionStatus: 'Connecting…', lastError: 'stale'}), {nowMs: NOW}],
    [live({lastError: 'Something broke'}), {nowMs: NOW}],
    [live({update: {current: '0.3.9'}}), {nowMs: NOW}],
    // A store install has no binary on PATH, so the send action goes.
    [live({helpers: false}), {nowMs: NOW}],
    [live({fileSharing: false}), {nowMs: NOW}],
    // Every phrase, and the wrap at both ends.
    ...[0, 5, 10, -3].map((i): [Record<string, unknown>, Record<string, unknown>] =>
        [live(), {nowMs: NOW, phraseIndex: i}]),
];

test('the whole panel agrees, section for section', () => {
    const fromRust = rust('panel', JSON.stringify(panelCases)) as unknown[];
    const fromTs = panelCases.map(([state, options]) =>
        JSON.parse(JSON.stringify(M.resolvePanel(state as never, options as never))));
    // One case at a time, so a failure names the state rather than dumping
    // every panel in the table.
    for (let i = 0; i < panelCases.length; i++)
        assert.deepEqual(fromRust[i], fromTs[i], `case ${i} disagrees`);
});
