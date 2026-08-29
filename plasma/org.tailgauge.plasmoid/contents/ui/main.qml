import QtQuick
import org.kde.plasma.plasmoid
import org.kde.plasma.core as PlasmaCore
import "../code/model.js" as Model

PlasmoidItem {
    id: root

    readonly property alias tailscale: service
    readonly property bool showStatusInPanel: Plasmoid.configuration.showStatusInPanel

    readonly property var recentMullvadRegions: {
        var stored = Plasmoid.configuration.recentMullvadRegions
        return stored instanceof Array ? stored : []
    }

    function persistRecentMullvad(region) {
        var next = Model.pushRecentMullvad(root.recentMullvadRegions, region, 5)
        Plasmoid.configuration.recentMullvadRegions = next
        Plasmoid.configuration.writeConfig()
    }

    TailscaleService {
        id: service
        refreshIntervalSec: Plasmoid.configuration.refreshIntervalSec
    }

    Plasmoid.icon: "network-vpn"
    Plasmoid.status: service.active ? PlasmaCore.Types.ActiveStatus : PlasmaCore.Types.PassiveStatus

    toolTipMainText: service.installed ? (service.selfName || "Tailscale") : "Tailscale"
    toolTipSubText: {
        if (!service.installed) return i18n("Tailscale CLI is not installed or not on PATH.")
        if (service.lastError !== "") return service.lastError
        if (!service.active) return i18n("Tailscale is disconnected")
        var lines = [service.statusText]
        if (service.selfIp !== "") lines.push(service.selfIp)
        var exit = activeExitNodeName()
        if (exit !== "") lines.push(i18n("Exit node: %1", exit))
        return lines.join("\n")
    }

    function activeExitNodeName() {
        var nodes = service.exitNodes || []
        for (var i = 0; i < nodes.length; i++)
            if (nodes[i].ExitNode === true)
                return String(nodes[i].DisplayName || nodes[i].HostName || "")
        return ""
    }

    Plasmoid.contextualActions: [
        PlasmaCore.Action {
            text: service.active ? i18n("Turn Tailscale off") : i18n("Turn Tailscale on")
            icon.name: "network-vpn"
            enabled: service.installed
            onTriggered: service.toggleTailscale()
        },
        PlasmaCore.Action {
            text: i18n("Refresh")
            icon.name: "view-refresh"
            enabled: service.installed
            onTriggered: service.refresh(true)
        }
    ]

    compactRepresentation: CompactRep {}
    fullRepresentation: FullRep {}
}
