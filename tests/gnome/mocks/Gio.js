// Enough of Gio for a service that spawns and reads settings. Nothing is
// spawned: a test answers a command, which is what makes these tests about
// what the extension does with an answer.

const spawned = [];

class Cancellable {
    cancel() {
        this.cancelled = true;
    }
}

class Subprocess {
    static new(argv, flags) {
        const proc = new Subprocess();
        // argv is `['sh', '-c', preamble, 'sh', ...real]`, which is how the
        // extension reaches a binary the session PATH may not carry.
        proc.argv = argv;
        proc.real = argv.slice(4);
        proc.flags = flags;
        proc.answered = false;
        spawned.push(proc);
        return proc;
    }

    communicate_utf8_async(_stdin, cancellable, callback) {
        this.cancellable = cancellable;
        this.callback = callback;
    }

    communicate_utf8_finish() {
        if (this.thrown)
            throw this.thrown;
        return [true, this.stdout, this.stderr];
    }

    get_exit_status() {
        return this.status;
    }

    /// What a test calls to answer a command that was started.
    answer(stdout = '', stderr = '', status = 0) {
        this.stdout = stdout;
        this.stderr = stderr;
        this.status = status;
        this.answered = true;
        this.callback?.(this, null);
    }

    /// The failure path: `communicate_utf8_finish` raising rather than
    /// returning, which is how a cancelled or broken spawn arrives.
    fail(error) {
        this.thrown = error;
        this.callback?.(this, null);
    }
}

class Settings {
    constructor(values = {}) {
        this.values = {
            'active-provider': '',
            'refresh-interval': 30,
            'recent-mullvad-regions': [],
            ...values,
        };
        this._handlers = new Map();
        this._nextId = 1;
    }

    get_string(key) { return this.values[key] ?? ''; }
    get_int(key) { return this.values[key] ?? 0; }
    get_strv(key) { return this.values[key] ?? []; }
    set_string(key, value) { this.values[key] = value; }
    set_strv(key, value) { this.values[key] = value; }
    connect(name, callback) {
        const id = this._nextId++;
        this._handlers.set(id, {name, callback});
        return id;
    }
    disconnect(id) { this._handlers.delete(id); }
}

/// Test helpers over what has been spawned.
function __spawned() {
    return spawned;
}

/// The most recent unanswered command whose argv contains `needle`.
function __find(needle) {
    for (let i = spawned.length - 1; i >= 0; i--) {
        const proc = spawned[i];
        if (!proc.answered && proc.real.join(' ').includes(needle))
            return proc;
    }
    return null;
}

function __reset() {
    spawned.length = 0;
}

export default {
    Cancellable,
    Subprocess,
    Settings,
    SubprocessFlags: {NONE: 0, STDOUT_PIPE: 1, STDERR_PIPE: 2},
    IOErrorEnum: {CANCELLED: 19},
    __spawned,
    __find,
    __reset,
};
