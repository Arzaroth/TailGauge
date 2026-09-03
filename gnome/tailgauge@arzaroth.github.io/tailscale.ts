import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import * as Model from './model.js';

const PATH_PREAMBLE = 'export PATH="$HOME/.local/bin:$HOME/bin:/usr/local/bin:$PATH"; ';

const POLL_WATCHDOG_MS = 15000;
const ACTION_STATUS_MS = 2200;
const LOGIN_TIMEOUT_MS = 10000;
const DELAYED_REFRESH_MS = 600;
const STARTUP_RAMP_MS = 2000;
const STARTUP_RAMP_TICKS = 15;
const ACCOUNTS_MAX_AGE_MS = 60000;
const ATTENTIVE_INTERVAL_SEC = 3;
const WATCH_TIMEOUT_SEC = 300;
const WATCH_REARM_MS = 250;
const WATCH_BACKOFF_MS = 30000;

const USER_KINDS = ['action', 'login', 'switch', 'exitNode', 'operator', 'applyUpdate'];

type RunCallback = (status: number, stdout: string, stderr: string) => void;

// GJS raises GLib.Error, which carries matches(); everything else reaching a
// catch here is a plain throw, and only its text is ever used.
function isCancelled(error: unknown): boolean {
    const candidate = error as {matches?: (domain: unknown, code: number) => boolean};
    return candidate?.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED) === true;
}

// gnome-shell inherits the session PATH, which often lacks the user bin dirs
// the installer drops the Taildrop and clipboard helpers into. `exec "$@"`
// keeps argv intact rather than pushing it back through the shell's parser.
function spawn(argv: string[], flags: Gio.SubprocessFlags): Gio.Subprocess {
    return Gio.Subprocess.new(
        ['sh', '-c', `${PATH_PREAMBLE}exec "$@"`, 'sh', ...argv], flags);
}

export const TailscaleService = GObject.registerClass({
    Signals: {'changed': {}},
}, class TailscaleService extends GObject.Object {
    declare _settings: Gio.Settings;
    declare _cancellables: Map<string, Gio.Cancellable>;
    declare _timeouts: Map<string, number>;
    declare _destroyed: boolean;
    declare _desired: number;
    declare _attentive: boolean;
    declare _lastAccountsRefreshMs: number;
    declare _loginInProgress: boolean;
    declare _loginUrlOpened: boolean;
    declare _preLoginAuthUrl: string;
    declare _startupTicks: number;
    declare _settingsChangedId: number;

    declare installed: boolean;
    declare running: boolean;
    declare needsLogin: boolean;
    declare refreshing: boolean;
    declare backendState: string;
    declare statusText: string;
    declare selfName: string;
    declare selfDnsName: string;
    declare selfIp: string;
    declare selfUserId: string;
    declare selfPeer: Model.Peer | null;
    declare fileSharing: boolean;
    declare authUrl: string;
    declare peers: Model.Peer[];
    declare exitNodes: Model.Peer[];
    declare tailnetExitNodes: Model.Peer[];
    declare mullvadExitNodes: Model.Peer[];
    declare mullvadRegions: Model.Peer[];
    declare accounts: Model.Account[];
    declare selectedAccountId: string;
    declare selectedAccountLabel: string;
    declare switchingAccountId: string;
    declare settingExitNodeId: string;
    declare accountsAccessDenied: boolean;
    declare actionStatus: string;
    declare lastError: string;
    declare helpers: boolean;
    declare update: Model.UpdateInfo;
    declare updating: boolean;
    declare version: string;

    // The extension's own version, out of the metadata it shipped with. The
    // extension and the helpers install separately, so the panel reports the
    // one it is actually running rather than the one the last release carried.
    override _init(settings: Gio.Settings, version: string = ''): void {
        super._init();

        this._settings = settings;
        this.version = version;
        this._cancellables = new Map();
        this._timeouts = new Map();
        this._destroyed = false;

        this.installed = false;
        this.running = false;
        this.needsLogin = false;
        // Optimistic off state so the UI reacts the instant you click, rather
        // than waiting for the next status refresh. -1 follows the real state,
        // 0/1 while a toggle is still catching up.
        this._desired = -1;
        this.refreshing = false;
        this.backendState = 'Unknown';
        this.statusText = _('Checking…');
        this.selfName = '';
        this.selfDnsName = '';
        this.selfIp = '';
        this.selfUserId = '';
        this.selfPeer = null;
        this.fileSharing = false;
        this.authUrl = '';
        this.peers = [];
        this.exitNodes = [];
        this.tailnetExitNodes = [];
        this.mullvadExitNodes = [];
        this.mullvadRegions = [];
        this.accounts = [];
        this.selectedAccountId = '';
        this.selectedAccountLabel = '';
        this.switchingAccountId = '';
        this.settingExitNodeId = '';
        this.accountsAccessDenied = false;
        this.actionStatus = '';
        this.lastError = '';

        // Assume the helpers are there until the probe says otherwise, so the
        // send action does not flicker away on a slow first poll.
        this.helpers = true;
        this._attentive = false;
        this.update = {available: false, updatable: false, latest: '', url: '', error: ''};
        this.updating = false;

        this._lastAccountsRefreshMs = 0;
        this._loginInProgress = false;
        this._loginUrlOpened = false;
        this._preLoginAuthUrl = '';
        this._startupTicks = 0;

        this._run('helpers', ['which', 'tailgauge-send'], status => {
            this.helpers = status === 0;
        });
        this.checkUpdate();
        // The helper caches its GitHub answer, so this mostly reads a file; the
        // interval is about how stale the banner may be, not about rate limits.
        this._timeouts.set('update', GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 6 * 3600, () => {
            this.checkUpdate();
            return GLib.SOURCE_CONTINUE;
        }));

        this._settingsChangedId = this._settings.connect('changed::refresh-interval', () => this._armRefresh());
        this._armRefresh();
        this._addTimeout('startup', STARTUP_RAMP_MS, () => {
            this._startupTicks += 1;
            if (this.running || this._startupTicks >= STARTUP_RAMP_TICKS)
                return GLib.SOURCE_REMOVE;
            this.refresh();
            return GLib.SOURCE_CONTINUE;
        });
        this.refresh();
    }

    get active(): boolean {
        return this._desired === -1 ? this.running : this._desired === 1;
    }

    // Only work the user asked for. A status poll or an update check is not
    // something the panel should ever report as busy, let alone gate a control
    // on: the poll runs every few seconds and would flicker the whole panel.
    get busy(): boolean {
        for (const kind of USER_KINDS) {
            if (this._cancellables.has(kind))
                return true;
        }
        return false;
    }

    // True while the menu is on screen. Polling follows the panel: fast enough
    // to feel live while it is open, lazy while it is not.
    set attentive(value: boolean) {
        if (this._attentive === value)
            return;
        this._attentive = value;
        this._armRefresh();
    }

    get attentive(): boolean {
        return this._attentive === true;
    }

    // The flat state resolvePanel() reads. Both desktops hand it the same
    // shape, so the panel they get back cannot disagree.
    snapshot(): Model.PanelState {
        return {
            installed: this.installed,
            running: this.running,
            active: this.active,
            needsLogin: this.needsLogin,
            busy: this.busy,
            selfName: this.selfName,
            selfIp: this.selfIp,
            selfUserId: this.selfUserId,
            selfPeer: this.selfPeer,
            fileSharing: this.fileSharing,
            peers: this.peers,
            tailnetExitNodes: this.tailnetExitNodes,
            mullvadRegions: this.mullvadRegions,
            accounts: this.accounts,
            selectedAccountId: this.selectedAccountId,
            switchingAccountId: this.switchingAccountId,
            settingExitNodeId: this.settingExitNodeId,
            accountsAccessDenied: this.accountsAccessDenied,
            actionStatus: this.actionStatus,
            lastError: this.lastError,
            helpers: this.helpers,
            update: this.update,
            updating: this.updating,
            version: this.version,
        };
    }

    // ---- plumbing --------------------------------------------------------

    _emit(): void {
        if (!this._destroyed)
            this.emit('changed');
    }

    _armRefresh(): void {
        const seconds = this.attentive
            ? ATTENTIVE_INTERVAL_SEC
            : Math.max(5, this._settings.get_int('refresh-interval'));
        this._removeTimeout('refresh');
        this._timeouts.set('refresh', GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            this.refresh();
            return GLib.SOURCE_CONTINUE;
        }));
    }

    _addTimeout(name: string, ms: number, callback: () => boolean): void {
        this._removeTimeout(name);
        this._timeouts.set(name, GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            const keep = callback();
            if (keep !== GLib.SOURCE_CONTINUE)
                this._timeouts.delete(name);
            return keep === GLib.SOURCE_CONTINUE ? GLib.SOURCE_CONTINUE : GLib.SOURCE_REMOVE;
        }));
    }

    _removeTimeout(name: string): void {
        const id = this._timeouts.get(name);
        if (id) {
            GLib.Source.remove(id);
            this._timeouts.delete(name);
        }
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
            this._emit();
        });
        return true;
    }

    _detach(argv: string[]): void {
        try {
            spawn(argv, Gio.SubprocessFlags.NONE);
        } catch (e) {
            logError(e as object, 'TailGauge');
        }
    }

    _cancel(kind: string): void {
        const cancellable = this._cancellables.get(kind);
        if (!cancellable)
            return;
        cancellable.cancel();
        this._cancellables.delete(kind);
    }

    // ---- polling ---------------------------------------------------------

    // A long poll that returns the moment tailscaled reports a change, so an
    // external `tailscale up` shows up at once instead of on the next tick.
    watch(): void {
        if (!this.installed || this._destroyed)
            return;
        this._run('watch', ['tailgauge-watch', String(WATCH_TIMEOUT_SEC)], status => {
            // 0 means something changed, 2 means the wait simply expired.
            // Anything else is a broken watcher, so back off rather than spin.
            if (status === 0)
                this.refresh();
            const delay = (status === 0 || status === 2) ? WATCH_REARM_MS : WATCH_BACKOFF_MS;
            this._addTimeout('rearmWatch', delay, () => {
                this.watch();
                return GLib.SOURCE_REMOVE;
            });
        });
    }

    checkUpdate(force = false): void {
        const argv = ['tailgauge-update', '--check', '--json'];
        if (force)
            argv.push('--force');
        // --check exits 2 when an update is available, which is a result, not a
        // failure.
        this._run('update', argv, (status, stdout) => {
            if (status !== 0 && status !== 2)
                return;
            try {
                this.update = JSON.parse(stdout);
            } catch (e) {
                this.update = {available: false, updatable: false, latest: '', url: '', error: ''};
            }
        });
    }

    applyUpdate(): void {
        if (this.updating || this.update?.updatable !== true)
            return;
        this.updating = true;
        this.actionStatus = _('Updating TailGauge…');
        this._run('applyUpdate', ['tailgauge-update', '--apply', '--quiet'], (status, stdout, stderr) => {
            this.updating = false;
            if (status !== 0) {
                this.lastError = Model.elideStatus(stderr || stdout || _('Update failed'));
                this._flashStatus(this.lastError);
            } else {
                this._flashStatus(_('Updated - log back in to load it'));
                this.checkUpdate(true);
            }
        });
        this._emit();
    }

    openUrl(url: string): void {
        const target = String(url || '');
        if (target !== '')
            Gio.AppInfo.launch_default_for_uri(target, null);
    }

    refresh(forceAccounts = false): void {
        if (this.installed) {
            this._refreshStatusAndAccounts(forceAccounts);
            return;
        }
        const started = this._run('which', ['which', 'tailscale'], (status) => {
            this.installed = status === 0;
            if (this.installed) {
                this._refreshStatusAndAccounts(false);
                if (!this._cancellables.has('watch'))
                    this.watch();
            } else {
                this.refreshing = false;
                this._resetUnavailable(_('Not installed'));
            }
        });
        if (started)
            this.refreshing = true;
    }

    _refreshStatusAndAccounts(forceAccounts: boolean): void {
        if (!this.installed)
            return;
        let launched = false;

        if (this._run('status', ['tailscale', 'status', '--json'], (status, stdout, stderr) => {
            this.refreshing = false;
            if (status === 0) {
                this._parseStatus(stdout);
            } else {
                this._resetUnavailable(_('Disconnected'));
                this.lastError = stderr.trim();
            }
        })) {
            this.refreshing = true;
            launched = true;
        }

        if (this._run('mullvad', ['tailscale', 'exit-node', 'list'], (status, stdout) => {
            this._parseMullvadExitNodes(status === 0 ? stdout : '');
        }))
            launched = true;

        const now = GLib.get_monotonic_time() / 1000;
        const stale = now - this._lastAccountsRefreshMs > ACCOUNTS_MAX_AGE_MS;
        if (forceAccounts || this.accounts.length === 0 || stale) {
            if (this._run('accounts', ['tailscale', 'switch', '--list', '--json'], (status, stdout, stderr) => {
                if (status === 0) {
                    this._parseAccounts(stdout);
                } else {
                    this._parseAccounts('');
                    if (Model.isProfilesAccessDenied(stderr) || Model.isProfilesAccessDenied(stdout)) {
                        this.accountsAccessDenied = true;
                        this.lastError = _('Authorize Tailscale operator to show connections');
                    } else {
                        this.lastError = Model.elideStatus(stderr || stdout || _('Could not list Tailscale connections'));
                    }
                }
            })) {
                this._lastAccountsRefreshMs = now;
                launched = true;
            }
        }

        // Arm on the launch that needs watching and leave it alone after that.
        // Restarting it every refresh pushes the deadline out ahead of a hung
        // process forever once the refresh interval is shorter than the
        // timeout, and the interval goes down to five seconds.
        if (launched && !this._timeouts.has('watchdog')) {
            this._addTimeout('watchdog', POLL_WATCHDOG_MS, () => {
                // Every poll is skipped while its own process is still running,
                // so one that never exits silently stops the panel refreshing
                // at all, and it stays stopped.
                this._cancel('status');
                this._cancel('mullvad');
                this._cancel('accounts');
                this.refreshing = false;
                this._emit();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    _delayedRefresh(): void {
        this._addTimeout('delayed', DELAYED_REFRESH_MS, () => {
            this.refresh();
            return GLib.SOURCE_REMOVE;
        });
    }

    _flashStatus(message: string): void {
        this.actionStatus = message;
        this._addTimeout('actionStatus', ACTION_STATUS_MS, () => {
            this.actionStatus = '';
            this._emit();
            return GLib.SOURCE_REMOVE;
        });
    }

    // ---- parsing ---------------------------------------------------------

    _resetUnavailable(message: string): void {
        this.running = false;
        this.needsLogin = false;
        this._desired = -1;
        this.backendState = 'Unavailable';
        this.statusText = message;
        this.selfName = '';
        this.selfDnsName = '';
        this.selfIp = '';
        this.selfUserId = '';
        this.selfPeer = null;
        this.fileSharing = false;
        this.authUrl = '';
        this.peers = [];
        this.exitNodes = [];
        this.tailnetExitNodes = [];
        this.mullvadExitNodes = [];
        this.mullvadRegions = [];
        this.accounts = [];
        this.selectedAccountId = '';
        this.selectedAccountLabel = '';
        this.switchingAccountId = '';
        this.settingExitNodeId = '';
        this.accountsAccessDenied = false;
    }

    _parseStatus(raw: string): void {
        const parsed = Model.parseStatus(raw);
        if (!parsed.ok) {
            this._resetUnavailable(parsed.message || _('Status error'));
            this.lastError = parsed.error || 'Failed to parse tailscale status';
            return;
        }
        if (parsed.unavailable) {
            this._resetUnavailable(parsed.message || _('Disconnected'));
            return;
        }

        this.backendState = parsed.backendState;
        this.running = parsed.running;
        // Reality caught up to the pending toggle, so stop overriding.
        if (this._desired !== -1 && this.running === (this._desired === 1))
            this._desired = -1;
        this.needsLogin = parsed.needsLogin;
        this.authUrl = parsed.authUrl;
        if (this.needsLogin && this._loginInProgress && !this._loginUrlOpened &&
            this.authUrl !== '' && this.authUrl !== this._preLoginAuthUrl)
            this._openAuthUrlFrom(this.authUrl, false);
        this.selfName = parsed.selfName;
        this.selfDnsName = parsed.selfDnsName;
        this.selfIp = parsed.selfIp;
        this.selfUserId = parsed.selfUserId;
        this.selfPeer = parsed.selfPeer;
        this.fileSharing = parsed.fileSharing;
        this.peers = parsed.running ? parsed.peers : [];
        this.tailnetExitNodes = parsed.running ? parsed.exitNodes : [];
        this.exitNodes = parsed.running ? this.tailnetExitNodes.concat(this.mullvadRegions) : [];

        if (this.needsLogin) {
            this.statusText = _('Needs login');
        } else if (this.running) {
            this.statusText = _('Connected');
            this._loginInProgress = false;
            this._loginUrlOpened = false;
            this._preLoginAuthUrl = '';
            this._removeTimeout('loginTimeout');
        } else if (this.backendState === 'Stopped') {
            this.statusText = _('Disconnected');
        } else {
            this.statusText = this.backendState;
        }
        this.lastError = '';
    }

    _parseAccounts(raw: string): void {
        const parsed = Model.parseAccounts(raw);
        this.accounts = parsed.accounts;
        this.selectedAccountId = parsed.selectedAccountId;
        this.selectedAccountLabel = parsed.selectedAccountLabel;
        this.accountsAccessDenied = false;
    }

    _parseMullvadExitNodes(raw: string): void {
        this.mullvadExitNodes = Model.parseExitNodeList(raw);
        this.mullvadRegions = Model.mullvadRegionOptions(this.mullvadExitNodes);
        this.exitNodes = this.running ? this.tailnetExitNodes.concat(this.mullvadRegions) : [];
    }

    // ---- actions ---------------------------------------------------------

    toggleTailscale(): void {
        if (!this.installed)
            return;
        if (this.active)
            this.down();
        else
            this.loginOrUp();
    }

    down(): void {
        // No progress status here: the greyed icon and header line already
        // convey the optimistic off, so only a failure is worth a message.
        this._desired = 0;
        this._run('action', ['tailscale', 'down'], (status, stdout, stderr) => {
            if (status !== 0) {
                this._desired = -1;
                this.lastError = Model.elideStatus(stderr || stdout || _('Tailscale command failed'));
                this._flashStatus(this.lastError);
            } else {
                this.lastError = '';
                this.actionStatus = '';
            }
            this._delayedRefresh();
        });
        this._emit();
    }

    loginOrUp(): void {
        if (!this.installed || this._cancellables.has('login'))
            return;
        this._desired = -1;
        const plan = Model.loginPlan(this.needsLogin, this.authUrl);
        if (plan.authUrl !== '') {
            this._loginUrlOpened = false;
            this._openAuthUrlFrom(plan.authUrl, true);
            return;
        }
        if (this.needsLogin)
            this.actionStatus = _('Starting Tailscale login…');
        else
            this._desired = 1;
        this._loginInProgress = this.needsLogin;
        this._loginUrlOpened = false;
        this._preLoginAuthUrl = this.authUrl;
        this._startLogin(plan.command);
        if (this.needsLogin) {
            this._addTimeout('loginTimeout', LOGIN_TIMEOUT_MS, () => {
                if (this._loginInProgress && !this._loginUrlOpened &&
                    !this._openAuthUrlFrom(this.authUrl, true)) {
                    this._loginInProgress = false;
                    this.actionStatus = _('Tailscale login link not available yet');
                    this._emit();
                }
                return GLib.SOURCE_REMOVE;
            });
        }
        this._emit();
    }

    // `tailscale up` prints the authorization URL and then blocks until the
    // browser round-trip finishes, so the line has to be read as it arrives
    // rather than collected at exit.
    _startLogin(argv: string[]): void {
        const cancellable = new Gio.Cancellable();
        this._cancellables.set('login', cancellable);

        let proc: Gio.Subprocess;
        try {
            proc = spawn(argv, Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE);
        } catch (e) {
            this._cancellables.delete('login');
            this._desired = -1;
            this._loginInProgress = false;
            this.lastError = `${e}`;
            this._flashStatus(this.lastError);
            return;
        }

        const collected: string[] = [];
        for (const stream of [proc.get_stdout_pipe(), proc.get_stderr_pipe()]) {
            if (!stream)
                continue;
            const reader = new Gio.DataInputStream({base_stream: stream});
            this._readLoginLine(reader, cancellable, collected);
        }

        proc.wait_async(cancellable, (source, result) => {
            if (this._cancellables.get('login') === cancellable)
                this._cancellables.delete('login');
            let status = 1;
            try {
                source!.wait_finish(result);
                status = source!.get_exit_status();
            } catch (e) {
                if (isCancelled(e))
                    return;
            }
            if (this._destroyed)
                return;

            const combined = collected.join('\n');
            const opened = this._openAuthUrlFrom(combined, true);
            if (status !== 0 && !opened) {
                this._desired = -1;
                this._loginInProgress = false;
                this.lastError = Model.elideStatus(combined || 'tailscale up failed');
                this._flashStatus(this.lastError);
            } else if (!opened) {
                this.lastError = '';
                this.actionStatus = '';
            }
            this._delayedRefresh();
            this._emit();
        });
    }

    _readLoginLine(reader: Gio.DataInputStream, cancellable: Gio.Cancellable, collected: string[]): void {
        reader.read_line_async(GLib.PRIORITY_DEFAULT, cancellable, (source, result) => {
            let line: string | null = null;
            try {
                [line] = source!.read_line_finish_utf8(result);
            } catch (e) {
                return;
            }
            if (line === null || this._destroyed)
                return;
            collected.push(line);
            if (this._loginInProgress && !this._loginUrlOpened)
                this._openAuthUrlFrom(line, false);
            this._readLoginLine(source!, cancellable, collected);
        });
    }

    _openAuthUrlFrom(text: string, allowFallback: boolean): boolean {
        if (this._loginUrlOpened)
            return true;
        const url = Model.firstUrl(text, allowFallback ? this.authUrl : '');
        if (url === '')
            return false;
        // Turning on ended up needing browser auth, so stop pretending we're up.
        this._desired = -1;
        this._loginUrlOpened = true;
        this._loginInProgress = false;
        this._removeTimeout('loginTimeout');
        Gio.AppInfo.launch_default_for_uri(url, null);
        return true;
    }

    switchAccount(id: string): void {
        const accountId = String(id || '');
        if (!this.installed || accountId === '' || accountId === this.selectedAccountId)
            return;
        const started = this._run('switch', ['tailscale', 'switch', accountId], (status, stdout, stderr) => {
            if (status !== 0) {
                this.lastError = Model.elideStatus(stderr || stdout || _('Account switch failed'));
                this._flashStatus(this.lastError);
            } else {
                this.lastError = '';
                this.actionStatus = '';
                this._lastAccountsRefreshMs = 0;
            }
            this.switchingAccountId = '';
            this._delayedRefresh();
        });
        if (started) {
            this.switchingAccountId = accountId;
            this._emit();
        }
    }

    setExitNode(peer: Model.Peer | null | undefined): void {
        if (!this.installed || !this.running || !peer)
            return;
        const isActive = peer.ExitNode === true;
        const target = isActive ? '' : Model.exitNodeTarget(peer);
        if (!isActive && target === '')
            return;
        const started = this._run('exitNode', ['tailscale', 'set', `--exit-node=${target}`], (status, stdout, stderr) => {
            if (status !== 0) {
                this.lastError = Model.elideStatus(stderr || stdout || _('Exit node selection failed'));
                this._flashStatus(this.lastError);
            } else {
                this.lastError = '';
                this.actionStatus = '';
            }
            this.settingExitNodeId = '';
            this._delayedRefresh();
        });
        if (started) {
            this.settingExitNodeId = String(peer.id || '');
            this._emit();
        }
    }

    authorizeTailscaleOperator(): void {
        if (!this.installed)
            return;
        const user = GLib.get_user_name();
        const started = this._run('operator', ['pkexec', 'tailscale', 'set', `--operator=${user}`], (status, stdout, stderr) => {
            if (status !== 0) {
                this.lastError = Model.elideStatus(stderr || stdout || _('Tailscale authorization failed'));
                this._flashStatus(this.lastError);
            } else {
                this.accountsAccessDenied = false;
                this.lastError = '';
                this._flashStatus(_('Tailscale operator authorized'));
                this._lastAccountsRefreshMs = 0;
            }
            this._delayedRefresh();
        });
        if (started) {
            this.actionStatus = _('Authorizing Tailscale operator…');
            this._emit();
        }
    }

    // ---- clipboard and Taildrop -----------------------------------------

    copyToClipboard(value: string): void {
        const text = String(value || '');
        if (text === '')
            return;
        St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, text);
    }

    copyPeerIp(peer: Model.Peer | null | undefined): void {
        if (!peer)
            return;
        const ips = Model.filterIPv4(peer.TailscaleIPs || []);
        this.copyToClipboard(ips.length > 0 ? ips[0] : '');
    }

    copyPeerName(peer: Model.Peer | null | undefined): void {
        if (!peer)
            return;
        this.copyToClipboard(Model.displayHostName(peer.HostName, peer.DNSName));
    }

    copyPeerDnsName(peer: Model.Peer | null | undefined): void {
        if (!peer)
            return;
        this.copyToClipboard(Model.cleanDnsName(peer.DNSName));
    }

    canSendFiles(peer: Model.Peer | null | undefined): boolean {
        if (!this.fileSharing || !this.running || !peer)
            return false;
        return Model.isTaildropTarget(peer, this.selfUserId);
    }

    sendFile(peer: Model.Peer | null | undefined): void {
        if (!this.canSendFiles(peer))
            return;
        const target = Model.peerAddress(peer);
        if (target === '')
            return;
        this._detach(['tailgauge-send', target]);
    }

    destroy(): void {
        this._destroyed = true;
        for (const cancellable of this._cancellables.values())
            cancellable.cancel();
        this._cancellables.clear();
        for (const id of this._timeouts.values())
            GLib.Source.remove(id);
        this._timeouts.clear();
        if (this._settingsChangedId) {
            this._settings.disconnect(this._settingsChangedId);
            this._settingsChangedId = 0;
        }
    }
});
