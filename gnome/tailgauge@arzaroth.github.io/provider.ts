import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';

import * as Panel from './panel.js';

// gnome-shell inherits the session PATH, which often lacks the user bin dirs
// the installer drops the binary into. `exec "$@"` keeps argv intact rather
// than pushing it back through the shell's parser.
const PATH_PREAMBLE = 'export PATH="$HOME/.local/bin:$HOME/bin:/usr/local/bin:$PATH"; ';

const ACTION_STATUS_MS = 2200;
const DELAYED_REFRESH_MS = 600;
const ATTENTIVE_INTERVAL_SEC = 3;
const WATCH_TIMEOUT_SEC = 300;
const WATCH_REARM_MS = 250;
const WATCH_BACKOFF_MS = 30000;
const UPDATE_CHECK_MS = 6 * 3600 * 1000;
const RECENT_MULLVAD_LIMIT = 5;

type RunCallback = (status: number, stdout: string, stderr: string) => void;

// GJS raises GLib.Error, which carries matches(); everything else reaching a
// catch here is a plain throw, and only its text is ever used.
function isCancelled(error: unknown): boolean {
    const candidate = error as {matches?: (domain: unknown, code: number) => boolean};
    return candidate?.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED) === true;
}

function spawn(argv: string[], flags: Gio.SubprocessFlags): Gio.Subprocess {
    return Gio.Subprocess.new(
        ['sh', '-c', `${PATH_PREAMBLE}exec "$@"`, 'sh', ...argv], flags);
}

// Everything this extension knows, read by one call to the tailgauge binary.
//
// There is no model here and no parsing: `tailgauge panel --json` probes for
// the CLIs, polls every installed provider at once, and returns the whole
// resolved panel. This class spawns it, pushes the answer into `panel`, and
// turns a row's action back into a command.
export const ProviderService = GObject.registerClass({
    Signals: {'changed': {}},
}, class ProviderService extends GObject.Object {
    declare _settings: Gio.Settings;
    declare _cancellables: Map<string, Gio.Cancellable>;
    declare _timeouts: Map<string, number>;
    declare _destroyed: boolean;
    declare _attentive: boolean;
    declare _settingsChangedId: number;

    /// The panel the binary last sent. Push-assigned rather than computed: it
    /// is on the far side of a process boundary now, so it arrives.
    declare panel: Panel.Panel;

    /// The optimistic toggle: -1 is "whatever the daemon says", 0 and 1 are a
    /// click that has not been reconciled yet.
    declare _desired: number;

    declare activeProviderId: string;
    declare actionStatus: string;
    declare lastError: string;
    declare updating: boolean;
    declare version: string;
    declare switchingAccountId: string;
    declare settingExitNodeId: string;
    declare selectingNetworkId: string;
    /// What the extension, not the daemon, decides: the hero phrase, the
    /// picker, the expanded row. Assigning it re-asks.
    declare ui: Panel.Ui;

    get installed(): boolean {
        return this.panel.header.toggleVisible;
    }

    get active(): boolean {
        return this.panel.header.toggleChecked;
    }

    get statusText(): string {
        return this.panel.status.text;
    }

    get busy(): boolean {
        return ['action', 'switch', 'exitNode', 'network', 'operator']
            .some(kind => this._cancellables.has(kind));
    }

    override _init(settings: Gio.Settings, version: string): void {
        super._init();
        this._settings = settings;
        this._cancellables = new Map();
        this._timeouts = new Map();
        this._destroyed = false;
        this._attentive = false;
        this._desired = -1;

        this.panel = Panel.emptyPanel();
        this.activeProviderId = settings.get_string('active-provider') ?? '';
        this.actionStatus = '';
        this.lastError = '';
        this.updating = false;
        this.version = version;
        this.switchingAccountId = '';
        this.settingExitNodeId = '';
        this.selectingNetworkId = '';
        this.ui = {};

        this._settingsChangedId = settings.connect('changed::refresh-interval',
            () => this._armPoll());

        this._armPoll();
        this._armUpdateCheck();
        this._watch();
        this.refresh();
    }

    set attentive(value: boolean) {
        if (this._attentive === value)
            return;
        this._attentive = value;
        this._armPoll();
        if (value)
            this.refresh();
    }

    get attentive(): boolean {
        return this._attentive;
    }

    // ---- asking -----------------------------------------------------------

    /// What the binary cannot read off the machine: which provider is being
    /// shown, what this side is optimistically showing, and what it has in
    /// flight.
    _ui(): Panel.Ui {
        const ui: Panel.Ui = {
            activeProviderId: this.activeProviderId,
            busy: this.busy,
            updating: this.updating,
            version: this.version,
            actionStatus: this.actionStatus,
            lastError: this.lastError,
            switchingAccountId: this.switchingAccountId,
            settingExitNodeId: this.settingExitNodeId,
            selectingNetworkId: this.selectingNetworkId,
            ...this.ui,
        };
        if (this._desired !== -1)
            ui.active = this._desired === 1;
        return ui;
    }

    refresh(): void {
        this._run('panel', ['tailgauge', 'panel', '--json', '--ui', JSON.stringify(this._ui())],
            (status, stdout, stderr) => {
                if (status !== 0) {
                    this.lastError = stderr.trim() || 'The panel could not be read';
                    return;
                }
                let next: Panel.Panel | null = null;
                try {
                    next = JSON.parse(stdout) as Panel.Panel;
                } catch (e) {
                    this.lastError = `The panel could not be read: ${e}`;
                    return;
                }
                if (!next?.header)
                    return;
                this.panel = next;
                // The daemon caught up with the click, so stop overriding it.
                if (this._desired !== -1 && next.header.toggleChecked === (this._desired === 1))
                    this._desired = -1;
            });
    }

    _run(kind: string, argv: string[], callback: RunCallback): boolean {
        if (this._cancellables.has(kind))
            return false;

        const cancellable = new Gio.Cancellable();
        this._cancellables.set(kind, cancellable);

        let proc: Gio.Subprocess;
        try {
            proc = spawn(argv, Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE);
        } catch (e) {
            this._cancellables.delete(kind);
            callback(1, '', `${e}`);
            return true;
        }

        proc.communicate_utf8_async(null, cancellable, (source, result) => {
            if (this._cancellables.get(kind) === cancellable)
                this._cancellables.delete(kind);
            let stdout = '';
            let stderr = '';
            let status = 1;
            try {
                const [, out, err] = source!.communicate_utf8_finish(result);
                stdout = out ?? '';
                stderr = err ?? '';
                status = source!.get_exit_status();
            } catch (e) {
                if (isCancelled(e))
                    return;
                stderr = `${e}`;
            }
            if (this._destroyed)
                return;
            callback(status, stdout, stderr);
            this.emit('changed');
        });
        return true;
    }

    _detach(argv: string[]): void {
        try {
            spawn(argv, Gio.SubprocessFlags.NONE);
        } catch (e) {
            this.lastError = `${e}`;
        }
    }

    // ---- acting -----------------------------------------------------------

    /// Every command is the binary's. Nothing here names a provider's CLI, so
    /// one provider's argv can never be fired at another's daemon.
    _ctl(kind: string, action: string, extra: string[] = []): boolean {
        const argv = ['tailgauge', 'ctl'];
        if (this.activeProviderId !== '')
            argv.push('--provider', this.activeProviderId);
        argv.push(action, ...extra);
        return this._run(kind, argv, (status, _stdout, stderr) => {
            if (kind === 'switch')
                this.switchingAccountId = '';
            else if (kind === 'exitNode')
                this.settingExitNodeId = '';
            else if (kind === 'network')
                this.selectingNetworkId = '';
            this.actionStatus = '';
            this.lastError = status === 0 ? '' : (stderr.trim() || 'The command failed');
            this._delayedRefresh();
        });
    }

    toggleTailscale(): void {
        if (!this.installed)
            return;
        if (this.active)
            this.down();
        else
            this.loginOrUp();
    }

    down(): void {
        // No progress status here: the greyed icon and hero line already
        // convey the optimistic off, so only a failure is worth a message.
        this._desired = 0;
        this.refresh();
        this._ctl('action', 'down');
    }

    // The binary scrapes the login URL off the daemon's own output and opens
    // it, so the state machine that used to live here is gone with it.
    loginOrUp(): void {
        if (!this.installed)
            return;
        this._desired = 1;
        this.refresh();
        if (this._ctl('action', 'up'))
            this.actionStatus = 'Turning it on…';
    }

    switchAccount(id: string): void {
        const accountId = String(id ?? '');
        if (!this.installed || accountId === '')
            return;
        if (this._ctl('switch', 'switch-account', [accountId]))
            this.switchingAccountId = accountId;
    }

    // The row's payload goes back as it came: which address a peer is reached
    // at is the model's rule, and a Mullvad node is set by address where a
    // tailnet one is set by name.
    setExitNode(peer: Panel.Payload | null): void {
        if (!this.installed || !peer)
            return;
        if (this._ctl('exitNode', 'exit-node', ['--peer', JSON.stringify(peer)]))
            this.settingExitNodeId = String(peer.id ?? '');
    }

    selectNetwork(network: Panel.Payload | null): void {
        if (!this.installed || !network)
            return;
        const id = String(network.id ?? '');
        if (id === '')
            return;
        const extra = network.selected === true ? [id, '--leave'] : [id];
        if (this._ctl('network', 'select-network', extra))
            this.selectingNetworkId = id;
    }

    authorizeTailscaleOperator(): void {
        if (!this.installed)
            return;
        if (this._ctl('operator', 'authorize'))
            this.actionStatus = 'Authorizing the operator…';
    }

    switchProvider(provider: Panel.Payload | null): void {
        const id = String(provider?.id ?? '');
        if (id === '' || id === this.activeProviderId)
            return;
        this.activeProviderId = id;
        this._settings.set_string('active-provider', id);
        // The panel belongs to the old provider until the next answer arrives.
        this.panel = Panel.emptyPanel();
        this.refresh();
    }

    rememberMullvadRegion(peer: Panel.Payload): void {
        const key = Panel.mullvadRegionKey(peer);
        if (key === '')
            return;
        this._settings.set_strv('recent-mullvad-regions', Panel.pushRecentMullvad(
            this._settings.get_strv('recent-mullvad-regions'), key, RECENT_MULLVAD_LIMIT));
    }

    openUrl(url: string): void {
        const target = String(url ?? '');
        if (target !== '')
            this._detach(['xdg-open', target]);
    }

    copyToClipboard(value: string): void {
        const text = String(value ?? '');
        if (text !== '')
            this._detach(['tailgauge', 'copy', text]);
    }

    copyPeerName(peer: Panel.Payload | null): void {
        if (peer)
            this.copyToClipboard(String(peer.DisplayName || peer.HostName || ''));
    }

    copyPeerDnsName(peer: Panel.Payload | null): void {
        if (peer)
            this.copyToClipboard(String(peer.DNSName ?? ''));
    }

    copyPeerIp(peer: Panel.Payload | null): void {
        if (peer?.IPv4?.length)
            this.copyToClipboard(peer.IPv4[0]);
    }

    sendFile(peer: Panel.Payload | null): void {
        if (peer)
            this._detach(['tailgauge', 'send', '--peer', JSON.stringify(peer)]);
    }

    checkUpdate(force = false): void {
        const argv = ['tailgauge', '--check-update'];
        if (force)
            argv.push('--force');
        this._run('update', argv, () => this.refresh());
    }

    applyUpdate(): void {
        if (this.updating)
            return;
        this.updating = true;
        this.actionStatus = 'Updating TailGauge…';
        this.refresh();
        this._run('applyUpdate', ['tailgauge', '--update'], (status, _stdout, stderr) => {
            this.updating = false;
            this.actionStatus = '';
            if (status !== 0)
                this.lastError = stderr.trim() || 'The update failed';
            this.checkUpdate(true);
        });
    }

    // ---- the clocks -------------------------------------------------------

    _timeout(kind: string, ms: number, handler: () => void): void {
        this._clearTimeout(kind);
        this._timeouts.set(kind, GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            this._timeouts.delete(kind);
            if (!this._destroyed)
                handler();
            return GLib.SOURCE_REMOVE;
        }));
    }

    _clearTimeout(kind: string): void {
        const id = this._timeouts.get(kind);
        if (id !== undefined) {
            GLib.source_remove(id);
            this._timeouts.delete(kind);
        }
    }

    /// The floor under the watcher, and the only thing running when there is
    /// no watcher to be had.
    _armPoll(): void {
        const configured = this._settings.get_int('refresh-interval');
        const seconds = this._attentive ? ATTENTIVE_INTERVAL_SEC : Math.max(5, configured || 30);
        this._timeout('poll', seconds * 1000, () => {
            this.refresh();
            this._armPoll();
        });
    }

    _armUpdateCheck(): void {
        this.checkUpdate();
        this._timeout('updateCheck', UPDATE_CHECK_MS, () => this._armUpdateCheck());
    }

    /// A command changes what the daemon will say next, so ask again once it
    /// has had a moment to settle rather than racing it.
    _delayedRefresh(): void {
        this._timeout('delayed', DELAYED_REFRESH_MS, () => this.refresh());
        this._timeout('actionStatus', ACTION_STATUS_MS, () => {
            this.actionStatus = '';
            this.emit('changed');
        });
    }

    /// The watcher carries the news: it blocks until the daemon reports a
    /// change, so a change made anywhere shows up at once.
    _watch(): void {
        this._run('watch', ['tailgauge', 'watch', String(WATCH_TIMEOUT_SEC)], status => {
            if (status === 0)
                this.refresh();
            // 0 is a change and 2 is an expired wait; anything else is a
            // broken watcher, so back off rather than spin.
            const rearm = status === 0 || status === 2 ? WATCH_REARM_MS : WATCH_BACKOFF_MS;
            this._timeout('rearmWatch', rearm, () => this._watch());
        });
    }

    destroy(): void {
        this._destroyed = true;
        for (const cancellable of this._cancellables.values())
            cancellable.cancel();
        this._cancellables.clear();
        for (const id of this._timeouts.values())
            GLib.source_remove(id);
        this._timeouts.clear();
        if (this._settingsChangedId) {
            this._settings.disconnect(this._settingsChangedId);
            this._settingsChangedId = 0;
        }
    }
});
