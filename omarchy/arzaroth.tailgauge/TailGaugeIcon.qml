import QtQuick
import qs.Commons
import qs.Commons as Commons
import qs.Ui

Item {
  id: root

  property real iconSize: Style.font.icon
  property color color: Commons.Color.foreground
  property color badgeColor: Commons.Color.urgent
  property bool crossed: false
  property bool warning: false
  property string providerId: ""
  property string badge: ""

  readonly property real countGap: root.badge !== "" ? Math.max(1, root.iconSize * 0.06) : 0

  width: implicitWidth
  height: iconSize
  implicitWidth: iconSize + (root.badge !== "" ? countGap + count.implicitWidth : 0)
  implicitHeight: iconSize

  // Native rendering of the Tailscale mark from the SVG: a 3x3 dot grid with
  // the inactive dots faded. Drawing it rather than shipping an SVG avoids Qt's
  // rendering quirks at bar icon sizes.
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

      Dot { x: 0;         y: 0;         opacity: 0.24 }
      Dot { x: mark.mid;  y: 0;         opacity: 0.24 }
      Dot { x: mark.last; y: 0;         opacity: 0.24 }
      Dot { x: 0;         y: mark.mid;  opacity: 1.0 }
      Dot { x: mark.mid;  y: mark.mid;  opacity: 1.0 }
      Dot { x: mark.last; y: mark.mid;  opacity: 1.0 }
      Dot { x: 0;         y: mark.last; opacity: 0.24 }
      Dot { x: mark.mid;  y: mark.last; opacity: 1.0 }
      Dot { x: mark.last; y: mark.last; opacity: 0.24 }
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

    BorderSurface {
      visible: root.warning
      width: Math.max(7, parent.width * 0.42)
      height: width
      radius: width / 2
      color: root.badgeColor
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      borderSpec: Border.flat(Commons.Color.popups.background, 1)

      Text {
        anchors.centerIn: parent
        text: "!"
        color: Commons.Color.background
        font.family: Style.font.family
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
    font.family: Style.font.family
    font.pixelSize: Math.max(8, Math.round(root.iconSize * 0.66))
    font.bold: true
  }
}
