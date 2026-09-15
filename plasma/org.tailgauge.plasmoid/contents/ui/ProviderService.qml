import QtQuick
import org.kde.plasma.plasma5support as Plasma5Support

// Everything this widget knows, read by one call to the tailgauge binary.
//
// There is no model here and no parsing: `tailgauge panel --json` probes for
// the CLIs, polls every installed provider at once, and returns the whole
// resolved panel. This file spawns it, pushes the answer into `panel`, and
// turns a row's action back into a command.
//
// Plasma's executable engine takes a command line rather than an argv and has
// no stdin, so everything here goes out shell-quoted through `sh -c`. That is
// the one thing this frontend does that the others do not.
//
// The few strings it writes are bare rather than wrapped in `i18n`, the way
// the other two frontends write theirs: they say what a command is doing or
// why one failed, there is no catalogue behind them, and `i18n` is a global
// the surrounding shell injects - which this file otherwise needs nothing of.
Item {
    id: root

    // ---- what the binary answered ------------------------------------------

    // Push-assigned when a snapshot lands, rather than computed: the panel is
    // on the far side of a process boundary now, so it arrives.
    property var panel: _empty
    readonly property var _empty: ({
        bar: { connected: false, warning: false, crossed: true, tooltip: [] },
        header: { id: "header", title: "TailGauge", providerId: "", icon: "", glyph: "",
                  meta: "", action: "toggle", toggleVisible: false, toggleEnabled: false,
                  toggleChecked: false, busy: false, toggleHint: "", crossed: true,
                  warning: false, dimmed: true, actions: [] },
        status: { text: "Checking…", tone: "dim" },
        sections: [],
        footer: "",
        navigation: []
    })

    readonly property bool installed: panel.header.toggleVisible
    readonly property bool active: panel.header.toggleChecked
    readonly property string selfName: panel.header.title

    // ---- what only this side knows -----------------------------------------

    property var ui: ({})
    property string activeProviderId: ""
    property string actionStatus: ""
    property string lastError: ""
    property bool updating: false
    property string version: ""
    property string switchingAccountId: ""
    property string settingExitNodeId: ""
    property string selectingNetworkId: ""

    // The optimistic toggle: -1 is "whatever the daemon says", 0 and 1 are a
    // click that has not been reconciled yet.
    property int _desired: -1

    readonly property bool busy: _userInflight > 0

    // An open panel is worth polling for; a closed one rides the watcher.
    property bool attentive: false
    property int refreshIntervalSec: 30

    signal providerChanged(string id)

    // ---- the engine --------------------------------------------------------

    // Every value interpolated into a command line has to survive the shell
    // verbatim, and the engine gives us no argv to avoid it.
    function _quote(value) {
        return "'" + String(value === null || value === undefined ? "" : value)
            .replace(/'/g, "'\\''") + "'"
    }

    function _command(argv) {
        var parts = []
        for (var i = 0; i < argv.length; i++) parts.push(_quote(argv[i]))
        // The session PATH often lacks the directory the installer drops the
        // binary into.
        return "sh -c " + _quote(
            'export PATH="$HOME/.local/bin:$HOME/bin:/usr/local/bin:$PATH"; ' + parts.join(" "))
    }

    property var _kinds: ({})
    property var _inflight: ({})
    property int _seq: 0
    property int _userInflight: 0
    readonly property var _userKinds: ["action", "switch", "exitNode", "network", "operator", "applyUpdate"]

    function _run(kind, argv) {
        if (root._inflight[kind]) return false
        root._seq += 1
        // The engine keys a source by its command line, so two runs of the
        // same command would be one source; the counter keeps them apart.
        var source = _command(argv) + " # tailgauge-" + root._seq
        var kinds = root._kinds
        kinds[source] = kind
        root._kinds = kinds
        var inflight = root._inflight
        inflight[kind] = source
        root._inflight = inflight
        if (root._userKinds.indexOf(kind) !== -1) root._userInflight += 1
        exec.connectSource(source)
        return true
    }

    function _detach(argv) {
        root._seq += 1
        var source = _command(argv) + " >/dev/null 2>&1 # tailgauge-detach-" + root._seq
        var kinds = root._kinds
        kinds[source] = "detached"
        root._kinds = kinds
        exec.connectSource(source)
    }

    function _release(source) {
        var kinds = root._kinds
        var kind = kinds[source] || ""
        if (kind === "") return ""
        delete kinds[source]
        root._kinds = kinds
        var inflight = root._inflight
        if (inflight[kind] === source) {
            delete inflight[kind]
            root._inflight = inflight
            if (root._userKinds.indexOf(kind) !== -1)
                root._userInflight = Math.max(0, root._userInflight - 1)
        }
        return kind
    }

    Plasma5Support.DataSource {
        id: exec
        engine: "executable"
        connectedSources: []
        onNewData: (source, data) => {
            exec.disconnectSource(source)
            var kind = root._release(source)
            if (kind === "" || kind === "detached") return
            root._handle(kind, Number(data["exit code"]),
                         String(data.stdout || ""), String(data.stderr || ""))
        }
    }

    function _handle(kind, exitCode, stdout, stderr) {
        if (kind === "panel") {
            if (exitCode !== 0) {
                root.lastError = stderr.trim() || "The panel could not be read"
                return
            }
            var next = null
            try {
                next = JSON.parse(stdout)
            } catch (e) {
                root.lastError = "The panel could not be read"
                return
            }
            if (!next || !next.header) return
            root.panel = next
            // The daemon caught up with the click, so stop overriding it.
            if (root._desired !== -1 && next.header.toggleChecked === (root._desired === 1))
                root._desired = -1
            return
        }

        if (kind === "watch") {
            if (exitCode === 0) root.refresh()
            // 0 is a change and 2 is an expired wait; anything else is a
            // broken watcher, so back off rather than spin.
            rearmWatch.interval = (exitCode === 0 || exitCode === 2) ? 250 : 30000
            rearmWatch.restart()
            return
        }

        if (kind === "update") {
            root.refresh()
            return
        }

        if (kind === "applyUpdate") {
            root.updating = false
            root.actionStatus = ""
            if (exitCode !== 0) root.lastError = stderr.trim() || "The update failed"
            root.checkUpdate(true)
            return
        }

        // Every other command is an action: it either worked, or it said why.
        if (kind === "switch") root.switchingAccountId = ""
        else if (kind === "exitNode") root.settingExitNodeId = ""
        else if (kind === "network") root.selectingNetworkId = ""
        root.actionStatus = ""
        root.lastError = exitCode === 0 ? "" : (stderr.trim() || "The command failed")
        delayedRefresh.restart()
    }

    // ---- asking ------------------------------------------------------------

    // What the binary cannot read off the machine: which provider is being
    // shown, what this side is optimistically showing, and what it has in
    // flight.
    function _ui() {
        var out = {
            activeProviderId: root.activeProviderId,
            busy: root.busy,
            updating: root.updating,
            version: root.version,
            actionStatus: root.actionStatus,
            lastError: root.lastError,
            switchingAccountId: root.switchingAccountId,
            settingExitNodeId: root.settingExitNodeId,
            selectingNetworkId: root.selectingNetworkId
        }
        if (root._desired !== -1) out.active = root._desired === 1
        for (var key in root.ui) out[key] = root.ui[key]
        return out
    }

    function refresh() {
        _run("panel", ["tailgauge", "panel", "--json", "--ui", JSON.stringify(_ui())])
    }

    // A click that changes what the panel shows - opening the picker,
    // expanding a machine - has to redraw now rather than on the next tick.
    onUiChanged: refresh()

    // ---- acting ------------------------------------------------------------

    // Every command is the binary's. Nothing here names a provider's CLI, so
    // one provider's argv can never be fired at another's daemon.
    function _ctl(kind, action, extra) {
        var argv = ["tailgauge", "ctl"]
        if (root.activeProviderId !== "") argv = argv.concat(["--provider", root.activeProviderId])
        argv.push(action)
        if (extra !== undefined) argv = argv.concat(extra)
        return _run(kind, argv)
    }

    // The click shows on the frame it was clicked rather than a round trip
    // later: the local copy of the panel is patched, and the next answer
    // replaces it wholesale. Only the header moves - the bar icon describes the
    // machine's connections, which a click on one provider has not changed yet.
    function _showOptimistically(on) {
        if (!panel || !panel.header) return
        var header = {}
        for (var key in panel.header) header[key] = panel.header[key]
        header.toggleChecked = on
        header.dimmed = !on
        header.crossed = !on && !header.warning
        var next = {}
        // Shallow: `sections` stays the same array, so the counted repeaters
        // keep the delegates they hold rather than rebuilding every row.
        for (var field in panel) next[field] = panel[field]
        next.header = header
        panel = next
    }

    function toggleTailscale() {
        if (!installed) return
        if (active) down()
        else loginOrUp()
    }

    function down() {
        // No progress status here: the greyed icon and hero line already
        // convey the optimistic off, so only a failure is worth a message.
        _desired = 0
        _showOptimistically(false)
        refresh()
        _ctl("action", "down")
    }

    // The binary scrapes the login URL off the daemon's own output and opens
    // it, so the state machine that used to live here is gone with it.
    function loginOrUp() {
        if (!installed) return
        _desired = 1
        _showOptimistically(true)
        refresh()
        if (_ctl("action", "up")) actionStatus = "Turning it on…"
    }

    function switchAccount(id) {
        var accountId = String(id || "")
        if (!installed || accountId === "") return
        if (_ctl("switch", "switch-account", [accountId])) switchingAccountId = accountId
    }

    // The row's payload goes back as it came: which address a peer is reached
    // at is the model's rule, and a Mullvad node is set by address where a
    // tailnet one is set by name.
    function setExitNode(peer) {
        if (!installed || !peer) return
        if (_ctl("exitNode", "exit-node", ["--peer", JSON.stringify(peer)]))
            settingExitNodeId = String(peer.id || "")
    }

    function selectNetwork(network) {
        if (!installed || !network) return
        var id = String(network.id || "")
        if (id === "") return
        var argv = [id]
        if (network.selected === true) argv.push("--leave")
        if (_ctl("network", "select-network", argv)) selectingNetworkId = id
    }

    function authorizeTailscaleOperator() {
        if (!installed) return
        if (_ctl("operator", "authorize")) actionStatus = "Authorizing the operator…"
    }

    function switchProvider(provider) {
        if (!provider) return
        var id = String(provider.id || "")
        if (id === "" || id === activeProviderId) return
        activeProviderId = id
        // The panel belongs to the old provider until the next answer arrives.
        panel = _empty
        providerChanged(id)
        refresh()
    }

    function openUrl(url) {
        var target = String(url || "")
        if (target !== "") _detach(["xdg-open", target])
    }

    function copyToClipboard(value) {
        var text = String(value || "")
        if (text !== "") _detach(["tailgauge", "copy", text])
    }

    function copyPeerName(peer) { if (peer) copyToClipboard(peer.DisplayName || peer.HostName) }
    function copyPeerDnsName(peer) { if (peer) copyToClipboard(peer.DNSName) }
    function copyPeerIp(peer) {
        if (peer && peer.IPv4 && peer.IPv4.length > 0) copyToClipboard(peer.IPv4[0])
    }

    function sendFile(peer) {
        if (peer) _detach(["tailgauge", "send", "--peer", JSON.stringify(peer)])
    }

    function checkUpdate(force) {
        var argv = ["tailgauge", "--check-update"]
        if (force === true) argv.push("--force")
        _run("update", argv)
    }

    function applyUpdate() {
        if (updating) return
        updating = true
        actionStatus = "Updating TailGauge…"
        refresh()
        _run("applyUpdate", ["tailgauge", "--update"])
    }

    // ---- the clocks --------------------------------------------------------

    // The floor under the watcher, and the only thing running when there is no
    // watcher to be had.
    Timer {
        interval: (root.attentive ? 3 : Math.max(5, root.refreshIntervalSec)) * 1000
        repeat: true
        running: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }

    // A command changes what the daemon will say next, so ask again once it
    // has had a moment to settle rather than racing it.
    Timer {
        id: delayedRefresh
        interval: 600
        onTriggered: root.refresh()
    }

    // The watcher carries the news: it blocks until the daemon reports a
    // change, so a change made anywhere shows up at once.
    Timer {
        id: rearmWatch
        interval: 250
        onTriggered: root._run("watch", ["tailgauge", "watch", "300"])
    }

    // The check is cached for six hours in the binary, so this mostly reads a
    // file; the interval is about how stale the banner may be, not rate limits.
    Timer {
        interval: 6 * 3600 * 1000
        repeat: true
        running: true
        triggeredOnStart: true
        onTriggered: root.checkUpdate()
    }

    Component.onCompleted: rearmWatch.start()
}
