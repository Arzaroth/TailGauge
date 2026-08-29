import QtQuick
import org.kde.plasma.plasma5support as Plasma5Support
import "../code/model.js" as Model

Item {
    id: root

    property int refreshIntervalSec: 30

    property bool installed: false
    property bool running: false
    property bool needsLogin: false

    // Optimistic off state so the UI reacts the instant you click, rather than
    // waiting for the next status refresh. _desired is -1 while we just follow
    // the real state, or 0/1 while a toggle is still catching up.
    property int _desired: -1
    readonly property bool active: _desired === -1 ? running : (_desired === 1)
    property bool refreshing: false
    property string backendState: "Unknown"
    property string statusText: "Checking…"
    property string selfName: ""
    property string selfDnsName: ""
    property string selfIp: ""
    property string selfUserId: ""
    property bool fileSharing: false
    property string authUrl: ""
    property var peers: []
    property var exitNodes: []
    property var tailnetExitNodes: []
    property var mullvadExitNodes: []
    property var mullvadRegions: []
    property var accounts: []
    property string selectedAccountId: ""
    property string selectedAccountLabel: ""
    property string switchingAccountId: ""
    property string settingExitNodeId: ""
    property bool accountsAccessDenied: false
    property string actionStatus: ""
    property string lastError: ""

    property var _inflight: ({})
    property int _inflightCount: 0
    property var _kinds: ({})
    property int _seq: 0
    property double _lastAccountsRefreshMs: 0
    property bool _loginInProgress: false
    property bool _loginUrlOpened: false
    property string _preLoginAuthUrl: ""

    readonly property bool busy: _inflightCount > 0

    function osIcon(os) { return Model.osIcon(os) }
    function accountLabel(account) { return Model.accountLabel(account) }
    function displayHostName(hostName, dnsName) { return Model.displayHostName(hostName, dnsName) }
    function cleanDnsName(name) { return Model.cleanDnsName(name) }
    function peerAddress(peer) { return Model.peerAddress(peer) }

    // ---- command plumbing ---------------------------------------------------

    Plasma5Support.DataSource {
        id: exec
        engine: "executable"
        connectedSources: []
        onNewData: (source, data) => {
            var kind = root._kinds[source] || ""
            exec.disconnectSource(source)
            root._release(source)
            if (kind !== "")
                root._handle(kind, Number(data["exit code"]),
                             String(data.stdout || ""), String(data.stderr || ""))
        }
    }

    // plasmashell's session PATH often lacks ~/.local/bin, which is where the
    // installer drops the Taildrop and clipboard helpers.
    function _shellCommand(command) {
        return "sh -c " + Model.shellQuote(
            'export PATH="$HOME/.local/bin:$HOME/bin:/usr/local/bin:$PATH"; ' + command)
    }

    // A unique suffix per invocation: the executable engine keys sources by
    // their command line, so two runs of the same command would otherwise
    // collide, and a source reaped by the watchdog would block its own retry.
    function _run(kind, argv) {
        return _runShell(kind, Model.shellCommand(argv))
    }

    function _runShell(kind, command) {
        if (root._inflight[kind]) return false
        root._seq += 1
        var source = _shellCommand(command) + " # tailgauge-" + root._seq
        var kinds = root._kinds
        kinds[source] = kind
        root._kinds = kinds
        var inflight = root._inflight
        inflight[kind] = source
        root._inflight = inflight
        root._inflightCount += 1
        exec.connectSource(source)
        return true
    }

    function _release(source) {
        var kinds = root._kinds
        var kind = kinds[source] || ""
        if (kind === "") return
        delete kinds[source]
        root._kinds = kinds
        var inflight = root._inflight
        if (inflight[kind] === source) {
            delete inflight[kind]
            root._inflight = inflight
            root._inflightCount = Math.max(0, root._inflightCount - 1)
        }
    }

    function _reap(kind) {
        var source = root._inflight[kind]
        if (!source) return
        exec.disconnectSource(source)
        root._release(source)
    }

    function _detach(argv) {
        root._seq += 1
        var source = _shellCommand(Model.shellCommand(argv)) + " >/dev/null 2>&1 # tailgauge-detach-" + root._seq
        var kinds = root._kinds
        kinds[source] = "detached"
        root._kinds = kinds
        exec.connectSource(source)
    }

    function _handle(kind, exitCode, stdout, stderr) {
        if (kind === "which") {
            root.installed = exitCode === 0
            if (root.installed) root.refreshStatusAndAccounts()
            else {
                root.refreshing = false
                root.resetUnavailable("Not installed")
            }
        } else if (kind === "status") {
            root.refreshing = false
            if (exitCode === 0) root.parseStatus(stdout)
            else {
                root.resetUnavailable("Disconnected")
                root.lastError = stderr.trim()
            }
        } else if (kind === "accounts") {
            if (exitCode === 0) root.parseAccounts(stdout)
            else {
                root.parseAccounts("")
                if (Model.isProfilesAccessDenied(stderr) || Model.isProfilesAccessDenied(stdout)) {
                    root.accountsAccessDenied = true
                    root.lastError = "Authorize Tailscale operator to show connections"
                } else {
                    root.lastError = Model.elideStatus(stderr || stdout || "Could not list Tailscale connections")
                }
            }
        } else if (kind === "mullvad") {
            root.parseMullvadExitNodes(exitCode === 0 ? stdout : "")
        } else if (kind === "action") {
            if (exitCode !== 0) {
                root._desired = -1
                root.lastError = Model.elideStatus(stderr || stdout || "Tailscale command failed")
                root.actionStatus = root.lastError
                actionStatusTimer.restart()
            } else {
                root.lastError = ""
                root.actionStatus = ""
            }
            delayedRefresh.restart()
        } else if (kind === "login") {
            var combined = stdout + "\n" + stderr
            var opened = root.openAuthUrlFrom(combined, true)
            if (exitCode !== 0 && !opened) {
                root._desired = -1
                root._loginInProgress = false
                root.lastError = Model.elideStatus(combined || "tailscale up failed")
                root.actionStatus = root.lastError
                actionStatusTimer.restart()
            } else if (!opened) {
                root.lastError = ""
                root.actionStatus = ""
            }
            delayedRefresh.restart()
        } else if (kind === "switch") {
            if (exitCode !== 0) {
                root.lastError = Model.elideStatus(stderr || stdout || "Account switch failed")
                root.actionStatus = root.lastError
                actionStatusTimer.restart()
            } else {
                root.lastError = ""
                root.actionStatus = ""
                root._lastAccountsRefreshMs = 0
            }
            root.switchingAccountId = ""
            delayedRefresh.restart()
        } else if (kind === "exitNode") {
            if (exitCode !== 0) {
                root.lastError = Model.elideStatus(stderr || stdout || "Exit node selection failed")
                root.actionStatus = root.lastError
                actionStatusTimer.restart()
            } else {
                root.lastError = ""
                root.actionStatus = ""
            }
            root.settingExitNodeId = ""
            delayedRefresh.restart()
        } else if (kind === "operator") {
            if (exitCode !== 0) {
                root.lastError = Model.elideStatus(stderr || stdout || "Tailscale authorization failed")
                root.actionStatus = root.lastError
                actionStatusTimer.restart()
            } else {
                root.accountsAccessDenied = false
                root.lastError = ""
                root.actionStatus = "Tailscale operator authorized"
                actionStatusTimer.restart()
                root._lastAccountsRefreshMs = 0
            }
            delayedRefresh.restart()
        }
    }

    // ---- actions ------------------------------------------------------------

    function copyToClipboard(value) {
        var text = String(value || "")
        if (text === "") return
        _detach(["tailgauge-copy", text])
    }

    function copyPeerIp(peer) {
        if (!peer) return
        var ips = Model.filterIPv4(peer.TailscaleIPs || [])
        copyToClipboard(ips.length > 0 ? ips[0] : "")
    }

    function copyPeerName(peer) {
        if (!peer) return
        copyToClipboard(Model.displayHostName(peer.HostName, peer.DNSName))
    }

    function copyPeerDnsName(peer) {
        if (!peer) return
        copyToClipboard(Model.cleanDnsName(peer.DNSName))
    }

    function canSendFiles(peer) {
        if (!fileSharing || !running || !peer) return false
        return Model.isTaildropTarget(peer, selfUserId)
    }

    function sendFile(peer) {
        if (!canSendFiles(peer)) return
        var target = Model.peerAddress(peer)
        if (target === "") return
        _detach(["tailgauge-send", target])
    }

    function refresh(forceAccounts) {
        if (installed) {
            refreshStatusAndAccounts(forceAccounts === true)
            return
        }
        if (_run("which", ["which", "tailscale"])) refreshing = true
    }

    function refreshStatusAndAccounts(forceAccounts) {
        if (!installed) return
        var launched = false
        if (_run("status", ["tailscale", "status", "--json"])) {
            refreshing = true
            launched = true
        }
        if (_run("mullvad", ["tailscale", "exit-node", "list"])) launched = true

        var now = Date.now()
        var shouldRefreshAccounts = forceAccounts === true || accounts.length === 0 || now - _lastAccountsRefreshMs > 60000
        if (shouldRefreshAccounts) {
            if (_run("accounts", ["tailscale", "switch", "--list", "--json"])) {
                _lastAccountsRefreshMs = now
                launched = true
            }
        }
        // Arm on the launch that needs watching and leave it alone after that.
        // Restarting it every refresh pushes the deadline out ahead of a hung
        // process forever once the refresh interval is shorter than the timeout,
        // and refreshIntervalSec goes down to five seconds.
        if (launched && !pollWatchdog.running) pollWatchdog.start()
    }

    function resetUnavailable(message) {
        running = false
        needsLogin = false
        _desired = -1
        backendState = "Unavailable"
        statusText = message
        selfName = ""
        selfDnsName = ""
        selfIp = ""
        selfUserId = ""
        fileSharing = false
        authUrl = ""
        peers = []
        exitNodes = []
        tailnetExitNodes = []
        mullvadExitNodes = []
        mullvadRegions = []
        accounts = []
        selectedAccountId = ""
        selectedAccountLabel = ""
        switchingAccountId = ""
        settingExitNodeId = ""
        accountsAccessDenied = false
    }

    function parseStatus(raw) {
        var parsed = Model.parseStatus(raw)
        if (!parsed.ok) {
            resetUnavailable(parsed.message || "Status error")
            lastError = parsed.error || "Failed to parse tailscale status"
            return
        }
        if (parsed.unavailable) {
            resetUnavailable(parsed.message || "Disconnected")
            return
        }

        backendState = parsed.backendState
        running = parsed.running
        // Reality caught up to the pending toggle, so stop overriding.
        if (_desired !== -1 && running === (_desired === 1)) _desired = -1
        needsLogin = parsed.needsLogin
        authUrl = parsed.authUrl
        if (needsLogin && _loginInProgress && !_loginUrlOpened && authUrl !== "" && authUrl !== _preLoginAuthUrl)
            openAuthUrlFrom(authUrl, false)
        selfName = parsed.selfName
        selfDnsName = parsed.selfDnsName
        selfIp = parsed.selfIp
        selfUserId = parsed.selfUserId
        fileSharing = parsed.fileSharing
        peers = parsed.running ? parsed.peers : []
        tailnetExitNodes = parsed.running ? parsed.exitNodes : []
        exitNodes = parsed.running ? tailnetExitNodes.concat(mullvadRegions) : []

        if (needsLogin) statusText = "Needs login"
        else if (running) {
            statusText = "Connected"
            _loginInProgress = false
            _loginUrlOpened = false
            _preLoginAuthUrl = ""
            loginTimeoutTimer.stop()
        } else if (backendState === "Stopped") {
            statusText = "Disconnected"
        } else {
            statusText = backendState
        }
        lastError = ""
    }

    function parseAccounts(raw) {
        var parsed = Model.parseAccounts(raw)
        accounts = parsed.accounts
        selectedAccountId = parsed.selectedAccountId
        selectedAccountLabel = parsed.selectedAccountLabel
        accountsAccessDenied = false
    }

    function parseMullvadExitNodes(raw) {
        mullvadExitNodes = Model.parseExitNodeList(raw)
        mullvadRegions = Model.mullvadRegionOptions(mullvadExitNodes)
        exitNodes = running ? tailnetExitNodes.concat(mullvadRegions) : []
    }

    function toggleTailscale() {
        if (!installed) return
        if (active) down()
        else loginOrUp()
    }

    function down() {
        // No progress status here: the greyed icon and hero line already convey
        // the optimistic off, so only a failure is worth a message.
        _desired = 0
        _run("action", ["tailscale", "down"])
    }

    function loginOrUp() {
        if (!installed || _inflight["login"]) return
        _desired = -1
        var plan = Model.loginPlan(needsLogin, authUrl)
        if (plan.authUrl !== "") {
            _loginUrlOpened = false
            openAuthUrlFrom(plan.authUrl, true)
            return
        }
        if (needsLogin) actionStatus = "Starting Tailscale login…"
        else _desired = 1
        _loginInProgress = needsLogin
        _loginUrlOpened = false
        _preLoginAuthUrl = authUrl
        _run("login", plan.command)
        if (needsLogin) loginTimeoutTimer.restart()
    }

    function switchAccount(id) {
        var accountId = String(id || "")
        if (!installed || accountId === "" || accountId === selectedAccountId) return
        if (_run("switch", ["tailscale", "switch", accountId])) switchingAccountId = accountId
    }

    function setExitNode(peer) {
        if (!installed || !running || !peer) return
        var isActive = peer.ExitNode === true
        var target = isActive ? "" : Model.exitNodeTarget(peer)
        if (!isActive && target === "") return
        if (_run("exitNode", ["tailscale", "set", "--exit-node=" + target]))
            settingExitNodeId = String(peer.id || "")
    }

    // A plasmoid cannot read the environment, so the shell that runs the
    // command resolves the user name it needs to authorize.
    function authorizeTailscaleOperator() {
        if (!installed) return
        if (_runShell("operator", 'pkexec tailscale set --operator="$(id -un)"'))
            actionStatus = "Authorizing Tailscale operator…"
    }

    function openAuthUrlFrom(text, allowFallback) {
        if (_loginUrlOpened) return true
        var url = Model.firstUrl(text, allowFallback === true ? authUrl : "")
        if (url !== "") {
            // Turning on ended up needing browser auth, so stop pretending
            // we're up.
            _desired = -1
            _loginUrlOpened = true
            _loginInProgress = false
            loginTimeoutTimer.stop()
            Qt.openUrlExternally(url)
            return true
        }
        return false
    }

    // ---- timers -------------------------------------------------------------

    Timer {
        interval: Math.max(5, root.refreshIntervalSec) * 1000
        repeat: true
        running: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }

    Timer {
        // After a fresh login session the first poll usually lands before
        // tailscaled has connected, which left the icon stale until the next
        // periodic refresh. Poll quickly until the service shows up, or give up
        // after ~30 seconds.
        id: startupRamp
        property int ticks: 0
        interval: 2000
        repeat: true
        running: true
        onTriggered: {
            ticks += 1
            if (root.running || ticks >= 15) startupRamp.running = false
            else root.refresh()
        }
    }

    Timer {
        id: delayedRefresh
        interval: 600
        repeat: false
        onTriggered: root.refresh()
    }

    Timer {
        // Every poll is skipped while its own command is still running, so one
        // that never exits - tailscale can hang on a network that is coming and
        // going - silently stops the panel refreshing at all, and it stays
        // stopped. Reap anything still running well inside the refresh interval
        // so the next tick starts clean.
        id: pollWatchdog
        interval: 15000
        repeat: false
        onTriggered: {
            root._reap("status")
            root._reap("mullvad")
            root._reap("accounts")
            root.refreshing = false
        }
    }

    Timer {
        id: actionStatusTimer
        interval: 2200
        repeat: false
        onTriggered: root.actionStatus = ""
    }

    Timer {
        id: loginTimeoutTimer
        interval: 10000
        repeat: false
        onTriggered: {
            if (!root._loginInProgress || root._loginUrlOpened) return
            if (!root.openAuthUrlFrom(root.authUrl, true)) {
                root._loginInProgress = false
                root.actionStatus = "Tailscale login link not available yet"
            }
        }
    }
}
