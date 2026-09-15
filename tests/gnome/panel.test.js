import assert from 'node:assert/strict';
import test from 'node:test';

import * as Panel from '../../build/tailgauge@arzaroth.github.io/panel.js';

// panel.ts is this side's description of the JSON the binary sends, plus the
// three things the extension genuinely decides for itself.

test('the empty panel says it is checking rather than drawing nothing', () => {
    const panel = Panel.emptyPanel();
    assert.equal(panel.status.text, 'Checking…');
    assert.equal(panel.header.toggleVisible, false, 'nothing to toggle yet');
    assert.equal(panel.header.crossed, true);
    assert.deepEqual(panel.sections, []);
    assert.deepEqual(panel.navigation, [], 'and nothing for a cursor to land on');
});

test('panelHasRow finds a row wherever it is drawn', () => {
    const row = (id, children = []) => ({id, children});
    const panel = {
        sections: [
            {id: 'machines', rows: [row('machines:search'), row('peer:a')]},
            {id: 'exitNodes', rows: [row('mullvad:add', [row('region:paris')])]},
        ],
    };
    assert.equal(Panel.panelHasRow(panel, 'machines:search'), true);
    assert.equal(Panel.panelHasRow(panel, 'region:paris'), true, 'children count');
    assert.equal(Panel.panelHasRow(panel, 'nothing:here'), false);
    assert.equal(Panel.panelHasRow({sections: []}, 'anything'), false);
});

// The pair that names a region is already in the id the binary minted for it.
// Rebuilding it from the peer's fields is the model's job, not this side's.
test('the region key is read off the id rather than rebuilt', () => {
    assert.equal(Panel.mullvadRegionKey({id: 'mullvad-region:France\nParis'}), 'France\nParis');
    assert.equal(Panel.mullvadRegionKey({id: 'peer:laptop'}), '', 'a machine is not a region');
    assert.equal(Panel.mullvadRegionKey(null), '');
    assert.equal(Panel.mullvadRegionKey(undefined), '');
    assert.equal(Panel.mullvadRegionKey({}), '');
});

test('choosing a region moves it to the front, capped', () => {
    const recent = ['a', 'b', 'c'];
    assert.deepEqual(Panel.pushRecentMullvad(recent, 'b', 5), ['b', 'a', 'c'], 'and never twice');
    assert.deepEqual(Panel.pushRecentMullvad(recent, 'd', 3), ['d', 'a', 'b'], 'the oldest falls off');
    assert.deepEqual(Panel.pushRecentMullvad(recent, 'a', 5), ['a', 'b', 'c']);
    assert.deepEqual(Panel.pushRecentMullvad(recent, '', 5), recent, 'nothing chosen changes nothing');
    assert.deepEqual(Panel.pushRecentMullvad([], 'a', 5), ['a']);
    // The list it was handed is not the list it returns.
    const original = ['x'];
    Panel.pushRecentMullvad(original, 'y', 5);
    assert.deepEqual(original, ['x']);
});
