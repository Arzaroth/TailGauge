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

    property string focusSection: "header"
    property int accountIndex: 0
    property int peerIndex: 0
    property int exitNodeIndex: 0
    property int mullvadRegionIndex: 0
    property bool cursorActive: false
    property bool mullvadPickerOpen: false
    property string mullvadQuery: ""
    property int phraseIndex: 0

    readonly property var activePhrases: [
        i18n("Encrypting connections"),
        i18n("Sending secrets"),
        i18n("Guarding wires"),
        i18n("Braiding packets"),
        i18n("Polishing tunnels"),
        i18n("Hiding routes"),
        i18n("Sealing ports"),
        i18n("Sorting tailnets"),
        i18n("Shuffling keys"),
        i18n("Watching machines")
    ]
    readonly property string heroPhraseText: activePhrases[phraseIndex % activePhrases.length]

    readonly property color dimColor: Qt.darker(Kirigami.Theme.textColor, 1.55)

    readonly property bool showConnections: service.accounts.length > 1 || service.accountsAccessDenied
    readonly property bool showPeers: service.active && service.peers.length > 0
    readonly property var recentMullvadExitNodes: Model.recentMullvadNodes(service.mullvadRegions, root.recentMullvadRegions, 5)
    readonly property var exitNodes: displayExitNodes()
    readonly property bool showExitNodes: service.active && (exitNodes.length > 0 || service.mullvadRegions.length > 0)
    readonly property var filteredMullvadRegions: Model.filterMullvadRegions(service.mullvadRegions, mullvadQuery)

    // ---- selection ----------------------------------------------------------

    function displayExitNodes() {
        var nodes = []
        for (var i = 0; i < service.tailnetExitNodes.length; i++) nodes.push(service.tailnetExitNodes[i])
        for (var j = 0; j < recentMullvadExitNodes.length; j++) nodes.push(recentMullvadExitNodes[j])
        if (service.mullvadRegions.length > 0)
            nodes.push({ id: "mullvad:add", AddMullvad: true, DisplayName: i18n("Choose Mullvad region") })
        return nodes
    }

    function selectedPeer() {
        if (service.peers.length === 0) return null
        return service.peers[Math.max(0, Math.min(peerIndex, service.peers.length - 1))]
    }

    function selectedExitNode() {
        if (exitNodes.length === 0) return null
        return exitNodes[Math.max(0, Math.min(exitNodeIndex, exitNodes.length - 1))]
    }

    function selectedAccount() {
        if (service.accounts.length === 0) return null
        return service.accounts[Math.max(0, Math.min(accountIndex, service.accounts.length - 1))]
    }

    function selectedMullvadRegion() {
        if (filteredMullvadRegions.length === 0) return null
        return filteredMullvadRegions[Math.max(0, Math.min(mullvadRegionIndex, filteredMullvadRegions.length - 1))]
    }

    function chooseExitNode(peer) {
        if (!peer) return
        if (peer.AddMullvad === true) {
            mullvadPickerOpen = !mullvadPickerOpen
            mullvadRegionIndex = 0
            if (mullvadPickerOpen) Qt.callLater(function() { mullvadSearch.forceActiveFocus() })
            return
        }
        if (peer.Mullvad === true) root.persistRecentMullvad(Model.mullvadRegionKey(peer))
        service.setExitNode(peer)
        mullvadPickerOpen = false
    }

    function sendPeerFile(peer) {
        if (!service.canSendFiles(peer)) return
        // The file chooser takes over from here, so get the popup out of the way.
        service.sendFile(peer)
        root.expanded = false
    }

    function ensureCursor() {
        if (accountIndex >= service.accounts.length) accountIndex = Math.max(0, service.accounts.length - 1)
        if (peerIndex >= service.peers.length) peerIndex = Math.max(0, service.peers.length - 1)
        if (exitNodeIndex >= exitNodes.length) exitNodeIndex = Math.max(0, exitNodes.length - 1)
        if (mullvadRegionIndex >= filteredMullvadRegions.length) mullvadRegionIndex = Math.max(0, filteredMullvadRegions.length - 1)
        if (focusSection === "auth" && !service.accountsAccessDenied)
            focusSection = service.accounts.length > 1 ? "accounts" : (showExitNodes ? "exitNodes" : (showPeers ? "peers" : "header"))
        if (focusSection === "accounts" && service.accounts.length <= 1)
            focusSection = service.accountsAccessDenied ? "auth" : (showExitNodes ? "exitNodes" : (showPeers ? "peers" : "header"))
        if (focusSection === "peers" && !showPeers)
            focusSection = showExitNodes ? "exitNodes" : (service.accountsAccessDenied ? "auth" : (service.accounts.length > 1 ? "accounts" : "header"))
        if (focusSection === "exitNodes" && !showExitNodes)
            focusSection = showPeers ? "peers" : (service.accountsAccessDenied ? "auth" : (service.accounts.length > 1 ? "accounts" : "header"))
    }

    function moveCursor(dy) {
        cursorActive = true
        ensureCursor()
        if (dy === 0) return

        if (focusSection === "header") {
            if (dy > 0) {
                if (service.accountsAccessDenied) focusSection = "auth"
                else if (service.accounts.length > 1) focusSection = "accounts"
                else if (showExitNodes) focusSection = "exitNodes"
                else if (showPeers) focusSection = "peers"
            }
        } else if (focusSection === "auth") {
            if (dy < 0) focusSection = "header"
            else if (service.accounts.length > 1) focusSection = "accounts"
            else if (showExitNodes) focusSection = "exitNodes"
            else if (showPeers) focusSection = "peers"
        } else if (focusSection === "accounts") {
            if (dy < 0) {
                if (accountIndex <= 0) focusSection = service.accountsAccessDenied ? "auth" : "header"
                else accountIndex--
            } else {
                if (accountIndex < service.accounts.length - 1) accountIndex++
                else if (showExitNodes) focusSection = "exitNodes"
                else if (showPeers) focusSection = "peers"
            }
        } else if (focusSection === "exitNodes") {
            if (dy < 0) {
                if (exitNodeIndex <= 0) focusSection = service.accounts.length > 1 ? "accounts" : (service.accountsAccessDenied ? "auth" : "header")
                else exitNodeIndex--
            } else if (exitNodeIndex < exitNodes.length - 1) {
                exitNodeIndex++
            } else if (showPeers) {
                focusSection = "peers"
            }
        } else if (focusSection === "peers") {
            if (dy < 0) {
                if (peerIndex <= 0) focusSection = showExitNodes ? "exitNodes" : (service.accounts.length > 1 ? "accounts" : (service.accountsAccessDenied ? "auth" : "header"))
                else peerIndex--
            } else if (peerIndex < service.peers.length - 1) {
                peerIndex++
            }
        }

        ensureCursor()
        scrollCursorIntoView()
    }

    function activateCursor() {
        ensureCursor()
        if (focusSection === "header") service.toggleTailscale()
        else if (focusSection === "auth") service.authorizeTailscaleOperator()
        else if (focusSection === "accounts") {
            var account = selectedAccount()
            if (account) service.switchAccount(account.id)
        } else if (focusSection === "exitNodes") chooseExitNode(selectedExitNode())
        else if (focusSection === "peers") openSelectedPeerCopyMenu()
    }

    function moveMullvadRegionCursor(delta) {
        if (filteredMullvadRegions.length === 0) return
        cursorActive = true
        mullvadRegionIndex = Math.max(0, Math.min(filteredMullvadRegions.length - 1, mullvadRegionIndex + delta))
        scrollMullvadRegionCursorIntoView()
    }

    function activateMullvadRegionCursor() {
        var region = selectedMullvadRegion()
        if (region) chooseExitNode(region)
    }

    function openSelectedPeerCopyMenu() {
        if (peerIndex < 0 || peerIndex >= peerRepeater.count) return
        var item = peerRepeater.itemAt(peerIndex)
        if (item && item.openCopyMenu) item.openCopyMenu()
    }

    // ---- scrolling ----------------------------------------------------------

    function scrollItemIntoView(item) {
        if (!item) return
        Qt.callLater(function() {
            if (!item) return
            var flick = scroll.contentItem
            if (!flick) return
            var margin = Kirigami.Units.gridUnit
            var point = item.mapToItem(flick.contentItem, 0, 0)
            var top = point.y
            var bottom = top + item.height
            var viewTop = flick.contentY
            var viewBottom = viewTop + flick.height
            var maxY = Math.max(0, flick.contentHeight - flick.height)
            if (top < viewTop + margin) flick.contentY = Math.max(0, top - margin)
            else if (bottom > viewBottom - margin) flick.contentY = Math.min(maxY, bottom + margin - flick.height)
        })
    }

    function scrollCursorIntoView() {
        if (focusSection === "peers" && peerIndex >= 0 && peerIndex < peerRepeater.count)
            scrollItemIntoView(peerRepeater.itemAt(peerIndex))
        else if (focusSection === "exitNodes" && exitNodeIndex >= 0 && exitNodeIndex < exitNodeRepeater.count)
            scrollItemIntoView(exitNodeRepeater.itemAt(exitNodeIndex))
    }

    function scrollMullvadRegionCursorIntoView() {
        if (mullvadRegionIndex >= 0 && mullvadRegionIndex < mullvadRegionRepeater.count)
            scrollItemIntoView(mullvadRegionRepeater.itemAt(mullvadRegionIndex))
    }

    onPeerIndexChanged: scrollCursorIntoView()
    onExitNodeIndexChanged: scrollCursorIntoView()
    onMullvadRegionIndexChanged: if (mullvadPickerOpen) scrollMullvadRegionCursorIntoView()
    onShowConnectionsChanged: ensureCursor()
    onShowPeersChanged: ensureCursor()
    onShowExitNodesChanged: ensureCursor()

    Connections {
        target: full.service
        function onPeersChanged() { full.ensureCursor() }
        function onAccountsChanged() { full.ensureCursor() }
        function onAccountsAccessDeniedChanged() { full.ensureCursor() }
    }

    // ---- keyboard -----------------------------------------------------------

    focus: true
    Keys.onPressed: (event) => {
        switch (event.key) {
        case Qt.Key_Down:
        case Qt.Key_J:
            if (event.key === Qt.Key_J && (event.modifiers & Qt.ControlModifier)) break
            if (!full.cursorActive) full.cursorActive = true
            else full.moveCursor(1)
            event.accepted = true
            return
        case Qt.Key_Up:
        case Qt.Key_K:
            if (event.key === Qt.Key_K && (event.modifiers & Qt.ControlModifier)) break
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
            root.expanded = false
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
            full.service.copyPeerIp(full.selectedPeer())
            event.accepted = true
            return
        case Qt.Key_N:
            full.service.copyPeerName(full.selectedPeer())
            event.accepted = true
            return
        case Qt.Key_D:
            full.service.copyPeerDnsName(full.selectedPeer())
            event.accepted = true
            return
        case Qt.Key_S:
            full.sendPeerFile(full.selectedPeer())
            event.accepted = true
            return
        }
    }

    onVisibleChanged: {
        if (!visible) return
        cursorActive = false
        mullvadPickerOpen = false
        if (scroll.contentItem) scroll.contentItem.contentY = 0
        service.refresh()
        Qt.callLater(function() { full.forceActiveFocus() })
    }

    Timer {
        interval: 2800
        running: full.visible && full.service.active
        repeat: true
        onTriggered: full.phraseIndex = (full.phraseIndex + 1) % full.activePhrases.length
    }

    // ---- layout -------------------------------------------------------------

    QQC2.ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: scroll.availableWidth
            spacing: Kirigami.Units.smallSpacing

            // Header
            RowSurface {
                Layout.fillWidth: true
                Layout.preferredHeight: heroRow.implicitHeight + Kirigami.Units.largeSpacing
                hasCursor: full.cursorActive && full.focusSection === "header" && full.service.installed
                hovered: heroMouse.containsMouse

                MouseArea {
                    id: heroMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.NoButton
                    onContainsMouseChanged: if (containsMouse) {
                        full.cursorActive = true
                        full.focusSection = "header"
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
                        color: full.service.active ? Kirigami.Theme.textColor : full.dimColor
                        badgeColor: Kirigami.Theme.negativeTextColor
                        crossed: !full.service.active && !full.service.needsLogin
                        warning: full.service.needsLogin
                        opacity: full.service.active ? 1.0 : 0.5
                        Layout.alignment: Qt.AlignVCenter
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        PlasmaExtras.Heading {
                            level: 4
                            text: full.service.installed ? (full.service.selfName || "Tailscale") : "Tailscale"
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                        }

                        PlasmaComponents3.Label {
                            id: heroPhrase
                            text: full.service.active ? full.heroPhraseText : i18n("Tailscale is disconnected")
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
                        visible: full.service.installed
                        checked: full.service.active
                        enabled: !full.service.busy
                        Layout.alignment: Qt.AlignVCenter
                        onToggled: full.service.toggleTailscale()

                        PlasmaComponents3.ToolTip.visible: hovered
                        PlasmaComponents3.ToolTip.text: full.service.active
                            ? i18n("Turn Tailscale off")
                            : (full.service.needsLogin ? i18n("Authorize this device") : i18n("Turn Tailscale on"))
                    }
                }
            }

            // Status / error line
            PlasmaComponents3.Label {
                visible: full.service.actionStatus !== "" || full.service.lastError !== ""
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.smallSpacing * 2
                Layout.rightMargin: Kirigami.Units.smallSpacing * 2
                text: full.service.actionStatus !== "" ? full.service.actionStatus : full.service.lastError
                color: full.service.lastError !== "" && full.service.actionStatus === ""
                    ? Kirigami.Theme.negativeTextColor : full.dimColor
                font: Kirigami.Theme.smallFont
                wrapMode: Text.WordWrap
            }

            PlasmaComponents3.Label {
                visible: !full.service.installed
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing * 2
                text: i18n("Tailscale CLI is not installed or not on PATH.")
                color: full.dimColor
                wrapMode: Text.WordWrap
            }

            // Connections
            Kirigami.Separator {
                visible: full.showConnections
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.smallSpacing
            }

            SectionHeader {
                visible: full.showConnections
                text: i18n("Connections")
                Layout.fillWidth: true
            }

            AuthRow {
                visible: full.service.accountsAccessDenied
                Layout.fillWidth: true
            }

            Repeater {
                model: full.service.accounts
                AccountRow {
                    required property var modelData
                    required property int index
                    account: modelData
                    rowIndex: index
                    Layout.fillWidth: true
                }
            }

            // Exit nodes
            Kirigami.Separator {
                visible: full.showExitNodes
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.smallSpacing
            }

            SectionHeader {
                visible: full.showExitNodes
                text: i18n("Exit nodes")
                Layout.fillWidth: true
            }

            Repeater {
                id: exitNodeRepeater
                model: full.exitNodes
                ExitNodeRow {
                    required property var modelData
                    required property int index
                    peer: modelData
                    rowIndex: index
                    Layout.fillWidth: true
                }
            }

            PlasmaComponents3.TextField {
                id: mullvadSearch
                visible: full.mullvadPickerOpen
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.smallSpacing * 2
                Layout.rightMargin: Kirigami.Units.smallSpacing * 2
                placeholderText: i18n("Search regions")
                text: full.mullvadQuery
                onTextChanged: {
                    full.mullvadQuery = text
                    full.mullvadRegionIndex = 0
                }
                onAccepted: full.activateMullvadRegionCursor()
                Keys.onPressed: (event) => {
                    if (event.key === Qt.Key_Down) {
                        full.moveMullvadRegionCursor(1)
                        event.accepted = true
                    } else if (event.key === Qt.Key_Up) {
                        full.moveMullvadRegionCursor(-1)
                        event.accepted = true
                    } else if (event.key === Qt.Key_Escape) {
                        full.mullvadPickerOpen = false
                        full.forceActiveFocus()
                        event.accepted = true
                    }
                }
            }

            PlasmaComponents3.Label {
                visible: full.mullvadPickerOpen && full.filteredMullvadRegions.length === 0
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                text: i18n("No Mullvad regions found.")
                color: full.dimColor
                font: Kirigami.Theme.smallFont
            }

            Repeater {
                id: mullvadRegionRepeater
                model: full.mullvadPickerOpen ? full.filteredMullvadRegions : []
                MullvadRegionRow {
                    required property var modelData
                    required property int index
                    peer: modelData
                    rowIndex: index
                    Layout.fillWidth: true
                }
            }

            // Machines
            Kirigami.Separator {
                visible: full.service.installed && full.service.active
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.smallSpacing
            }

            SectionHeader {
                visible: full.service.installed && full.service.active
                text: i18n("Machines")
                Layout.fillWidth: true
            }

            PlasmaComponents3.Label {
                visible: full.service.installed && full.service.active && full.service.peers.length === 0
                Layout.fillWidth: true
                horizontalAlignment: Text.AlignHCenter
                text: i18n("No machines found on this tailnet.")
                color: full.dimColor
            }

            Repeater {
                id: peerRepeater
                model: full.showPeers ? full.service.peers : []
                PeerRow {
                    required property var modelData
                    required property int index
                    peer: modelData
                    rowIndex: index
                    Layout.fillWidth: true
                }
            }
        }
    }

    // ---- rows ---------------------------------------------------------------

    component AuthRow: RowSurface {
        id: authRow

        implicitHeight: authContent.implicitHeight + Kirigami.Units.largeSpacing
        hasCursor: full.cursorActive && full.focusSection === "auth"
        hovered: authMouse.containsMouse

        MouseArea {
            id: authMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: full.service.busy ? Qt.ArrowCursor : Qt.PointingHandCursor
            enabled: !full.service.busy
            onEntered: {
                full.cursorActive = true
                full.focusSection = "auth"
            }
            onClicked: full.service.authorizeTailscaleOperator()
        }

        RowLayout {
            id: authContent
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Kirigami.Units.smallSpacing * 2
            anchors.rightMargin: Kirigami.Units.smallSpacing * 2
            spacing: Kirigami.Units.smallSpacing * 2

            Kirigami.Icon {
                source: "security-medium-symbolic"
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                Layout.alignment: Qt.AlignVCenter
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                PlasmaComponents3.Label {
                    text: i18n("Authorize Tailscale operator")
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                PlasmaComponents3.Label {
                    text: i18n("Allow this user to operate this Tailscale profile")
                    color: full.dimColor
                    font: Kirigami.Theme.smallFont
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
        }
    }

    component AccountRow: RowSurface {
        id: accountRow

        property var account: null
        property int rowIndex: 0

        readonly property bool isSelected: account && account.selected === true
        readonly property bool isSwitching: account && full.service.switchingAccountId === String(account.id || "")

        implicitHeight: accountContent.implicitHeight + Kirigami.Units.largeSpacing
        hasCursor: full.cursorActive && full.focusSection === "accounts" && full.accountIndex === rowIndex
        current: isSelected
        hovered: accountMouse.containsMouse

        MouseArea {
            id: accountMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onEntered: {
                full.cursorActive = true
                full.focusSection = "accounts"
                full.accountIndex = accountRow.rowIndex
            }
            onClicked: if (accountRow.account) full.service.switchAccount(accountRow.account.id)
        }

        RowLayout {
            id: accountContent
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Kirigami.Units.smallSpacing * 2
            anchors.rightMargin: Kirigami.Units.smallSpacing * 2
            spacing: Kirigami.Units.smallSpacing * 2

            Kirigami.Icon {
                source: accountRow.isSelected ? "checkmark-symbolic" : "user-symbolic"
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                opacity: accountRow.isSwitching ? 0.45 : 1.0
                Layout.alignment: Qt.AlignVCenter

                SequentialAnimation on opacity {
                    running: accountRow.isSwitching
                    loops: Animation.Infinite
                    NumberAnimation { to: 1.0; duration: 420; easing.type: Easing.InOutQuad }
                    NumberAnimation { to: 0.45; duration: 420; easing.type: Easing.InOutQuad }
                }
            }

            PlasmaComponents3.Label {
                text: accountRow.account ? full.service.accountLabel(accountRow.account) : ""
                font.bold: accountRow.isSelected
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }

    component ExitNodeRow: RowSurface {
        id: exitNodeRow

        property var peer: null
        property int rowIndex: 0

        readonly property bool addMullvad: peer && peer.AddMullvad === true
        readonly property bool activeExitNode: peer && peer.ExitNode === true
        readonly property bool settingExitNode: peer && full.service.settingExitNodeId === String(peer.id || "")
        readonly property string peerName: peer ? String(peer.DisplayName || peer.HostName || i18n("Unknown")) : i18n("Unknown")

        implicitHeight: exitNodeContent.implicitHeight + Kirigami.Units.largeSpacing
        hasCursor: full.cursorActive && full.focusSection === "exitNodes" && full.exitNodeIndex === rowIndex
        current: activeExitNode || settingExitNode || (addMullvad && full.mullvadPickerOpen)
        hovered: exitNodeMouse.containsMouse

        MouseArea {
            id: exitNodeMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onEntered: {
                full.cursorActive = true
                full.focusSection = "exitNodes"
                full.exitNodeIndex = exitNodeRow.rowIndex
            }
            onClicked: full.chooseExitNode(exitNodeRow.peer)

            PlasmaComponents3.ToolTip.visible: containsMouse && !exitNodeRow.addMullvad
            PlasmaComponents3.ToolTip.text: exitNodeRow.activeExitNode ? i18n("Disconnect") : i18n("Connect")
        }

        RowLayout {
            id: exitNodeContent
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Kirigami.Units.smallSpacing * 2
            anchors.rightMargin: Kirigami.Units.smallSpacing * 2
            spacing: Kirigami.Units.smallSpacing * 2

            Kirigami.Icon {
                source: exitNodeRow.addMullvad
                    ? "list-add-symbolic"
                    : (exitNodeRow.peer && exitNodeRow.peer.Mullvad === true ? "network-vpn-symbolic" : "network-connect-symbolic")
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                Layout.alignment: Qt.AlignVCenter

                RotationAnimation on rotation {
                    running: exitNodeRow.settingExitNode
                    loops: Animation.Infinite
                    from: 0
                    to: 360
                    duration: 900
                }
                onRotationChanged: if (!exitNodeRow.settingExitNode && rotation !== 0) rotation = 0
            }

            PlasmaComponents3.Label {
                text: exitNodeRow.peerName
                font.bold: exitNodeRow.activeExitNode
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }

    component MullvadRegionRow: RowSurface {
        id: regionRow

        property var peer: null
        property int rowIndex: 0

        readonly property bool activeExitNode: peer && peer.ExitNode === true
        readonly property bool settingExitNode: peer && full.service.settingExitNodeId === String(peer.id || "")
        readonly property bool selectedRegion: full.mullvadPickerOpen && full.mullvadRegionIndex === rowIndex

        implicitHeight: regionContent.implicitHeight + Kirigami.Units.largeSpacing
        current: activeExitNode || settingExitNode || selectedRegion
        hovered: regionMouse.containsMouse

        MouseArea {
            id: regionMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onEntered: full.mullvadRegionIndex = regionRow.rowIndex
            onClicked: full.chooseExitNode(regionRow.peer)

            PlasmaComponents3.ToolTip.visible: containsMouse
            PlasmaComponents3.ToolTip.text: regionRow.activeExitNode ? i18n("Disconnect") : i18n("Connect")
        }

        RowLayout {
            id: regionContent
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Kirigami.Units.gridUnit
            anchors.rightMargin: Kirigami.Units.smallSpacing * 2
            spacing: Kirigami.Units.smallSpacing * 2

            Kirigami.Icon {
                source: "network-vpn-symbolic"
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                Layout.alignment: Qt.AlignVCenter
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                PlasmaComponents3.Label {
                    text: Model.mullvadRegionTitle(regionRow.peer)
                    font.bold: regionRow.activeExitNode
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                PlasmaComponents3.Label {
                    text: Model.mullvadRegionSubtitle(regionRow.peer)
                    visible: text !== ""
                    color: full.dimColor
                    font: Kirigami.Theme.smallFont
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
        }
    }

    component PeerRow: RowSurface {
        id: peerRow

        property var peer: null
        property int rowIndex: 0

        readonly property string peerName: peer ? String(peer.DisplayName || peer.HostName || i18n("Unknown")) : i18n("Unknown")
        readonly property string peerIp: peer && peer.TailscaleIPs && peer.TailscaleIPs.length > 0 ? String(peer.TailscaleIPs[0]) : ""
        readonly property string peerIpv6: peer && peer.TailscaleIPv6 && peer.TailscaleIPv6.length > 0 ? String(peer.TailscaleIPv6[0]) : ""
        readonly property string peerDns: peer ? String(peer.DNSName || "") : ""

        readonly property var copyOptions: {
            var options = []
            if (peerName !== "") options.push({ kind: "name", label: peerName })
            if (peerDns !== "") options.push({ kind: "dns", label: peerDns })
            if (peerIpv6 !== "") options.push({ kind: "ipv6", label: peerIpv6 })
            if (peerIp !== "") options.push({ kind: "ip", label: peerIp })
            return options
        }

        function openCopyMenu() {
            if (copyOptions.length === 0) return
            copyMenu.popup(copyButton, 0, copyButton.height)
        }

        function copyOption(kind) {
            if (kind === "name") full.service.copyPeerName(peer)
            else if (kind === "dns") full.service.copyPeerDnsName(peer)
            else if (kind === "ipv6") full.service.copyToClipboard(peerIpv6)
            else if (kind === "ip") full.service.copyPeerIp(peer)
        }

        implicitHeight: peerContent.implicitHeight + Kirigami.Units.largeSpacing
        hasCursor: full.cursorActive && full.focusSection === "peers" && full.peerIndex === rowIndex
        hovered: peerMouse.containsMouse

        MouseArea {
            id: peerMouse
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.NoButton
            onContainsMouseChanged: if (containsMouse) {
                full.cursorActive = true
                full.focusSection = "peers"
                full.peerIndex = peerRow.rowIndex
            }
        }

        RowLayout {
            id: peerContent
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: Kirigami.Units.smallSpacing * 2
            anchors.rightMargin: Kirigami.Units.smallSpacing
            spacing: Kirigami.Units.smallSpacing * 2

            Kirigami.Icon {
                source: Model.osIconName(peerRow.peer ? peerRow.peer.OS : "")
                implicitWidth: Kirigami.Units.iconSizes.small
                implicitHeight: Kirigami.Units.iconSizes.small
                Layout.alignment: Qt.AlignVCenter
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                PlasmaComponents3.Label {
                    text: peerRow.peerName
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                PlasmaComponents3.Label {
                    text: {
                        var parts = []
                        if (peerRow.peerIp !== "") parts.push(peerRow.peerIp)
                        if (peerRow.peerDns !== "") parts.push(peerRow.peerDns)
                        return parts.join(" · ")
                    }
                    color: full.dimColor
                    font: Kirigami.Theme.smallFont
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }

            PlasmaComponents3.ToolButton {
                visible: full.service.canSendFiles(peerRow.peer)
                icon.name: "document-send-symbolic"
                display: QQC2.AbstractButton.IconOnly
                text: i18n("Send files")
                Layout.alignment: Qt.AlignVCenter
                onClicked: full.sendPeerFile(peerRow.peer)

                PlasmaComponents3.ToolTip.visible: hovered
                PlasmaComponents3.ToolTip.text: i18n("Send files")
            }

            PlasmaComponents3.ToolButton {
                id: copyButton
                icon.name: "edit-copy-symbolic"
                display: QQC2.AbstractButton.IconOnly
                text: i18n("Copy")
                enabled: peerRow.copyOptions.length > 0
                Layout.alignment: Qt.AlignVCenter
                onClicked: peerRow.openCopyMenu()

                PlasmaComponents3.ToolTip.visible: hovered
                PlasmaComponents3.ToolTip.text: i18n("Copy")
            }
        }

        QQC2.Menu {
            id: copyMenu
            onClosed: Qt.callLater(function() { full.forceActiveFocus() })
        }

        Instantiator {
            model: peerRow.copyOptions
            delegate: QQC2.MenuItem {
                required property var modelData
                text: String(modelData.label || "")
                icon.name: "edit-copy-symbolic"
                onTriggered: peerRow.copyOption(String(modelData.kind || ""))
            }
            onObjectAdded: (index, object) => copyMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => copyMenu.removeItem(object)
        }
    }
}
