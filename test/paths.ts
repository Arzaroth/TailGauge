import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

// The tests read the repository, but they run from wherever tsc emitted them,
// so the root is found rather than assumed.
function findRoot(): string {
    let dir = path.dirname(fileURLToPath(import.meta.url));
    for (;;) {
        if (fs.existsSync(path.join(dir, 'package.json')))
            return dir;
        const parent = path.dirname(dir);
        if (parent === dir)
            throw new Error('no package.json above the test directory');
        dir = parent;
    }
}

export const root = findRoot();
export const testDir = path.join(root, 'test');
export const read = (p: string): string => fs.readFileSync(path.join(root, p), 'utf8');
