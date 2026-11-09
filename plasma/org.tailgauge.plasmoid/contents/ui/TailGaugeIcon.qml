import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: root

    property real iconSize: Kirigami.Units.iconSizes.small
    property color color: Kirigami.Theme.textColor
    property color badgeColor: Kirigami.Theme.negativeTextColor
    property bool crossed: false
    property bool warning: false
    property string providerId: ""
    property string badge: ""

    readonly property real countGap: root.badge !== "" ? Math.max(1, root.iconSize * 0.06) : 0

    implicitWidth: iconSize + (root.badge !== "" ? countGap + count.implicitWidth : 0)
    implicitHeight: iconSize
    width: implicitWidth
    height: iconSize

    // Native rendering of the Tailscale mark from the SVG: a 3x3 dot grid with
    // the inactive dots faded. Drawing it rather than shipping an SVG avoids
    // Qt's rendering quirks at panel icon sizes.
    component Dot: Rectangle {
        width: mark.dotSize
        height: mark.dotSize
        radius: width / 2
        color: root.color
    }

    Item {
        id: mark
        width: root.iconSize
        height: root.iconSize

        readonly property real dotSize: Math.max(2, root.iconSize * 0.24)
        readonly property real mid: (root.iconSize - dotSize) / 2
        readonly property real last: root.iconSize - dotSize

        Item {
            anchors.fill: parent
            visible: root.providerId !== "netbird"

            Dot { x: 0;          y: 0;          opacity: 0.24 }
            Dot { x: mark.mid;   y: 0;          opacity: 0.24 }
            Dot { x: mark.last;  y: 0;          opacity: 0.24 }
            Dot { x: 0;          y: mark.mid;   opacity: 1.0 }
            Dot { x: mark.mid;   y: mark.mid;   opacity: 1.0 }
            Dot { x: mark.last;  y: mark.mid;   opacity: 1.0 }
            Dot { x: 0;          y: mark.last;  opacity: 0.24 }
            Dot { x: mark.mid;   y: mark.last;  opacity: 1.0 }
            Dot { x: mark.last;  y: mark.last;  opacity: 0.24 }
        }

        Loader {
            anchors.fill: parent
            active: root.providerId === "netbird"
            sourceComponent: Component {
                NetBirdIcon {
                    iconSize: root.iconSize
                    color: root.color
                }
            }
        }

        Rectangle {
            visible: root.crossed
            anchors.centerIn: parent
            width: parent.width * 1.22
            height: Math.max(2, parent.height * 0.14)
            radius: height / 2
            color: root.color
            // NetBird's mark runs along the usual slash and would swallow it.
            rotation: root.providerId === "netbird" ? 45 : -45
        }

        Rectangle {
            visible: root.warning
            width: Math.max(7, parent.width * 0.42)
            height: width
            radius: width / 2
            color: root.badgeColor
            border.color: Kirigami.Theme.backgroundColor
            border.width: 1
            anchors.right: parent.right
            anchors.bottom: parent.bottom

            Text {
                anchors.centerIn: parent
                text: "!"
                color: Kirigami.Theme.backgroundColor
                font.pixelSize: Math.max(6, parent.height * 0.72)
                font.bold: true
            }
        }
    }

    Text {
        id: count
        visible: root.badge !== ""
        text: root.badge
        x: root.iconSize + root.countGap
        y: -root.iconSize * 0.18
        color: root.color
        font.pixelSize: Math.max(8, Math.round(root.iconSize * 0.66))
        font.bold: true
    }
}
