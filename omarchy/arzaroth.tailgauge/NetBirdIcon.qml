import QtQuick
import QtQuick.Shapes
import qs.Commons

// The NetBird mark, drawn as vector paths rather than loaded as an SVG so it
// can be painted in the panel's foreground colour like every other icon here.
//
// Path data is NetBird's own artwork, taken unmodified from the brand SVG. The
// approach, and the note about fill rules below, follow vstoms.netbird
// (MIT, Copyright (c) 2026 Viggo Stomsvik), which solved the same problem
// first.
Item {
  id: root

  property real iconSize: Style.font.icon
  property color color: Color.foreground
  // The hero can afford the brand oranges; a bar icon has to follow the theme.
  property bool monochrome: true

  // The artwork's viewBox is "0.015 36.7 512 405.4": the paths start at
  // y~36.7, so the canvas is shifted before it is scaled.
  readonly property real viewX: 0.015
  readonly property real viewY: 36.7
  readonly property real viewWidth: 512
  readonly property real viewHeight: 405.4

  width: iconSize
  height: iconSize
  implicitWidth: iconSize
  implicitHeight: iconSize

  Item {
    id: canvas
    width: root.viewWidth
    height: root.viewHeight
    anchors.centerIn: parent
    scale: Math.min(root.width / root.viewWidth, root.height / root.viewHeight)

    Shape {
      x: -root.viewX
      y: -root.viewY
      width: canvas.width
      height: canvas.height
      antialiasing: true
      // Analytic antialiasing keeps the mark crisp once a viewBox-sized canvas
      // is scaled down to about 24px.
      preferredRendererType: Shape.CurveRenderer

      Mark {
        brandColor: "#f68330"
        PathSvg { path: "m363.915 69.9c-61.8 5.7-92.5 41.3-104.1 59.3l-5.2 9.1c-.4.8-.6 1.3-.6 1.3l-.1-.1-174.8 302.6h218l214.9-372.2z" }
      }
      Mark {
        brandColor: "#f68330"
        PathSvg { path: "m297.115 442.1-297.1-315.2s336-90.2 368.7 191.4z" }
      }
      Mark {
        brandColor: "#f35e32"
        PathSvg { path: "m253.115 140.8-91.2 157.9 135.2 143.4 71.6-124c-11.3-96.9-58.5-149.7-115.6-177.3" }
      }
    }
  }

  component Mark: ShapePath {
    property color brandColor: "#f68330"

    fillColor: root.monochrome ? root.color : brandColor
    strokeWidth: 0
    // SVG fills nonzero by default; Shape defaults to odd-even, which would
    // punch holes through the overlapping parts of the mark.
    fillRule: ShapePath.WindingFill
  }
}
