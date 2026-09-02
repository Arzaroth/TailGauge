import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.plasma.components as PlasmaComponents3
import org.kde.kirigami as Kirigami
import "../code/model.js" as Model

// One resolved row, whatever its kind. Everything it shows comes off `row`;
// this file only picks the widgets.
RowSurface {
    id: surface

    property var row: null
    property var panel: null

    // A delegate now outlives the row it happens to be showing, so registration
    // follows the row rather than the component. `register` is the registry's
    // own function, handed in by whoever owns it - it is what scrolls the
    // cursor into view and opens a machine's copy menu from the keyboard.
    property var register: null
    property string registeredId: ""

    function syncRegistration() {
        if (!register) return
        var id = row ? String(row.id) : ""
        if (id === registeredId) return
        // Handed a different machine with its copy menu open, this row would
        // otherwise offer the new one's addresses under the name that was
        // clicked.
        copyMenu.dismiss()
        if (registeredId !== "") register(registeredId, surface, false)
        registeredId = id
        if (id !== "") register(id, surface, true)
    }

    onRowChanged: syncRegistration()
    Component.onCompleted: syncRegistration()
    Component.onDestruction: if (register && registeredId !== "") register(registeredId, surface, false)
    property bool cursorActive: false
    property string cursorRowId: ""
    property color dimColor: Qt.darker(Kirigami.Theme.textColor, 1.55)

    signal activated()
    signal hoveredRow()
    signal actionTriggered(string actionId)
    signal copyRequested(string kind)

    function openCopyMenu() {
        if (!row || row.copyOptions.length === 0) return
        copyMenu.popup(surface, surface.width - Kirigami.Units.gridUnit * 2, surface.height)
    }

    implicitHeight: rowContent.implicitHeight + Kirigami.Units.largeSpacing
    hasCursor: cursorActive && row && cursorRowId === row.id
    current: row ? row.current : false
    hovered: rowMouse.containsMouse

    MouseArea {
        id: rowMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onContainsMouseChanged: if (containsMouse) surface.hoveredRow()
        onClicked: surface.activated()

        PlasmaComponents3.ToolTip.visible: containsMouse && surface.row && surface.row.hint !== ""
        PlasmaComponents3.ToolTip.text: surface.row ? surface.row.hint : ""
    }

    RowLayout {
        id: rowContent
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: Kirigami.Units.smallSpacing * 2
        anchors.rightMargin: Kirigami.Units.smallSpacing
        spacing: Kirigami.Units.smallSpacing * 2

        Kirigami.Icon {
            id: rowIcon
            source: surface.row ? surface.row.icon : ""
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: Kirigami.Units.iconSizes.small
            Layout.alignment: Qt.AlignVCenter

            RotationAnimation on rotation {
                running: surface.row ? surface.row.busy : false
                loops: Animation.Infinite
                from: 0
                to: 360
                duration: 900
            }
            onRotationChanged: if (surface.row && !surface.row.busy && rotation !== 0) rotation = 0
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            PlasmaComponents3.Label {
                text: surface.row ? surface.row.label : ""
                font.bold: surface.row ? surface.row.bold : false
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            PlasmaComponents3.Label {
                visible: text !== ""
                text: surface.row ? surface.row.sublabel : ""
                color: surface.dimColor
                font: Kirigami.Theme.smallFont
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        Repeater {
            model: surface.row ? surface.row.actions : []

            PlasmaComponents3.ToolButton {
                required property var modelData
                icon.name: modelData.icon
                display: QQC2.AbstractButton.IconOnly
                text: modelData.label
                Layout.alignment: Qt.AlignVCenter
                onClicked: surface.actionTriggered(modelData.id)

                PlasmaComponents3.ToolTip.visible: hovered
                PlasmaComponents3.ToolTip.text: modelData.label
            }
        }
    }

    QQC2.Menu {
        id: copyMenu
    }

    Instantiator {
        model: surface.row ? surface.row.copyOptions : []
        delegate: QQC2.MenuItem {
            required property var modelData
            text: modelData.label
            icon.name: "edit-copy-symbolic"
            onTriggered: surface.copyRequested(modelData.kind)
        }
        onObjectAdded: (index, object) => copyMenu.insertItem(index, object)
        onObjectRemoved: (index, object) => copyMenu.removeItem(object)
    }
}
