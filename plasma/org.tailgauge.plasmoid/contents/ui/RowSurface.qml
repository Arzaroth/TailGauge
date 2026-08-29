import QtQuick
import org.kde.kirigami as Kirigami

Rectangle {
    id: surface

    property bool hasCursor: false
    property bool current: false
    property bool hovered: false

    function tint(alpha) {
        var c = Kirigami.Theme.highlightColor
        return Qt.rgba(c.r, c.g, c.b, alpha)
    }

    radius: Kirigami.Units.cornerRadius
    color: {
        if (current) return tint(0.28)
        if (hasCursor || hovered) return tint(0.14)
        return "transparent"
    }
    border.width: hasCursor ? 1 : 0
    border.color: tint(0.75)

    Behavior on color {
        ColorAnimation { duration: Kirigami.Units.shortDuration }
    }
}
