import QtQuick
import QtQuick.Shapes
import qs.Commons
import qs.Commons as Commons

// The ZeroTier mark, drawn as a vector path rather than loaded as an SVG so it
// can be painted in the panel's foreground colour like every other icon here.
//
// Path data is the Simple Icons ZeroTier icon (CC0-1.0), taken unmodified.
Item {
  id: root

  property real iconSize: Style.font.icon
  property color color: Commons.Color.foreground
  // The hero can afford the brand yellow; a bar icon has to follow the theme.
  property bool monochrome: true

  // The artwork's viewBox is "0 0 24 24".
  readonly property real viewSize: 24

  width: iconSize
  height: iconSize
  implicitWidth: iconSize
  implicitHeight: iconSize

  Item {
    id: canvas
    width: root.viewSize
    height: root.viewSize
    anchors.centerIn: parent
    scale: Math.min(root.width, root.height) / root.viewSize

    Shape {
      anchors.fill: parent
      antialiasing: true
      preferredRendererType: Shape.CurveRenderer

      ShapePath {
        fillColor: root.monochrome ? root.color : "#ffb441"
        strokeWidth: 0
        // The ring is cut out of the square by winding, as the SVG fills.
        fillRule: ShapePath.WindingFill
        PathSvg { path: "M4.01 0A3.999 3.999 0 0 0 .014 4v16c0 2.209 1.79 4 3.996 4h15.98a3.998 3.998 0 0 0 3.996-4V4c0-2.209-1.79-4-3.996-4zm-.672 2.834h17.326a.568.568 0 1 1 0 1.137h-8.129c.021.059.033.123.033.19v1.804A6.06 6.06 0 0 1 18.057 12c0 3.157-2.41 5.75-5.489 6.037v2.56a.568.568 0 1 1-1.136 0v-2.56A6.061 6.061 0 0 1 5.943 12a6.06 6.06 0 0 1 5.489-6.035V4.16c0-.066.012-.13.033-.19H3.338a.568.568 0 1 1 0-1.136zm8.094 4.307A4.89 4.89 0 0 0 7.113 12a4.89 4.89 0 0 0 4.319 4.86zm1.136 0v9.718A4.892 4.892 0 0 0 16.888 12a4.892 4.892 0 0 0-4.32-4.86z" }
      }
    }
  }
}
