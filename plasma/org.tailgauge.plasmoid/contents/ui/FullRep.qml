import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents3
import org.kde.plasma.extras as PlasmaExtras
import org.kde.kirigami as Kirigami
import "../code/model.js" as Model

Item {
    id: full

    readonly property var service: root.tailscale

    Layout.minimumWidth: Kirigami.Units.gridUnit * 20
    Layout.minimumHeight: Kirigami.Units.gridUnit * 18
    Layout.preferredWidth: Kirigami.Units.gridUnit * 24
    Layout.preferredHeight: Kirigami.Units.gridUnit * 30

    property bool cursorActive: false
    property int cursorIndex: 0
    property bool mullvadPickerOpen: false
    property string mullvadQuery: ""
    property int phraseIndex: 0

    // Everything the panel shows is decided in the shared model: which sections
    // exist, their order, their rows, every label, and the cursor's traversal
    // order. This file only decides what a row looks like.
    readonly property var panel: Model.resolvePanel(service.snapshot(), {
        t: (text) => i18n(text),
        recentRegions: root.recentMullvadRegions,
        mullvadQuery: full.mullvadQuery,
        mullvadPickerOpen: full.mullvadPickerOpen,
        phraseIndex: full.phraseIndex
    })

    readonly property color dimColor: Qt.darker(Kirigami.Theme.textColor, 1.55)

    // ---- cursor -------------------------------------------------------------

    function moveCursor(delta) {
        cursorActive = true
        var count = panel.navigation.length
        if (count === 0) return
        cursorIndex = Math.max(0, Math.min(count - 1, cursorIndex + delta))
        scrollCursorIntoView()
    }

    function cursorRowId() {
        if (cursorIndex < 0 || cursorIndex >= panel.navigation.length) return ""
        return panel.navigation[cursorIndex].rowId
    }

    function selectedRow() {
        return Model.panelRowAt(panel, cursorIndex)
    }

    // The cursor follows the row's identity, not its slot: a machine that drops
    // off the tailnet between polls would otherwise slide a different one under
    // whatever was selected.
    property string _pinnedRowId: ""
    onCursorIndexChanged: _pinnedRowId = cursorRowId()
    onPanelChanged: {
        if (_pinnedRowId === "") return
        var next = Model.panelNavIndexOf(panel, _pinnedRowId)
        if (next !== cursorIndex) cursorIndex = next
    }

    function activateCursor() {
        if (cursorIndex === 0) {
            service.toggleTailscale()
            return
        }
        dispatch(selectedRow())
    }

    // The one place a resolved row turns back into a service call.
    function dispatch(row) {
        if (!row) return
        switch (row.action) {
        case "toggle":
            service.toggleTailscale()
            break
        case "authorize":
            service.authorizeTailscaleOperator()
            break
        case "switchAccount":
            service.switchAccount(row.payload.id)
            break
        case "setExitNode":
            if (row.payload.Mullvad === true)
                root.persistRecentMullvad(Model.mullvadRegionKey(row.payload))
            service.setExitNode(row.payload)
            mullvadPickerOpen = false
            break
        case "togglePicker":
            mullvadPickerOpen = !mullvadPickerOpen
            break
        case "copy":
            openCopyMenuFor(row.id)
            break
        }
    }

    function copyOption(row, kind) {
        if (!row) return
        if (kind === "name") service.copyPeerName(row.payload)
        else if (kind === "dns") service.copyPeerDnsName(row.payload)
        else if (kind === "ip") service.copyPeerIp(row.payload)
        else {
            for (var i = 0; i < row.copyOptions.length; i++)
                if (row.copyOptions[i].kind === kind) service.copyToClipboard(row.copyOptions[i].label)
        }
    }

    function sendPeerFile(row) {
        if (!row || !row.payload) return
        // The file chooser takes over from here, so get the popup out of the way.
        service.sendFile(row.payload)
        root.expanded = false
    }

    function rowAction(row, actionId) {
        if (actionId === "send") sendPeerFile(row)
        else openCopyMenuFor(row.id)
    }

    // ---- row registry -------------------------------------------------------
    // Rows live inside nested Repeaters, so they register themselves here for
    // the two things reached by id rather than by binding: scrolling the cursor
    // into view, and opening a machine's copy menu from the keyboard.

    property var rowItems: ({})

    function registerRow(id, item) {
        var items = rowItems
        if (item) items[id] = item
        else delete items[id]
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
        var item = rowItems[cursorRowId()]
        if (!item) return
        Qt.callLater(function () {
            var flick = scroll.contentItem
            if (!flick || !item) return
            var margin = Kirigami.Units.gridUnit
            var point = item.mapToItem(flick.contentItem, 0, 0)
            var top = point.y
            var bottom = top + item.height
            var maxY = Math.max(0, flick.contentHeight - flick.height)
            if (top < flick.contentY + margin) flick.contentY = Math.max(0, top - margin)
            else if (bottom > flick.contentY + flick.height - margin)
                flick.contentY = Math.min(maxY, bottom + margin - flick.height)
        })
    }

    // ---- keyboard -----------------------------------------------------------

    focus: true
    Keys.onPressed: (event) => {
        switch (event.key) {
        case Qt.Key_Down:
        case Qt.Key_J:
            if (!full.cursorActive) full.cursorActive = true
            else full.moveCursor(1)
            event.accepted = true
            return
        case Qt.Key_Up:
        case Qt.Key_K:
            if (!full.cursorActive) full.cursorActive = true
            else full.moveCursor(-1)
            event.accepted = true
            return
        case Qt.Key_Return:
        case Qt.Key_Enter:
        case Qt.Key_Space:
            if (full.cursorActive) full.activateCursor()
            else full.cursorActive = true
            event.accepted = true
            return
        case Qt.Key_Escape:
            if (full.mullvadPickerOpen) full.mullvadPickerOpen = false
            else root.expanded = false
            event.accepted = true
            return
        case Qt.Key_T:
            full.service.toggleTailscale()
            event.accepted = true
            return
        case Qt.Key_R:
            full.service.refresh(true)
            event.accepted = true
            return
        case Qt.Key_C:
        case Qt.Key_N:
        case Qt.Key_D:
        case Qt.Key_S:
            var row = full.selectedRow()
            if (!row || row.kind !== "peer") return
            if (event.key === Qt.Key_C) full.copyOption(row, "ip")
            else if (event.key === Qt.Key_N) full.copyOption(row, "name")
            else if (event.key === Qt.Key_D) full.copyOption(row, "dns")
            else full.sendPeerFile(row)
            event.accepted = true
            return
        }
    }

    onVisibleChanged: {
        if (!visible) return
        cursorActive = false
        cursorIndex = 0
        _pinnedRowId = ""
        mullvadPickerOpen = false
        mullvadQuery = ""
        if (scroll.contentItem) scroll.contentItem.contentY = 0
        service.refresh()
        Qt.callLater(() => full.forceActiveFocus())
    }

    Timer {
        interval: 2800
        running: full.visible && full.panel.header.toggleChecked
        repeat: true
        onTriggered: full.phraseIndex = full.phraseIndex + 1
    }

    // ---- layout -------------------------------------------------------------

    QQC2.ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: scroll.availableWidth
            spacing: Kirigami.Units.smallSpacing

            RowSurface {
                Layout.fillWidth: true
                Layout.preferredHeight: heroRow.implicitHeight + Kirigami.Units.largeSpacing
                hasCursor: full.cursorActive && full.cursorIndex === 0 && full.panel.header.toggleVisible
                hovered: heroMouse.containsMouse

                MouseArea {
                    id: heroMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.NoButton
                    onContainsMouseChanged: if (containsMouse) {
                        full.cursorActive = true
                        full.cursorIndex = 0
                    }
                }

                RowLayout {
                    id: heroRow
                    anchors.fill: parent
                    anchors.leftMargin: Kirigami.Units.smallSpacing * 2
                    anchors.rightMargin: Kirigami.Units.smallSpacing * 2
                    spacing: Kirigami.Units.largeSpacing

                    TailscaleIcon {
                        iconSize: Kirigami.Units.iconSizes.medium
                        color: full.panel.header.dimmed ? full.dimColor : Kirigami.Theme.textColor
                        badgeColor: Kirigami.Theme.negativeTextColor
                        crossed: full.panel.header.crossed
                        warning: full.panel.header.warning
                        opacity: full.panel.header.dimmed ? 0.5 : 1.0
                        Layout.alignment: Qt.AlignVCenter
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        PlasmaExtras.Heading {
                            level: 4
                            text: full.panel.header.title
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }

                        PlasmaComponents3.Label {
                            text: full.panel.header.meta
                            color: full.dimColor
                            font: Kirigami.Theme.smallFont
                            elide: Text.ElideRight
                            Layout.fillWidth: true

                            NumberAnimation on opacity {
                                id: heroPhraseFade
                                running: false
                                from: 0.0
                                to: 1.0
                                duration: 260
                                easing.type: Easing.InQuad
                            }

                            Connections {
                                target: full
                                function onPhraseIndexChanged() { heroPhraseFade.restart() }
                            }
                        }
                    }

                    PlasmaComponents3.Switch {
                        visible: full.panel.header.toggleVisible
                        checked: full.panel.header.toggleChecked
                        enabled: full.panel.header.toggleEnabled
                        Layout.alignment: Qt.AlignVCenter
                        onToggled: full.service.toggleTailscale()

                        PlasmaComponents3.ToolTip.visible: hovered
                        PlasmaComponents3.ToolTip.text: full.panel.header.toggleHint
                    }
                }
            }

            PlasmaComponents3.Label {
                visible: full.panel.status.text !== ""
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.smallSpacing * 2
                Layout.rightMargin: Kirigami.Units.smallSpacing * 2
                text: full.panel.status.text
                color: full.panel.status.tone === "error" ? Kirigami.Theme.negativeTextColor : full.dimColor
                font: Kirigami.Theme.smallFont
                wrapMode: Text.WordWrap
            }

            Repeater {
                model: full.panel.sections

                ColumnLayout {
                    id: sectionView
                    required property var modelData

                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing
                    visible: modelData.visible

                    Kirigami.Separator {
                        Layout.fillWidth: true
                        Layout.topMargin: Kirigami.Units.smallSpacing
                    }

                    SectionHeader {
                        text: sectionView.modelData.title
                        Layout.fillWidth: true
                    }

                    PlasmaComponents3.Label {
                        visible: sectionView.modelData.rows.length === 0 && sectionView.modelData.empty !== ""
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        text: sectionView.modelData.empty
                        color: full.dimColor
                    }

                    Repeater {
                        model: sectionView.modelData.rows

                        ColumnLayout {
                            id: rowGroup
                            required property var modelData

                            readonly property bool isPicker: modelData.kind === "mullvadPicker"
                            readonly property bool isEmpty: modelData.kind === "empty"

                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            PlasmaComponents3.Label {
                                visible: rowGroup.isEmpty
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignHCenter
                                text: rowGroup.modelData.label
                                color: full.dimColor
                                font: Kirigami.Theme.smallFont
                            }

                            PanelRowView {
                                id: rowView
                                visible: !rowGroup.isEmpty
                                Layout.fillWidth: true
                                row: rowGroup.modelData
                                cursorActive: full.cursorActive
                                cursorRowId: full.cursorRowId()
                                dimColor: full.dimColor
                                onActivated: full.dispatch(rowGroup.modelData)
                                onHoveredRow: full.focusRow(rowGroup.modelData.id)
                                onActionTriggered: (actionId) => full.rowAction(rowGroup.modelData, actionId)
                                onCopyRequested: (kind) => full.copyOption(rowGroup.modelData, kind)
                                Component.onCompleted: full.registerRow(rowGroup.modelData.id, rowView)
                                Component.onDestruction: full.registerRow(rowGroup.modelData.id, null)
                            }

                            PlasmaComponents3.TextField {
                                visible: rowGroup.isPicker && rowGroup.modelData.expanded
                                Layout.fillWidth: true
                                Layout.leftMargin: Kirigami.Units.gridUnit
                                placeholderText: rowGroup.isPicker ? rowGroup.modelData.searchPlaceholder : ""
                                text: full.mullvadQuery
                                onTextChanged: full.mullvadQuery = text
                                onVisibleChanged: if (visible) Qt.callLater(() => forceActiveFocus())
                                Keys.onPressed: (event) => {
                                    if (event.key === Qt.Key_Escape) {
                                        full.mullvadPickerOpen = false
                                        full.forceActiveFocus()
                                        event.accepted = true
                                    } else if (event.key === Qt.Key_Down || event.key === Qt.Key_Up) {
                                        full.moveCursor(event.key === Qt.Key_Down ? 1 : -1)
                                        event.accepted = true
                                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                                        full.activateCursor()
                                        event.accepted = true
                                    }
                                }
                            }

                            // One level of nesting is all the model ever emits,
                            // so the children render through the same view
                            // without the component having to recurse.
                            Repeater {
                                model: rowGroup.modelData.expanded ? rowGroup.modelData.children : []

                                PanelRowView {
                                    id: childView
                                    required property var modelData
                                    Layout.fillWidth: true
                                    Layout.leftMargin: Kirigami.Units.gridUnit
                                    row: modelData
                                    cursorActive: full.cursorActive
                                    cursorRowId: full.cursorRowId()
                                    dimColor: full.dimColor
                                    onActivated: full.dispatch(childView.modelData)
                                    onHoveredRow: full.focusRow(childView.modelData.id)
                                    onCopyRequested: (kind) => full.copyOption(childView.modelData, kind)
                                    Component.onCompleted: full.registerRow(childView.modelData.id, childView)
                                    Component.onDestruction: full.registerRow(childView.modelData.id, null)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
