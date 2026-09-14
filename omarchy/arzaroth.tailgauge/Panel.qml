import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

Panel {
  id: root
  moduleName: "arzaroth.tailgauge"
  ipcTarget: "arzaroth.tailgauge"
  manageIpc: false

  property bool cursorActive: false
  property int cursorIndex: 0
  property bool copyMenuOpen: false
  property bool mullvadPickerOpen: false
  property string mullvadQuery: ""
  property string machineQuery: ""
  property string expandedPeerId: ""
  property bool machineSearchActive: false
  property int phraseIndex: 0

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  // The bar describes the machine, not the panel's current view: switching
  // which provider is shown must not read as a disconnection.
  readonly property color barIconColor: root.panel.bar.connected ? barForeground : Qt.darker(barForeground, 1.55)
  readonly property color hoverFill: Style.hoverFillFor(foreground, Color.accent)
  readonly property color selectedFill: Style.selectedFillFor(foreground, Color.accent)

  readonly property var recentMullvadRegions: settings.recentMullvadRegions instanceof Array
    ? settings.recentMullvadRegions : []

  // Everything the panel shows is decided in the shared model: which sections
  // exist, their order, their rows, every label, and the cursor's traversal
  // order. This file only decides what a row looks like.
  readonly property var panel: Model.resolvePanel(tailscale.snapshot(), {
    recentRegions: root.recentMullvadRegions,
    mullvadPickerOpen: root.mullvadPickerOpen,
    expandedPeerId: root.expandedPeerId,
    nowMs: Date.now(),
    phraseIndex: root.phraseIndex
  })

  readonly property string cursorRowId: cursorIndex >= 0 && cursorIndex < panel.navigation.length
    ? String(panel.navigation[cursorIndex].rowId) : ""

  // ---- search ---------------------------------------------------------------
  // The model decides what a row is findable by and hands it over as a
  // pre-lowercased key; the only thing left here is the substring test.

  readonly property bool machinesMatched: scopeMatched("machines")
  readonly property bool mullvadMatched: scopeMatched("mullvad")

  function searchQuery(scope) {
    if (scope === "machines") return machineQuery.trim().toLowerCase()
    if (scope === "mullvad") return mullvadQuery.trim().toLowerCase()
    return ""
  }

  // Rows and navigation entries carry the same two fields, so both sides of the
  // panel stay filtered by one predicate rather than by two that drift.
  function searchMatches(item) {
    if (!item || item.searchScope === "") return true
    var query = searchQuery(item.searchScope)
    return query === "" || item.searchKey.indexOf(query) !== -1
  }

  function scopeMatched(scope) {
    if (searchQuery(scope) === "") return true
    var nav = panel.navigation
    for (var i = 0; i < nav.length; i++)
      if (nav[i].searchScope === scope && searchMatches(nav[i])) return true
    return false
  }

  function rowVisible(row) {
    if (!row || row.searchScope === "") return true
    if (row.searchKey !== "") return searchMatches(row)
    return !(row.searchScope === "machines" ? machinesMatched : mullvadMatched)
  }

  // ---- cursor ---------------------------------------------------------------

  function moveCursor(delta) {
    cursorActive = true
    var nav = panel.navigation
    if (nav.length === 0) return
    var step = delta < 0 ? -1 : 1
    var index = cursorIndex
    for (var moves = Math.abs(delta); moves > 0; moves--) {
      var next = index + step
      while (next >= 0 && next < nav.length && !searchMatches(nav[next])) next += step
      if (next < 0 || next >= nav.length) break
      index = next
    }
    cursorIndex = index
    scrollCursorIntoView()
  }

  // A query that filters the selected row out from under the cursor has to
  // leave it somewhere it can still be seen.
  onMachineQueryChanged: keepCursorVisible()
  onMullvadQueryChanged: keepCursorVisible()

  function dropOrphanedQueries() {
    if (!Model.panelHasRow(panel, "machines:search")) machineQuery = ""
    if (!Model.panelHasRow(panel, "mullvad:add")) mullvadQuery = ""
  }

  function keepCursorVisible() {
    var nav = panel.navigation
    if (cursorIndex < 0 || cursorIndex >= nav.length || searchMatches(nav[cursorIndex])) return
    for (var down = cursorIndex + 1; down < nav.length; down++)
      if (searchMatches(nav[down])) { cursorIndex = down; return }
    for (var up = cursorIndex - 1; up >= 0; up--)
      if (searchMatches(nav[up])) { cursorIndex = up; return }
    cursorIndex = 0
  }

  function selectedRow() {
    return Model.panelRowAt(panel, cursorIndex)
  }

  // The cursor follows the row's identity, not its slot: a machine that drops
  // off the tailnet between polls would otherwise slide a different one under
  // whatever was selected.
  property string _pinnedRowId: ""
  onCursorRowIdChanged: _pinnedRowId = cursorRowId
  onPanelChanged: {
    dropOrphanedQueries()
    if (_pinnedRowId === "") return
    var next = Model.panelNavIndexOf(panel, _pinnedRowId)
    if (next !== cursorIndex) cursorIndex = next
  }

  function activateCursor() {
    if (cursorIndex === 0) {
      tailscale.toggleTailscale()
      return
    }
    dispatch(selectedRow())
  }

  // The one place a resolved row turns back into a service call.
  function dispatch(row) {
    if (!row) return
    switch (row.action) {
    case "toggle":
      tailscale.toggleTailscale()
      break
    case "authorize":
      tailscale.authorizeTailscaleOperator()
      break
    case "switchAccount":
      tailscale.switchAccount(row.payload.id)
      break
    case "setExitNode":
      if (row.payload.Mullvad === true) persistRecentMullvad(Model.mullvadRegionKey(row.payload))
      tailscale.setExitNode(row.payload)
      mullvadPickerOpen = false
      break
    case "switchProvider":
      tailscale.switchProvider(row.payload)
      break
    case "selectNetwork":
      tailscale.selectNetwork(row.payload)
      break
    case "togglePicker":
      mullvadPickerOpen = !mullvadPickerOpen
      break
    case "copy":
      openCopyMenuFor(row.id)
      break
    case "update":
      tailscale.applyUpdate()
      break
    case "openUrl":
      tailscale.openUrl(row.payload ? row.payload.url : "")
      break
    }
  }

  function copyOption(row, kind) {
    if (!row) return
    if (kind === "name") tailscale.copyPeerName(row.payload)
    else if (kind === "dns") tailscale.copyPeerDnsName(row.payload)
    else if (kind === "ip") tailscale.copyPeerIp(row.payload)
    else {
      for (var i = 0; i < row.copyOptions.length; i++)
        if (row.copyOptions[i].kind === kind) tailscale.copyToClipboard(row.copyOptions[i].label)
    }
  }

  // The file chooser takes over from here, so get the panel out of the way.
  function sendPeerFile(row) {
    if (!row || !row.payload) return
    tailscale.sendFile(row.payload)
    close()
  }

  function cycleProvider() {
    var next = Model.nextProvider(tailscale.snapshot())
    if (next) tailscale.switchProvider(next)
  }

  function headerAction(actionId) {
    if (actionId === "refresh") tailscale.refresh(true)
  }

  function rowAction(row, actionId) {
    if (actionId === "send") sendPeerFile(row)
    else if (actionId === "detail") toggleDetail(row)
    else openCopyMenuFor(row.id)
  }

  function toggleDetail(row) {
    if (!row || !row.payload) return
    var id = String(row.payload.id || "")
    expandedPeerId = expandedPeerId === id ? "" : id
  }

  // The shell owns the widget's settings, so a recent region is persisted by
  // writing the entry back rather than by keeping a list here.
  function persistSetting(name, value) {
    if (!bar || !bar.shell || typeof bar.shell.updateEntryInline !== "function") return
    var entry = { id: root.moduleName }
    for (var key in settings) if (key !== "id") entry[key] = settings[key]
    entry[name] = value
    bar.shell.updateEntryInline(root.moduleName, entry)
  }

  function persistRecentMullvad(region) {
    persistSetting("recentMullvadRegions", Model.pushRecentMullvad(recentMullvadRegions, region, 5))
  }

  // ---- row registry ---------------------------------------------------------
  // Rows live inside nested Repeaters, so they register themselves here for the
  // two things reached by id rather than by binding: scrolling the cursor into
  // view, and opening a machine's copy menu from the keyboard.

  property var rowItems: ({})

  // `claim` false releases. Delegates are reused by index, so a row can already
  // have been claimed by another delegate before the one that used to hold it
  // gets round to letting go - releasing by id alone would take the newer entry
  // with it, and the copy menu it points at would stop opening.
  function registerRow(id, item, claim) {
    var items = rowItems
    if (claim) items[id] = item
    else if (items[id] === item) delete items[id]
    else return
    rowItems = items
  }

  function openCopyMenuFor(id) {
    var item = rowItems[id]
    if (item && item.openCopyMenu) item.openCopyMenu()
  }

  function focusRow(id) {
    cursorActive = true
    cursorIndex = Model.panelNavIndexOf(panel, id)
  }

  function scrollCursorIntoView() {
    var item = rowItems[cursorRowId]
    if (!panelFlick || !item) return
    Qt.callLater(function () {
      if (!item) return
      var margin = Style.space(6)
      var point = item.mapToItem(panelFlick.contentItem, 0, 0)
      var top = point.y
      var bottom = top + item.height
      var maxY = Math.max(0, panelFlick.contentHeight - panelFlick.height)
      if (top < panelFlick.contentY + margin) panelFlick.contentY = Math.max(0, top - margin)
      else if (bottom > panelFlick.contentY + panelFlick.height - margin)
        panelFlick.contentY = Math.min(maxY, bottom + margin - panelFlick.height)
    })
  }

  // ---- wiring ---------------------------------------------------------------

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onOpenedChanged: {
    if (!opened) return
    cursorActive = false
    cursorIndex = 0
    _pinnedRowId = ""
    machineSearchActive = false
    // A popup torn down with the panel never reports itself closed, and a
    // stale flag here leaves the key catcher blocked for good.
    copyMenuOpen = false
    mullvadPickerOpen = false
    mullvadQuery = ""
    machineQuery = ""
    if (panelFlick) panelFlick.contentY = 0
    tailscale.refresh()
    Qt.callLater(function () { keyCatcher.forceActiveFocus() })
  }

  Service {
    id: tailscale
    settings: root.settings
    activeProviderId: root.settings && root.settings.activeProvider ? root.settings.activeProvider : ""
    onProviderChanged: function (id) { root.persistSetting("activeProvider", id) }
    // An open panel is worth polling for; a closed one rides the watcher.
    attentive: root.opened
  }

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function show(): void { root.open() }
    function hide(): void { root.close() }
    function toggle(): void { root.toggle() }
    function refresh(): string { tailscale.refresh(true); return "ok" }
    function up(): string { tailscale.loginOrUp(); return "ok" }
    function down(): string { tailscale.down(); return "ok" }
    function toggleTailscale(): string { tailscale.toggleTailscale(); return "ok" }
    function status(): string { return tailscale.statusText }
    // What the panel believes right now. A multi-provider panel has state that
    // no log line shows and no test can reach, so it can be asked directly.
    function diagnose(): string {
      return JSON.stringify({
        activeProviderId: tailscale.activeProviderId,
        installed: tailscale.installed,
        providers: tailscale.providers,
        summaries: tailscale.summaries,
        pollProvider: tailscale._pollProvider,
        networks: tailscale.networks,
        tooltip: root.panel.bar.tooltip
      })
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    tooltipText: root.panel.bar.tooltip.join("\n")
    iconComponent: Component {
      Item {
        TailGaugeIcon {
          anchors.centerIn: parent
          iconSize: Style.space(11)
          color: root.barIconColor
          badgeColor: root.urgent
          crossed: root.panel.bar.crossed
          warning: root.panel.bar.warning
        }
      }
    }
    // Right-click used to turn the connection on and off. The icon describes
    // the machine rather than one provider, so that acted on a provider you
    // could not see and left the icon lit when the other was still up. It
    // cycles which provider the panel is about instead, and does nothing when
    // there is only one.
    onPressed: function (buttonCode) {
      if (buttonCode === Qt.RightButton) root.cycleProvider()
      else if (buttonCode === Qt.MiddleButton) tailscale.refresh(true)
      else root.toggle()
    }
  }

  Timer {
    interval: 2800
    running: root.opened && root.panel.header.toggleChecked
    repeat: true
    onTriggered: phraseSwap.restart()
  }

  SequentialAnimation {
    id: phraseSwap
    PropertyAnimation {
      target: hero
      property: "metaOpacity"
      to: 0.0
      duration: 180
      easing.type: Easing.OutQuad
    }
    ScriptAction { script: root.phraseIndex = root.phraseIndex + 1 }
    PropertyAnimation {
      target: hero
      property: "metaOpacity"
      to: 1.0
      duration: 260
      easing.type: Easing.InQuad
    }
  }

  KeyboardPanel {
    id: card
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: card.fittedContentWidth(Style.space(380))
    contentHeight: card.fittedContentHeight(column.implicitHeight, Style.space(560))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      // It takes keys before any focused descendant - that is what lets j and k
      // drive the cursor - so a search field only receives what it does not
      // claim. Blocking whenever the catcher itself is not focused hands every
      // key to whatever is: h, j, k, l, x and space were being swallowed
      // outright, which is why a lowercase query behaved differently from the
      // same query typed in capitals, and c, n, d, s, t and r fired a row
      // action on their way through.
      blocked: root.copyMenuOpen || !keyCatcher.activeFocus
      onMoveRequested: function (dx, dy) {
        if (!root.cursorActive) { root.cursorActive = true; return }
        if (dy !== 0) root.moveCursor(dy > 0 ? 1 : -1)
      }
      onActivateRequested: if (root.cursorActive) root.activateCursor(); else root.cursorActive = true
      onCloseRequested: {
        if (root.mullvadPickerOpen) root.mullvadPickerOpen = false
        else root.close()
      }
      onTabRequested: function (direction) { root.switchPanel(direction) }
      onTextKey: function (key) {
        var letter = String(key).toLowerCase()
        if (letter === "t") {
          tailscale.toggleTailscale()
          return
        }
        if (letter === "r") {
          tailscale.refresh(true)
          return
        }
        var row = root.selectedRow()
        if (!row) return
        if (letter === "s") {
          if (Model.panelRowHasAction(row, "send")) root.sendPeerFile(row)
          return
        }
        if (row.copyOptions.length === 0) return
        if (letter === "c") root.copyOption(row, "ip")
        else if (letter === "n") root.copyOption(row, "name")
        else if (letter === "d") root.copyOption(row, "dns")
      }

      Flickable {
        id: panelFlick
        anchors.fill: parent
        contentWidth: width
        contentHeight: column.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.VerticalFlick
        interactive: contentHeight > height
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
          id: column
          width: panelFlick.width
          spacing: Style.space(12)

          Item {
            id: header
            width: parent.width
            implicitHeight: hero.implicitHeight
            // Exposed for the hero's trailingControl, whose `root` resolves to
            // PanelHero (not this Panel) - reach panel state via `header`.
            readonly property bool ringVisible: root.cursorActive && root.cursorIndex === 0
              && root.panel.header.toggleVisible
            function focusHero() {
              root.cursorActive = true
              root.cursorIndex = 0
            }

            PanelHero {
              id: hero
              width: parent.width
              title: root.panel.header.title
              meta: root.panel.header.meta
              foreground: root.foreground
              fontFamily: root.fontFamily
              iconOpacity: root.panel.header.dimmed ? 0.5 : 1.0
              // Status only - the switch owns toggling, mouse and keyboard alike.
              // The bar icon carries the machine's state; the hero says which
              // provider the panel is showing, drawn as that provider's own
              // mark rather than as a font glyph standing in for one.
              iconComponent: Component {
                Loader {
                  readonly property color tint: root.panel.header.warning
                    ? root.urgent
                    : (root.panel.header.dimmed ? root.dim : root.foreground)
                  sourceComponent: root.panel.header.providerId === "netbird"
                    ? netbirdMark : tailscaleMark

                  Component {
                    id: tailscaleMark
                    TailGaugeIcon {
                      iconSize: Style.font.display
                      color: parent ? parent.tint : root.foreground
                      badgeColor: root.urgent
                    }
                  }

                  Component {
                    id: netbirdMark
                    NetBirdIcon {
                      iconSize: Style.font.display
                      color: parent ? parent.tint : root.foreground
                    }
                  }
                }
              }

              trailingControl: Component {
                Row {
                spacing: Style.space(6)

                Repeater {
                  model: root.panel.header.actions

                  PanelActionButton {
                    required property var modelData
                    anchors.verticalCenter: parent.verticalCenter
                    iconText: modelData.glyph
                    tooltipText: modelData.label
                    foreground: hero.foreground
                    hoverColor: hero.foreground
                    fontFamily: hero.fontFamily
                    onClicked: root.headerAction(modelData.id)
                  }
                }

                ToggleSwitch {
                  id: powerSwitch
                  anchors.verticalCenter: parent.verticalCenter
                  visible: root.panel.header.toggleVisible
                  checked: root.panel.header.toggleChecked
                  // Busy is shown, never enforced: a command in flight dims the
                  // switch but leaves it clickable.
                  busy: root.panel.header.busy
                  hasCursor: header.ringVisible
                  foreground: hero.foreground
                  onHovered: function (on) { if (on) header.focusHero() }
                  onToggled: tailscale.toggleTailscale()

                  PanelToolTip {
                    visible: powerSwitch.containsMouse
                    text: root.panel.header.toggleHint
                    fontFamily: hero.fontFamily
                  }
                }
                }
              }
            }
          }

          Text {
            textFormat: Text.PlainText
            visible: root.panel.status.text !== ""
            width: parent.width
            text: root.panel.status.text
            color: root.panel.status.tone === "error" ? root.urgent : root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            wrapMode: Text.WordWrap
          }

          // Counted, not fed the array: a Repeater handed a new JS array
          // destroys and rebuilds every delegate it holds, and `panel` is a new
          // object on every keystroke because the query is one of its inputs.
          // That took the field being typed into with it. Bound by index, a
          // delegate survives and simply re-reads its row.
          Repeater {
            model: root.panel.sections.length

            Column {
              id: sectionView
              required property int index
              readonly property var modelData: root.panel.sections[sectionView.index]
              readonly property bool isChips: !!modelData && modelData.id === "providers"

              width: column.width
              visible: !!modelData && modelData.visible
              spacing: Style.space(10)

              PanelSeparator {
                visible: sectionView.modelData.title !== ""
                foreground: root.foreground
              }

              PanelSectionHeader {
                visible: sectionView.modelData.title !== ""
                text: sectionView.modelData.title
                font.capitalization: Font.AllUppercase
                foreground: root.foreground
                fontFamily: root.fontFamily
              }

              Text {
                textFormat: Text.PlainText
                visible: sectionView.modelData.rows.length === 0 && sectionView.modelData.empty !== ""
                width: parent.width
                text: sectionView.modelData.empty
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                horizontalAlignment: Text.AlignHCenter
              }

              // Picking a provider is a choice between a handful of names, not a
              // list to walk: it reads as chips, the way TokenGauge picks its
              // AI provider.
              ButtonGroup {
                visible: sectionView.isChips
                width: parent.width
                foreground: root.foreground
                accent: Color.accent
                fontFamily: root.fontFamily
                // The panel cursor still walks these rows, so the chip under it
                // has to light up like any other row would.
                cursorIndex: {
                  if (!sectionView.isChips || !root.cursorActive) return -1
                  var rows = sectionView.modelData.rows
                  for (var i = 0; i < rows.length; i++)
                    if (String(rows[i].id) === root.cursorRowId) return i
                  return -1
                }
                focusable: false
                options: {
                  var out = []
                  if (!sectionView.isChips) return out
                  var rows = sectionView.modelData.rows
                  for (var i = 0; i < rows.length; i++)
                    out.push({ value: String(rows[i].id), label: String(rows[i].label) })
                  return out
                }
                value: {
                  if (!sectionView.isChips) return ""
                  var rows = sectionView.modelData.rows
                  for (var i = 0; i < rows.length; i++)
                    if (rows[i].current) return String(rows[i].id)
                  return ""
                }
                onChanged: function (value) {
                  var rows = sectionView.modelData.rows
                  for (var i = 0; i < rows.length; i++)
                    if (String(rows[i].id) === String(value)) root.dispatch(rows[i])
                }
              }

              Repeater {
                model: sectionView.modelData && !sectionView.isChips
                  ? sectionView.modelData.rows.length : 0

                Column {
                  id: rowGroup
                  required property int index
                  // A shrinking list can evaluate this before the delegate goes,
                  // so every reader below tolerates null.
                  readonly property var modelData: sectionView.modelData
                    ? (sectionView.modelData.rows[rowGroup.index] || null) : null

                  readonly property bool isPicker: !!modelData && modelData.kind === "mullvadPicker"
                  readonly property bool isEmpty: !!modelData && modelData.kind === "empty"
                  readonly property bool isSearch: !!modelData && modelData.kind === "machineSearch"

                  // The row a query excludes goes with its wrapper, or the list
                  // keeps the gap where it stood.
                  visible: root.rowVisible(rowGroup.modelData)
                  width: sectionView.width
                  spacing: Style.space(6)

                  Text {
                    textFormat: Text.PlainText
                    visible: rowGroup.isEmpty
                    width: parent.width
                    text: rowGroup.modelData ? rowGroup.modelData.label : ""
                    color: root.dim
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.bodySmall
                    horizontalAlignment: Text.AlignHCenter
                  }

                  TextField {
                    id: machineSearch
                    visible: rowGroup.isSearch
                    width: parent.width
                    foreground: root.foreground
                    placeholderText: rowGroup.modelData ? rowGroup.modelData.searchPlaceholder : ""
                    text: root.machineQuery
                    onTextChanged: root.machineQuery = text
                    onActiveFocusChanged: if (activeFocus) root.machineSearchActive = true
                    // A tailnet change rebuilds every row, this field included.
                    // Whoever was typing into it has to get it back, or the
                    // next keystroke reaches the panel as a shortcut.
                    Component.onCompleted: if (visible && root.machineSearchActive)
                      Qt.callLater(function () { machineSearch.forceActiveFocus() })
                    Keys.onPressed: function (event) {
                      if (event.key === Qt.Key_Escape) {
                        if (text !== "") text = ""
                        else {
                          root.machineSearchActive = false
                          keyCatcher.forceActiveFocus()
                        }
                        event.accepted = true
                      } else if (event.key === Qt.Key_Down || event.key === Qt.Key_Up) {
                        root.moveCursor(event.key === Qt.Key_Down ? 1 : -1)
                        event.accepted = true
                      } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        root.activateCursor()
                        event.accepted = true
                      }
                    }
                  }

                  RowView {
                    visible: !rowGroup.isEmpty && !rowGroup.isSearch
                    width: parent.width
                    row: rowGroup.modelData
                  }

                  TextField {
                    id: mullvadSearch
                    visible: rowGroup.isPicker && rowGroup.modelData.expanded
                    width: parent.width - Style.space(16)
                    x: Style.space(16)
                    foreground: root.foreground
                    placeholderText: rowGroup.modelData ? rowGroup.modelData.searchPlaceholder : ""
                    text: root.mullvadQuery
                    onTextChanged: root.mullvadQuery = text
                    onVisibleChanged: if (visible) Qt.callLater(function () { mullvadSearch.forceActiveFocus() })
                    // The picker being on screen at all means it was just
                    // opened, so a rebuilt field belongs back under the cursor.
                    Component.onCompleted: if (visible)
                      Qt.callLater(function () { mullvadSearch.forceActiveFocus() })
                    Keys.onPressed: function (event) {
                      if (event.key === Qt.Key_Escape) {
                        root.mullvadPickerOpen = false
                        keyCatcher.forceActiveFocus()
                        event.accepted = true
                      } else if (event.key === Qt.Key_Down || event.key === Qt.Key_Up) {
                        root.moveCursor(event.key === Qt.Key_Down ? 1 : -1)
                        event.accepted = true
                      } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        root.activateCursor()
                        event.accepted = true
                      }
                    }
                  }

                  // One level of nesting is all the model ever emits, so the
                  // children render through the same view without the component
                  // having to recurse.
                  Repeater {
                    model: rowGroup.modelData && rowGroup.modelData.expanded
                      ? rowGroup.modelData.children.length : 0

                    RowView {
                      id: childView
                      required property int index
                      visible: root.rowVisible(childView.row)
                      width: rowGroup.width - Style.space(16)
                      x: Style.space(16)
                      row: rowGroup.modelData
                        ? (rowGroup.modelData.children[childView.index] || null) : null
                    }
                  }
                }
              }
            }
          }

          Text {
            textFormat: Text.PlainText
            visible: root.panel.footer !== ""
            width: parent.width
            text: root.panel.footer
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            horizontalAlignment: Text.AlignRight
          }
        }
      }
    }
  }

  // ---- rows -----------------------------------------------------------------

  // One resolved row, whatever its kind. Everything it shows comes off `row`;
  // this component only picks the widgets.
  component RowView: CursorSurface {
    id: surface

    property var row: null

    // A detail line is subordinate to the machine above it, so it is set a
    // step smaller and packed tighter.
    readonly property bool isDetail: !!row && row.kind === "detail"

    // A delegate outlives the row it happens to be showing now, so the registry
    // has to follow the id rather than the component. It is what scrolls the
    // cursor into view and opens a machine's copy menu from the keyboard.
    property string registeredId: ""

    function syncRegistration() {
      var id = row ? String(row.id) : ""
      if (id === registeredId) return
      // A persistent delegate can be handed a different machine while its copy
      // menu is open, which would offer the new one's addresses under the name
      // that was clicked.
      if (copyPopup.opened) copyPopup.close()
      if (registeredId !== "") root.registerRow(registeredId, surface, false)
      registeredId = id
      if (id !== "") root.registerRow(id, surface, true)
    }

    onRowChanged: syncRegistration()
    Component.onCompleted: syncRegistration()
    Component.onDestruction: if (registeredId !== "") root.registerRow(registeredId, surface, false)

    function openCopyMenu() {
      if (!row || row.copyOptions.length === 0) return
      copyIndex = Math.max(0, Math.min(copyIndex, row.copyOptions.length - 1))
      copyPopup.open()
    }

    property int copyIndex: 0

    function moveCopyCursor(delta) {
      if (!row || row.copyOptions.length === 0) return
      copyIndex = Math.max(0, Math.min(row.copyOptions.length - 1, copyIndex + delta))
    }

    function copyCurrentOption() {
      if (!row || row.copyOptions.length === 0) return
      root.copyOption(row, String(row.copyOptions[copyIndex].kind || ""))
      copyPopup.close()
    }

    hasCursor: root.cursorActive && row && root.cursorRowId === row.id
    current: row ? row.current : false
    foreground: root.foreground
    fill: root.hoverFill
    currentFill: root.selectedFill
    implicitHeight: surface.isDetail
      ? rowContent.implicitHeight + Style.space(4)
      : Math.max(rowContent.implicitHeight, Style.spacing.controlHeight) + Style.spacing.rowPaddingX

    MouseArea {
      id: rowMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onContainsMouseChanged: if (containsMouse && surface.row) root.focusRow(surface.row.id)
      onClicked: root.dispatch(surface.row)
    }

    PanelToolTip {
      visible: rowMouse.containsMouse && surface.row && surface.row.hint !== ""
      text: surface.row ? surface.row.hint : ""
      fontFamily: root.fontFamily
    }

    RowLayout {
      id: rowContent
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(8)
      spacing: Style.space(8)

      Text {
        textFormat: Text.PlainText
        text: surface.row ? surface.row.glyph : ""
        color: surface.current ? root.foreground : root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.icon
        Layout.alignment: Qt.AlignVCenter

        NumberAnimation on rotation {
          running: surface.row ? surface.row.busy : false
          from: 0
          to: 360
          duration: 900
          loops: Animation.Infinite
        }
        onRotationChanged: if (surface.row && !surface.row.busy && rotation !== 0) rotation = 0
      }

      ColumnLayout {
        Layout.fillWidth: true
        spacing: Style.space(1)

        Text {
          textFormat: Text.PlainText
          Layout.fillWidth: true
          text: surface.row ? surface.row.label : ""
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: surface.isDetail ? Style.font.bodySmall : Style.font.body
          font.bold: surface.row ? surface.row.bold : false
          elide: Text.ElideRight
        }

        Text {
          textFormat: Text.PlainText
          visible: text !== ""
          Layout.fillWidth: true
          text: surface.row ? surface.row.sublabel : ""
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }

      Repeater {
        model: surface.row ? surface.row.actions : []

        PanelActionButton {
          required property var modelData
          iconText: modelData.glyph
          tooltipText: modelData.label
          foreground: root.foreground
          fontFamily: root.fontFamily
          Layout.alignment: Qt.AlignVCenter
          onClicked: root.rowAction(surface.row, String(modelData.id || ""))
        }
      }
    }

    Popup {
      id: copyPopup
      x: surface.width - width
      y: surface.height + Style.space(4)
      width: Style.space(280)
      padding: 0
      modal: false
      focus: true
      closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

      function handleKey(event) {
        if (event.key === Qt.Key_Escape) {
          close()
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Down || event.text === "j") {
          surface.moveCopyCursor(1)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Up || event.text === "k") {
          surface.moveCopyCursor(-1)
          event.accepted = true
          return
        }
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
          surface.copyCurrentOption()
          event.accepted = true
        }
      }

      onOpenedChanged: {
        root.copyMenuOpen = opened
        if (opened) Qt.callLater(function () { copyPopupContent.forceActiveFocus() })
        else if (root.opened) Qt.callLater(function () { keyCatcher.forceActiveFocus() })
      }

      background: BorderSurface {
        color: Color.popups.background
        borderSpec: Border.flat(root.dim, 1)
        radius: Style.cornerRadius
      }

      contentItem: Column {
        id: copyPopupContent
        width: parent.width
        focus: true
        Keys.priority: Keys.BeforeItem
        Keys.onPressed: function (event) { copyPopup.handleKey(event) }

        Repeater {
          model: surface.row ? surface.row.copyOptions : []

          CopyChoice {
            required property var modelData
            required property int index
            width: parent.width
            label: String(modelData.label || "")
            selected: surface.copyIndex === index
            onHovered: surface.copyIndex = index
            onChosen: {
              root.copyOption(surface.row, String(modelData.kind || ""))
              copyPopup.close()
            }
          }
        }
      }
    }
  }

  component CopyChoice: CursorSurface {
    id: copyChoice

    signal chosen()
    signal hovered()

    property string label: ""
    property bool selected: false

    foreground: root.foreground
    hasCursor: selected
    implicitHeight: Style.space(48)
    radius: 0

    MouseArea {
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onEntered: copyChoice.hovered()
      onClicked: copyChoice.chosen()
    }

    RowLayout {
      anchors.fill: parent
      anchors.leftMargin: Style.space(12)
      anchors.rightMargin: Style.space(12)
      spacing: Style.space(10)

      Text {
        textFormat: Text.PlainText
        Layout.fillWidth: true
        text: copyChoice.label
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        elide: Text.ElideRight
      }

      Text {
        text: "󰆏"
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.icon
        Layout.alignment: Qt.AlignVCenter
      }
    }
  }
}
