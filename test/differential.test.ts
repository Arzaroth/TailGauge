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
    // Both halves of the sort, and a tie the two languages could break
    // differently: localeCompare folds case, comparing bytes does not.
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
