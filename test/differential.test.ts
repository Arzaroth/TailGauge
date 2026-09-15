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

const built = path.join(root, 'build', 'tailgauge@arzaroth.github.io', 'model.js');
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
