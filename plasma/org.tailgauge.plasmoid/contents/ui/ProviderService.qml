import QtQuick
import org.kde.plasma.plasma5support as Plasma5Support
import "../code/model.js" as Model

Item {
    id: root

    property int refreshIntervalSec: 30

    // Every provider probed on PATH, and the one the panel drives. `installed`
    // stays the gate every command already checks: it now means the active
    // provider is one we can actually drive.
    signal providerChanged(string id)

    // Which provider each poll was launched for. A reply that outlives a
    // switch belongs to the provider that was asked, not the one now shown.
    property var _pollProvider: ({})

    // One entry per installed provider, kept whether or not it is the one on
    // screen, so the bar icon and its tooltip describe the machine.
    // The last good panel for each provider, so switching shows what that
    // provider looked like a moment ago rather than an empty panel until its
    // first poll lands.
    property var _cache: ({})
    property string _cachedProviderId: ""

    property var summaries: []
    property int _bgIndex: 0
    property string _bgProviderId: ""

    property var networks: []
    property string selectingNetworkId: ""
    property var providers: []
    property string activeProviderId: ""
    property bool installed: false
    property bool running: false
    property bool needsLogin: false

    // Optimistic off state so the UI reacts the instant you click, rather than
    // waiting for the next status refresh. _desired is -1 while we just follow
    // the real state, or 0/1 while a toggle is still catching up.
    property int _desired: -1
    readonly property bool active: _desired === -1 ? running : (_desired === 1)
    property bool refreshing: false
    property string daemonState: "Unknown"
    property string statusText: "Checking…"
    property string selfName: ""
    property string selfDnsName: ""
    property string selfIp: ""
    property string selfUserId: ""
    property var selfPeer: null
    property bool fileSharing: false
    property string authUrl: ""
    property var peers: []
    property var exitNodes: []
    property var ownExitNodes: []
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

    // Assume the helpers are there until the probe says otherwise, so the send
    // button does not flicker away on a slow first poll.
    property bool helpers: true
    property var update: ({ available: false, current: "", latest: "" })
    property bool updating: false

    // The applet's own version, handed down from the metadata it shipped with.
    // The widget and the helpers install separately, so the panel reports the
    // one it is actually running rather than the one the last release carried.
    property string version: ""

    property var _inflight: ({})
    property int _inflightCount: 0
    property var _kinds: ({})
    property int _seq: 0
    property double _lastAccountsRefreshMs: 0
    property bool _loginInProgress: false
    property bool _loginUrlOpened: false
    property string _preLoginAuthUrl: ""

    // Only work the user asked for. A status poll or an update check is not
    // something the panel should ever report as busy, let alone gate a control
    // on: the poll runs every few seconds and would flicker the whole panel.
    readonly property var _userKinds: ["action", "login", "switch", "exitNode", "operator", "applyUpdate"]
    property int _userInflight: 0
    readonly property bool busy: _userInflight > 0

    // True while the popup is on screen. Polling follows the panel: fast enough
    // to feel live while it is open, lazy while it is not.
    property bool attentive: false

    // The flat state resolvePanel() reads. Both desktops hand it the same
    // shape, so the panel they get back cannot disagree.
    function _commands() {
        return Model.providerCommands({
            providers: providers,
            activeProviderId: activeProviderId
        }) || {}
    }

    function snapshot() {
        return {
            summaries: summaries,
            networks: networks,
            selectingNetworkId: selectingNetworkId,
            providers: providers,
            activeProviderId: activeProviderId,
            installed: installed,
            running: running,
            active: active,
            needsLogin: needsLogin,
            busy: busy,
            selfName: selfName,
            selfIp: selfIp,
            selfUserId: selfUserId,
            selfPeer: selfPeer,
            fileSharing: fileSharing,
            peers: peers,
            ownExitNodes: ownExitNodes,
            mullvadRegions: mullvadRegions,
            accounts: accounts,
            selectedAccountId: selectedAccountId,
            switchingAccountId: switchingAccountId,
            settingExitNodeId: settingExitNodeId,
            accountsAccessDenied: accountsAccessDenied,
            actionStatus: actionStatus,
            lastError: lastError,
            helpers: helpers,
            update: update,
            updating: updating,
            version: version
        }
    }

    // A poll that changed nothing must not look like a change. Every parse builds
    // fresh arrays, and a new one reaches resolvePanel() as a different panel,
    // which rebuilds every row in it - several times a minute, destroying
    // whatever a search field held along with the focus that was in it.
    function _stable(current, next) {
      return JSON.stringify(current) === JSON.stringify(next) ? current : next
    }

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
        _pollProvider[kind] = activeProviderId
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
        if (root._userKinds.indexOf(kind) !== -1) root._userInflight += 1
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
            if (root._userKinds.indexOf(kind) !== -1)
                root._userInflight = Math.max(0, root._userInflight - 1)
        }
    }

    // Armed by the launch and disarmed by the landing, so it only ever fires on
    // a poll that really did hang. Left armed it reaps whichever healthy poll is
    // in flight fifteen seconds later, which at the three-second cadence of an
    // open popup is nearly always one - a refresh silently skipped.
    function _pollSettled(kind) {
        var polls = ["status", "mullvad", "accounts", "networks"]
        for (var i = 0; i < polls.length; i++) {
            if (polls[i] === kind) continue
            if (root._inflight[polls[i]]) return
        }
        pollWatchdog.stop()
    }

    // The watcher is meant to sit there for minutes; reaping it as a hung poll
    // would restart it forever.
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

    function _stale(kind) {
        return _pollProvider[kind] !== undefined
            && _pollProvider[kind] !== activeProviderId
    }

    function _handle(kind, exitCode, stdout, stderr) {
        // A poll answering for the provider we just left would be parsed with
        // the wrong parser and land as an empty panel.
        if (kind !== "which" && kind !== "bgStatus" && _stale(kind)) return
        if (kind === "which") {
            root.providers = Model.parseProviderProbe(stdout)
            root.installed = Model.providerReady({
                providers: root.providers,
                activeProviderId: root.activeProviderId
            })
            if (root.installed) {
                root.refreshStatusAndAccounts()
                if (!root._inflight["watch"]) root.watch()
            }
            else {
                root.refreshing = false
                root.resetUnavailable("Not installed")
            }
        } else if (kind === "status") {
            root.refreshing = false
            root._pollSettled(kind)
            if (exitCode === 0) root.parseStatus(stdout)
            else {
                root.resetUnavailable("Disconnected")
                root.lastError = stderr.trim()
            }
        } else if (kind === "accounts") {
            root._pollSettled(kind)
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
        } else if (kind === "bgStatus") {
        root._setSummary(root._bgProviderId,
          exitCode === 0 ? Model.parseProviderStatus({
            providers: root.providers,
            activeProviderId: root._bgProviderId
          }, stdout) : null)
      } else if (kind === "networks") {
        root._pollSettled(kind)
        root.parseNetworks(exitCode === 0 ? stdout : "")
      } else if (kind === "selectNetwork") {
        root.selectingNetworkId = ""
        if (exitCode !== 0) root.lastError = Model.elideStatus(stderr || stdout || "Network change failed")
        root.refresh(false)
      } else if (kind === "mullvad") {
            root._pollSettled(kind)
            root.parseMullvadExitNodes(exitCode === 0 ? stdout : "")
        } else if (kind === "action") {
            if (exitCode !== 0) {
                root._desired = -1
                root.lastError = Model.elideStatus(stderr || stdout || "Command failed")
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
        } else if (kind === "watch") {
        // 0 means something changed, 2 means the wait simply expired.
        // Anything else is a broken watcher, so back off rather than spin.
            if (exitCode === 0) root.refresh()
            rearmWatch.interval = (exitCode === 0 || exitCode === 2) ? 250 : 30000
            rearmWatch.restart()
        } else if (kind === "helpers") {
            root.helpers = exitCode === 0
        } else if (kind === "update") {
        // --check exits 2 when an update is available, which is a result,
        // not a failure.
            if (exitCode === 0 || exitCode === 2) {
                try {
                    root.update = root._stable(root.update, JSON.parse(stdout))
                } catch (e) {
                    root.update = { available: false, current: "", latest: "" }
                }
            }
        } else if (kind === "applyUpdate") {
            root.updating = false
            if (exitCode !== 0) {
                root.lastError = Model.elideStatus(stderr || stdout || "Update failed")
                root.actionStatus = root.lastError
                actionStatusTimer.restart()
            } else {
                root.actionStatus = "Updated - restart the shell to load it"
                actionStatusTimer.restart()
                root.checkUpdate(true)
            }
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
        _detach(["tailgauge", "copy", text])
    }

    function copyPeerIp(peer) {
        if (!peer) return
        var ips = Model.filterIPv4(peer.IPv4 || [])
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
        _detach(["tailgauge", "send", target])
    }

    // Re-armed from a timer rather than from inside its own handler, which is
    // still mid-disconnect when this runs.
    function watch() {
        if (!installed) return
        var watch = _commands().watch
        if (watch) _run("watch", watch(300))
    }

    function checkUpdate(force) {
        var argv = ["tailgauge", "--check-update"]
        if (force === true) argv.push("--force")
        _run("update", argv)
    }

    function applyUpdate() {
        if (updating || !update || update.available !== true) return
        updating = true
        actionStatus = "Updating TailGauge…"
        _run("applyUpdate", ["tailgauge", "--update"])
    }

    function openUrl(url) {
        var target = String(url || "")
        if (target !== "") Qt.openUrlExternally(target)
    }

    function refresh(forceAccounts) {
        if (installed) {
            refreshStatusAndAccounts(forceAccounts === true)
            return
        }
        if (_run("which", ["which"].concat(Model.providerCliNames()))) refreshing = true
    }

    function refreshStatusAndAccounts(forceAccounts) {
        if (!installed) return
        var launched = false
        if (_run("status", _commands().status)) {
            refreshing = true
            launched = true
        }
        if (_commands().exitNodeList && _run("mullvad", _commands().exitNodeList)) launched = true
      if (_commands().networks && _run("networks", _commands().networks)) launched = true
      _pollNextIdleProvider()

        var now = Date.now()
        var shouldRefreshAccounts = forceAccounts === true || accounts.length === 0 || now - _lastAccountsRefreshMs > 60000
        if (shouldRefreshAccounts) {
            if (_commands().accounts && _run("accounts", _commands().accounts)) {
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
        daemonState = "Unavailable"
        statusText = message
        selfName = ""
        selfDnsName = ""
        selfIp = ""
        selfUserId = ""
        selfPeer = null
        fileSharing = false
        authUrl = ""
        peers = []
        exitNodes = []
        ownExitNodes = []
        mullvadExitNodes = []
        networks = []
        selectingNetworkId = ""
        mullvadRegions = []
        accounts = []
        selectedAccountId = ""
        selectedAccountLabel = ""
        switchingAccountId = ""
        settingExitNodeId = ""
        accountsAccessDenied = false
    }

    function parseStatus(raw) {
        var parsed = Model.parseProviderStatus({
        providers: root.providers,
        activeProviderId: root.activeProviderId
      }, raw)
        _setSummary(activeProviderId, parsed)
      _cachedProviderId = activeProviderId
        if (!parsed.ok) {
            resetUnavailable(parsed.message || "Status error")
            lastError = parsed.error || "Failed to parse tailscale status"
            return
        }
        if (parsed.unavailable) {
            resetUnavailable(parsed.message || "Disconnected")
            return
        }

        daemonState = parsed.daemonState
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
        selfPeer = _stable(selfPeer, parsed.selfPeer)
        fileSharing = parsed.fileSharing
        peers = _stable(peers, parsed.running ? parsed.peers : [])
        ownExitNodes = _stable(ownExitNodes, parsed.running ? parsed.exitNodes : [])
        exitNodes = parsed.running ? ownExitNodes.concat(mullvadRegions) : []

        if (needsLogin) statusText = "Needs login"
        else if (running) {
            statusText = "Connected"
            _loginInProgress = false
            _loginUrlOpened = false
            _preLoginAuthUrl = ""
            loginTimeoutTimer.stop()
        } else if (daemonState === "Stopped") {
            statusText = "Disconnected"
        } else {
            statusText = daemonState
        }
        lastError = ""
    }

    function parseAccounts(raw) {
        var parsed = Model.parseAccounts(raw)
        accounts = _stable(accounts, parsed.accounts)
        selectedAccountId = parsed.selectedAccountId
        selectedAccountLabel = parsed.selectedAccountLabel
        accountsAccessDenied = false
    }

    // Every change to the active provider lands here, not just a click. The
    // desktop delivers this widget's settings after the first polls have gone
    // out, so the persisted choice arrives late and has to re-ask.
    onActiveProviderIdChanged: root._adoptProvider()

    function _adoptProvider() {
      var polls = ["status", "mullvad", "accounts", "networks", "watch"]
      for (var i = 0; i < polls.length; i++) _reap(polls[i])
      // The previous provider's machines and accounts are not this one's,
      // but they are still worth keeping for when it comes back.
      _saveCache(_cachedProviderId)
      _cachedProviderId = activeProviderId
      if (!_restoreCache(activeProviderId)) resetUnavailable("Switching")
      // A toggle pending on the provider we just left is not pending here.
      _desired = -1
      // Seed from what the background poll already knows, so the header does
      // not flash disconnected on the way to a provider that is up.
      for (var j = 0; j < summaries.length; j++) {
        if (String(summaries[j].id) !== activeProviderId) continue
        running = summaries[j].running
        needsLogin = summaries[j].needsLogin
        selfName = summaries[j].selfName
        selfIp = summaries[j].selfIp
      }
      installed = Model.providerReady({
        providers: providers,
        activeProviderId: activeProviderId
      })
      refresh(true)
      // The old provider's watcher was just reaped; this one may not have one.
      watch()
    }

    function switchProvider(provider) {
      if (!provider) return
      var id = String(provider.id || "")
      if (id === "" || id === activeProviderId) return
      activeProviderId = id
      providerChanged(id)
    }

    readonly property var _cachedFields: [
      "running", "needsLogin", "daemonState", "statusText", "authUrl",
      "selfName", "selfDnsName", "selfIp", "selfUserId", "selfPeer", "fileSharing",
      "peers", "exitNodes", "ownExitNodes", "mullvadExitNodes", "mullvadRegions",
      "networks", "accounts", "selectedAccountId", "selectedAccountLabel"
    ]

    function _saveCache(id) {
      if (!id) return
      var snap = {}
      for (var i = 0; i < _cachedFields.length; i++) snap[_cachedFields[i]] = root[_cachedFields[i]]
      _cache[id] = snap
    }

    function _restoreCache(id) {
      var snap = id ? _cache[id] : null
      if (!snap) return false
      for (var i = 0; i < _cachedFields.length; i++) root[_cachedFields[i]] = snap[_cachedFields[i]]
      return true
    }

    function _setSummary(providerId, parsed) {
      var next = []
      var replaced = false
      for (var i = 0; i < summaries.length; i++) {
        if (String(summaries[i].id) === String(providerId)) {
          next.push(Model.summarizeProvider(providerId, parsed))
          replaced = true
        } else next.push(summaries[i])
      }
      if (!replaced) next.push(Model.summarizeProvider(providerId, parsed))
      summaries = _stable(summaries, next)
    }

    // One inactive provider per tick, so the tooltip stays current without
    // running every provider's whole poll set every time.
    function _pollNextIdleProvider() {
      var others = []
      var all = Model.drivableProviders({ providers: providers, activeProviderId: activeProviderId })
      for (var i = 0; i < all.length; i++) {
        if (all[i].id !== activeProviderId) others.push(all[i])
      }
      if (others.length === 0) return
      var provider = others[_bgIndex % others.length]
      // The reply is read against _bgProviderId, so moving it for a poll
      // that never launched would file the running one under the wrong
      // provider.
      if (!_run("bgStatus", provider.commands.status)) return
      _bgIndex = (_bgIndex + 1) % others.length
      _bgProviderId = provider.id
    }

    function parseNetworks(raw) {
      var parsed = Model.parseNetbirdNetworks(raw)
      if (!parsed.ok) {
        lastError = Model.elideStatus(parsed.message)
        return
      }
      networks = _stable(networks, parsed.networks)
    }

    function selectNetwork(network) {
      if (!installed || !running || !network) return
      var id = String(network.id || "")
      if (id === "") return
      if (_run("selectNetwork", _commands().selectNetwork(id, network.selected !== true)))
        selectingNetworkId = id
    }

    function parseMullvadExitNodes(raw) {
        mullvadExitNodes = Model.parseExitNodeList(raw)
        mullvadRegions = _stable(mullvadRegions, Model.mullvadRegionOptions(mullvadExitNodes))
        exitNodes = running ? ownExitNodes.concat(mullvadRegions) : []
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
        _run("action", _commands().down)
    }

    function loginOrUp() {
        if (!installed || _inflight["login"]) return
        _desired = -1
        var plan = Model.loginPlan(needsLogin, authUrl, root._commands().up)
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
        if (_run("switch", _commands().switchAccount(accountId))) switchingAccountId = accountId
    }

    function setExitNode(peer) {
        if (!installed || !running || !peer) return
        var isActive = peer.ExitNode === true
        var target = isActive ? "" : Model.exitNodeTarget(peer)
        if (!isActive && target === "") return
        if (_run("exitNode", _commands().setExitNode(target)))
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
        id: rearmWatch
        interval: 250
        repeat: false
        onTriggered: root.watch()
    }

    Timer {
    // The watcher carries the news; this is the floor under it, and the
    // only thing running when the watcher is unavailable.
        interval: (root.attentive ? 3 : Math.max(5, root.refreshIntervalSec)) * 1000
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
    // The helper caches its GitHub answer, so this mostly reads a file; the
    // interval is about how stale the banner may be, not about rate limits.
        interval: 6 * 3600 * 1000
        repeat: true
        running: true
        triggeredOnStart: true
        onTriggered: root.checkUpdate()
    }

    Component.onCompleted: root._run("helpers", ["which", "tailgauge"])

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
            root._reap("networks")
            root._reap("bgStatus")
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
                root.actionStatus = "Login link not available yet"
            }
        }
    }
}
