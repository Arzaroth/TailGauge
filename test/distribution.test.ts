import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import {read, root} from './paths.js';

// Distribution is a contract spread across five files: the release workflow
// names the assets, the update helper downloads them by name, and three
// manifests declare a version. Nothing at runtime notices when they disagree -
// the updater just 404s - so it is checked here instead.



const release = read('.github/workflows/release.yml');
const updater = read('bin/tailgauge-update');
const plasmoidManifest = JSON.parse(read('plasma/org.tailgauge.plasmoid/metadata.json'));
const extensionManifest = JSON.parse(read('gnome/tailgauge@arzaroth.github.io/metadata.json'));
const pluginManifest = JSON.parse(read('omarchy/arzaroth.tailgauge/manifest.json'));

// The workflow writes dist/tailgauge-$safe-<suffix>; the updater fetches
// tailgauge-v$latest-<suffix>. Compare the suffixes.
const publishedSuffixes = [...release.matchAll(/dist\/tailgauge-\$safe-([\w.-]+)/g)]
    .map(m => m[1]).sort();
const downloadedSuffixes = [...updater.matchAll(/"tailgauge-v\$latest-([\w.-]+)"/g)]
    .map(m => m[1]).sort();

test('the updater downloads exactly the assets the release publishes', () => {
    assert.ok(publishedSuffixes.length >= 4, `found only ${publishedSuffixes.length} published assets`);
    assert.deepEqual(downloadedSuffixes, publishedSuffixes);
});

test('every packaged asset is actually uploaded', () => {
    const block = release.match(/files: \|\n((?:\s+\S+\n)+)/)?.[1];
    assert.ok(block, 'the release publishes no files block');
    const globs = block.trim().split('\n').map(l => l.trim()).filter(Boolean);
    const matches = (name: string) => globs.some(g => {
        const re = new RegExp('^' + g.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*') + '$');
        return re.test('dist/' + name);
    });
    for (const suffix of publishedSuffixes) {
        const name = `tailgauge-v0.0.0-${suffix}`;
        assert.ok(matches(name),
            `Package builds ${name} but no upload glob matches it: ${globs.join(', ')}`);
    }
});

test('the plasmoid ships as a .plasmoid zip', () => {
    // kpackagetool6 and the KDE Store both take a zip of the package contents;
    // a tarball installs from neither.
    assert.ok(publishedSuffixes.includes('plasmoid.plasmoid'),
        `published assets are ${publishedSuffixes.join(', ')}`);
    assert.match(release, /cd build\/org\.tailgauge\.plasmoid && zip/);
});

const helperNames = fs.readdirSync(path.join(root, 'bin')).sort();

test('every declared version agrees', () => {
    const plasmoid = plasmoidManifest.KPlugin.Version;
    assert.match(plasmoid, /^\d+\.\d+\.\d+$/);
    assert.equal(extensionManifest['version-name'], plasmoid, 'the GNOME extension disagrees with the plasmoid');
    assert.equal(pluginManifest.version, plasmoid, 'the Omarchy plugin disagrees with the plasmoid');
    for (const name of helperNames) {
        const declared = read(`bin/${name}`).match(/^VERSION="([^"]+)"$/m)?.[1];
        assert.equal(declared, plasmoid, `bin/${name} disagrees with the plasmoid`);
    }
});

// A helper reports the version of the file you actually ran, which is the only
// thing that distinguishes a half-applied update from a finished one. Run
// rather than read: a helper that carries the constant but never reaches the
// branch that prints it would answer nothing, and read the same in the source.
test('every helper prints its declared version', () => {
    const plasmoid = plasmoidManifest.KPlugin.Version;
    for (const name of helperNames) {
        const printed = execFileSync('bash', [path.join(root, 'bin', name), '--version'],
            {encoding: 'utf8', timeout: 10000});
        assert.equal(printed.trim(), `${name} ${plasmoid}`,
            `bin/${name} --version does not print its own name and version`);
    }
});

// The registry reads the manifest the archive carries, so a tarball that
// unpacks under any other name installs a plugin the updater cannot find again.
test('the Omarchy plugin ships under the id its manifest declares', () => {
    // The id carries a dot, which would otherwise match any character here.
    const id = pluginManifest.id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    assert.match(release, new RegExp(`tar -czf "dist/tailgauge-\\$safe-omarchy-plugin\\.tar\\.gz" -C build ${id}`));
    assert.match(pluginManifest.id, /^[A-Za-z0-9][A-Za-z0-9._-]*$/);
    assert.ok(!pluginManifest.id.startsWith('omarchy.'),
        'omarchy.* is reserved for first-party plugins');
});

test('the extension carries the integer version EGO expects', () => {
    assert.equal(typeof extensionManifest.version, 'number');
    assert.ok(Number.isInteger(extensionManifest.version));
});

test('the updater points at the repository the manifests name', () => {
    const repo = updater.match(/^REPO="([^"]+)"$/m)?.[1];
    assert.ok(repo, 'the updater declares no REPO');
    for (const url of [plasmoidManifest.KPlugin.Website, extensionManifest.url])
        assert.ok(url.includes(repo), `${url} does not point at ${repo}`);
});

test('the updater knows the package ids the manifests declare', () => {
    assert.match(updater, new RegExp(`PLASMOID_ID="${plasmoidManifest.KPlugin.Id}"`));
    assert.match(updater, new RegExp(`EXTENSION_UUID="${extensionManifest.uuid}"`));
    assert.match(updater, new RegExp(`PLUGIN_ID="${pluginManifest.id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}"`));
});

test('the release refuses to publish a tag the manifests disagree with', () => {
    assert.match(release, /tag \$REF_NAME does not match the manifests/);
});
