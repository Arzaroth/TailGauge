import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Kirigami.FormLayout {
    property alias cfg_refreshIntervalSec: refreshInterval.value
    property alias cfg_showStatusInPanel: showStatus.checked

    QQC2.SpinBox {
        id: refreshInterval
        Kirigami.FormData.label: i18n("Refresh interval:")
        from: 5
        to: 3600
        stepSize: 5
        textFromValue: (value) => i18np("%1 second", "%1 seconds", value)
        valueFromText: (text) => parseInt(text, 10)
    }

    QQC2.CheckBox {
        id: showStatus
        Kirigami.FormData.label: i18n("Panel:")
        text: i18n("Show the machine name next to the icon")
    }
}
