import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.components as PlasmaComponents3
import org.kde.kirigami as Kirigami

MouseArea {
    id: compact

    readonly property var service: root.tailscale
    readonly property bool vertical: Plasmoid.formFactor === PlasmaCore.Types.Vertical

    Layout.minimumWidth: contentRow.implicitWidth
    Layout.minimumHeight: Kirigami.Units.iconSizes.small
    Layout.preferredWidth: contentRow.implicitWidth
    Layout.preferredHeight: Kirigami.Units.iconSizes.small

    acceptedButtons: Qt.LeftButton | Qt.MiddleButton
    hoverEnabled: true

    // Right-click is left to Plasma's own applet menu, which carries the same
    // toggle and refresh as contextual actions.
    onClicked: (mouse) => {
        if (mouse.button === Qt.MiddleButton) service.toggleTailscale()
        else root.expanded = !root.expanded
    }

    RowLayout {
        id: contentRow
        anchors.centerIn: parent
        spacing: Kirigami.Units.smallSpacing

        TailscaleIcon {
            iconSize: Math.min(compact.height, Kirigami.Units.iconSizes.smallMedium)
            color: compact.service.active ? Kirigami.Theme.textColor
                                          : Qt.darker(Kirigami.Theme.textColor, 1.55)
            badgeColor: Kirigami.Theme.negativeTextColor
            crossed: !compact.service.active && !compact.service.needsLogin
            warning: compact.service.needsLogin
            Layout.alignment: Qt.AlignVCenter
        }

        PlasmaComponents3.Label {
            visible: root.showStatusInPanel && !compact.vertical && text !== ""
            text: compact.service.installed ? (compact.service.selfName || "") : ""
            elide: Text.ElideRight
            Layout.maximumWidth: Kirigami.Units.gridUnit * 8
            Layout.alignment: Qt.AlignVCenter
        }
    }
}
