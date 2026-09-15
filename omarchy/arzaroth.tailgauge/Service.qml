import QtQuick
import Quickshell
import Quickshell.Io

// Everything this widget knows, read by one call to the tailgauge binary.
//
// There is no model here and no parsing: `tailgauge panel --json` probes for
// the CLIs, polls every installed provider at once, and returns the whole
// resolved panel - the bar, the header, every section, and the cursor's
// traversal order. This file spawns it, pushes the answer into `panel`, and
// turns a row's action back into a command.
Item {
  id: root

  property var settings: ({})
  signal providerChanged(string id)

  // ---- what the binary answered ---------------------------------------------

  // Push-assigned when a snapshot lands, rather than computed: the panel is on
  // the far side of a process boundary now, so it arrives rather than being
  // derived. Everything bound downstream of it is unchanged.
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
  readonly property string statusText: panel.status.text

  // ---- what only this side knows --------------------------------------------

  property var ui: ({})
  property string activeProviderId: ""
  property string actionStatus: ""
  property string lastError: ""
  property bool updating: false

  // This widget's own version, read from the manifest sitting next to it: the
  // binary reports its own, and the footer says so when the two disagree.
  property string version: ""
  property string switchingAccountId: ""
  property string settingExitNodeId: ""
  property string selectingNetworkId: ""

  // The optimistic toggle: -1 is "whatever the daemon says", 0 and 1 are a
  // click that has not been reconciled yet. Only the layer it lives in moved.
  property int _desired: -1

  readonly property bool busy: actionProc.running || switchProc.running || exitNodeProc.running
    || selectNetworkProc.running || operatorProc.running

  // An open panel is worth polling for; a closed one rides the watcher.
  property bool attentive: false

  readonly property int refreshIntervalSec: intSetting("refreshIntervalSec", 30, 5, 3600)

  function setting(name, fallback) {
    var value = settings ? settings[name] : undefined
    return value === undefined || value === null ? fallback : value
  }

  function intSetting(name, fallback, min, max) {
    var value = parseInt(setting(name, fallback), 10)
    if (isNaN(value)) return fallback
    return Math.max(min, Math.min(max, value))
  }

  // ---- asking ---------------------------------------------------------------

  // What the binary cannot read off the machine: which provider is being shown,
  // what this side is optimistically showing, and what it has in flight.
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
    if (panelProc.running) return
    panelProc.command = ["tailgauge", "panel", "--json", "--ui", JSON.stringify(_ui())]
    panelProc.running = true
  }

  // A click that changes what the panel shows - opening the picker, expanding a
  // machine - has to redraw now rather than on the next tick.
  onUiChanged: refresh()

  Process {
    id: panelProc
    stdout: StdioCollector {
      onStreamFinished: {
        var next = null
        try {
          next = JSON.parse(text)
        } catch (e) {
          root.lastError = "The panel could not be read: " + e
          return
        }
        if (!next || !next.header) return
        root.panel = next
        // The daemon caught up with the click, so stop overriding it.
        if (root._desired !== -1 && next.header.toggleChecked === (root._desired === 1))
          root._desired = -1
      }
    }
    stderr: StdioCollector {
      onStreamFinished: if (text.trim() !== "") root.lastError = text.trim()
    }
  }

  // ---- acting ---------------------------------------------------------------

  // Every command is the binary's. Nothing here names a provider's CLI, so one
  // provider's argv can never be fired at another's daemon.
  function _ctl(proc, action, extra) {
    if (proc.running) return false
    var argv = ["tailgauge", "ctl"]
    if (root.activeProviderId !== "") argv = argv.concat(["--provider", root.activeProviderId])
    argv.push(action)
    if (extra !== undefined) argv = argv.concat(extra)
    proc.command = argv
    proc.running = true
    return true
  }

  function _detach(argv) {
    detachProc.command = argv
    detachProc.running = true
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
    // Shallow: `sections` stays the same array, so the counted repeaters keep
    // the delegates they hold rather than rebuilding every row.
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
    // No progress status here: the greyed icon and hero line already convey the
    // optimistic off, so only a failure is worth a message.
    _desired = 0
    _showOptimistically(false)
    refresh()
    _ctl(actionProc, "down")
  }

  // The binary scrapes the login URL off the daemon's own output and opens it,
  // so the state machine that used to live here - waiting on the stream,
  // guarding against opening twice, timing the wait out - is gone with it.
  function loginOrUp() {
    if (!installed) return
    _desired = 1
    _showOptimistically(true)
    refresh()
    if (_ctl(actionProc, "up")) actionStatus = "Turning it on…"
  }

  function switchAccount(id) {
    var accountId = String(id || "")
    if (!installed || accountId === "") return
    if (_ctl(switchProc, "switch-account", [accountId])) switchingAccountId = accountId
  }

  // The row's payload goes back as it came: which address a peer is reached at
  // is the model's rule, and a Mullvad node is set by address where a tailnet
  // one is set by name.
  function setExitNode(peer) {
    if (!installed || !peer) return
    if (_ctl(exitNodeProc, "exit-node", ["--peer", JSON.stringify(peer)]))
      settingExitNodeId = String(peer.id || "")
  }

  function selectNetwork(network) {
    if (!installed || !network) return
    var id = String(network.id || "")
    if (id === "") return
    var argv = [id]
    if (network.selected === true) argv.push("--leave")
    if (_ctl(selectNetworkProc, "select-network", argv)) selectingNetworkId = id
  }

  function authorizeTailscaleOperator() {
    if (!installed) return
    if (_ctl(operatorProc, "authorize")) actionStatus = "Authorizing the operator…"
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
    if (updateProc.running) return
    var argv = ["tailgauge", "--check-update"]
    if (force === true) argv.push("--force")
    updateProc.command = argv
    updateProc.running = true
  }

  function applyUpdate() {
    if (updating) return
    updating = true
    actionStatus = "Updating TailGauge…"
    refresh()
    applyUpdateProc.running = true
  }

  // ---- the processes --------------------------------------------------------

  // One shape for every command: it either worked, or it said why. A failure
  // that names nothing still clears the progress line, or the panel reports
  // something in flight that finished minutes ago.
  component Action: Process {
    property string clears: ""
    stdout: StdioCollector {}
    stderr: StdioCollector { id: errors }
    onExited: function (exitCode) {
      if (clears === "account") root.switchingAccountId = ""
      else if (clears === "exitNode") root.settingExitNodeId = ""
      else if (clears === "network") root.selectingNetworkId = ""
      root.actionStatus = ""
      root.lastError = exitCode === 0 ? "" : (errors.text.trim() || "The command failed")
      delayedRefresh.restart()
    }
  }

  Action { id: actionProc }
  Action { id: switchProc; clears: "account" }
  Action { id: exitNodeProc; clears: "exitNode" }
  Action { id: selectNetworkProc; clears: "network" }
  Action { id: operatorProc }

  Process { id: detachProc }

  FileView {
    path: Qt.resolvedUrl("manifest.json").toString().replace(/^file:\/\//, "")
    watchChanges: false
    printErrors: false
    onLoaded: {
      try {
        root.version = String((JSON.parse(text()) || {}).version || "")
      } catch (e) {
        root.version = ""
      }
      root.refresh()
    }
    onLoadFailed: root.version = ""
  }

  Process {
    id: updateProc
    stdout: StdioCollector { onStreamFinished: root.refresh() }
  }

  Process {
    id: applyUpdateProc
    command: ["tailgauge", "--update"]
    stdout: StdioCollector {}
    stderr: StdioCollector { id: updateErrors }
    onExited: function (exitCode) {
      root.updating = false
      root.actionStatus = ""
      if (exitCode !== 0) root.lastError = updateErrors.text.trim() || "The update failed"
      root.checkUpdate(true)
    }
  }

  // The watcher carries the news: it blocks until the daemon reports a change,
  // so a change made anywhere shows up at once rather than on the next tick.
  Process {
    id: watchProc
    command: ["tailgauge", "watch", "300"]
    onExited: function (exitCode) {
      if (exitCode === 0) root.refresh()
      // 0 is a change and 2 is an expired wait; anything else is a broken
      // watcher, so back off rather than spin.
      rearmWatch.interval = (exitCode === 0 || exitCode === 2) ? 250 : 30000
      rearmWatch.restart()
    }
  }

  Timer {
    id: rearmWatch
    interval: 250
    onTriggered: if (!watchProc.running) watchProc.running = true
  }

  // The floor under the watcher, and the only thing running when there is no
  // watcher to be had.
  Timer {
    interval: (root.attentive ? 3 : Math.max(5, root.refreshIntervalSec)) * 1000
    repeat: true
    running: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  // A command changes what the daemon will say next, so ask again once it has
  // had a moment to settle rather than racing it.
  Timer {
    id: delayedRefresh
    interval: 600
    onTriggered: root.refresh()
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

  Component.onCompleted: {
    activeProviderId = String(setting("activeProvider", ""))
    watchProc.running = true
  }
}
