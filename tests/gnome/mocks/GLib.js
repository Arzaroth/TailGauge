// Timers that do not fire on their own. A test that wants one fires it, so a
// suite never races a clock it did not ask for.

const timeouts = new Map();
let nextId = 1;

function timeout_add(_priority, interval, handler) {
    const id = nextId++;
    timeouts.set(id, {interval, handler});
    return id;
}

function source_remove(id) {
    return timeouts.delete(id);
}

/// Test helpers: what is armed, and firing one.
function __armed() {
    return [...timeouts.entries()].map(([id, t]) => ({id, interval: t.interval}));
}

function __fire(id) {
    const timer = timeouts.get(id);
    if (!timer)
        throw new Error(`no timer ${id}`);
    if (timer.handler() === false)
        timeouts.delete(id);
}

function __reset() {
    timeouts.clear();
    nextId = 1;
}

export default {
    PRIORITY_DEFAULT: 0,
    SOURCE_REMOVE: false,
    SOURCE_CONTINUE: true,
    Error: class GLibError extends Error {},
    timeout_add,
    source_remove,
    __armed,
    __fire,
    __reset,
};
