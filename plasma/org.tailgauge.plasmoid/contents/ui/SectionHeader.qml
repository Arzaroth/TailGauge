import QtQuick
import QtQuick.Layouts
import org.kde.plasma.components as PlasmaComponents3
import org.kde.kirigami as Kirigami

PlasmaComponents3.Label {
    Layout.leftMargin: Kirigami.Units.smallSpacing * 2
    Layout.rightMargin: Kirigami.Units.smallSpacing * 2
    Layout.topMargin: Kirigami.Units.smallSpacing

    color: Qt.darker(Kirigami.Theme.textColor, 1.4)
    font.family: Kirigami.Theme.smallFont.family
    font.pointSize: Kirigami.Theme.smallFont.pointSize
    font.capitalization: Font.AllUppercase
    font.bold: true
    elide: Text.ElideRight
}
