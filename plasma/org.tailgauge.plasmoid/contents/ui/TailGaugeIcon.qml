import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: root

    property real iconSize: Kirigami.Units.iconSizes.small
    property color color: Kirigami.Theme.textColor
    property color badgeColor: Kirigami.Theme.negativeTextColor
    property bool crossed: false
    property bool warning: false

    implicitWidth: iconSize
    implicitHeight: iconSize
    width: iconSize
    height: iconSize

    readonly property real dotSize: Math.max(2, root.iconSize * 0.24)
    readonly property real mid: (root.iconSize - dotSize) / 2
    readonly property real last: root.iconSize - dotSize

    // Native rendering of the Tailscale mark from the SVG: a 3x3 dot grid with
    // the inactive dots faded. Drawing it rather than shipping an SVG avoids
    // Qt's rendering quirks at panel icon sizes.
    component Dot: Rectangle {
        width: root.dotSize
        height: root.dotSize
        radius: width / 2
        color: root.color
    }

    Dot { x: 0;          y: 0;          opacity: 0.24 }
    Dot { x: root.mid;   y: 0;          opacity: 0.24 }
    Dot { x: root.last;  y: 0;          opacity: 0.24 }
    Dot { x: 0;          y: root.mid;   opacity: 1.0 }
    Dot { x: root.mid;   y: root.mid;   opacity: 1.0 }
    Dot { x: root.last;  y: root.mid;   opacity: 1.0 }
    Dot { x: 0;          y: root.last;  opacity: 0.24 }
    Dot { x: root.mid;   y: root.last;  opacity: 1.0 }
    Dot { x: root.last;  y: root.last;  opacity: 0.24 }

    Rectangle {
        visible: root.crossed
        anchors.centerIn: parent
        width: parent.width * 1.22
        height: Math.max(2, parent.height * 0.14)
        radius: height / 2
        color: root.color
        rotation: -45
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
