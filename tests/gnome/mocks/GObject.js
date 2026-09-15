// Enough of GObject for a service class: registerClass, a base with signals,
// and GJS's convention that `new Klass(args)` runs `_init(args)`.

class GObjectBase {
    constructor() {
        this._handlers = new Map();
        this._nextHandlerId = 1;
    }

    _init() {}

    connect(name, callback) {
        const id = this._nextHandlerId++;
        if (!this._handlers.has(name))
            this._handlers.set(name, new Map());
        this._handlers.get(name).set(id, callback);
        return id;
    }

    disconnect(id) {
        for (const handlers of this._handlers.values())
            handlers.delete(id);
    }

    emit(name, ...args) {
        for (const callback of [...(this._handlers.get(name)?.values() ?? [])])
            callback(this, ...args);
    }
}

function registerClass(metaOrClass, maybeClass) {
    const klass = maybeClass ?? metaOrClass;
    return class extends klass {
        constructor(...args) {
            super();
            this._init(...args);
        }
    };
}

export default {Object: GObjectBase, registerClass};
