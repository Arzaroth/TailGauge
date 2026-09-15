// `import Gio from 'gi://Gio'` is GJS's own scheme, which Node knows nothing
// about. This points it at the mocks so the extension's own sources can be
// loaded and driven here.
import {fileURLToPath} from 'node:url';

export async function resolve(specifier, context, next) {
    if (specifier.startsWith('gi://')) {
        const name = specifier.slice('gi://'.length);
        const url = new URL(`./mocks/${name}.js`, import.meta.url).href;
        return {url, shortCircuit: true, format: 'module'};
    }
    return next(specifier, context);
}

export const __mocksDir = fileURLToPath(new URL('./mocks/', import.meta.url));
