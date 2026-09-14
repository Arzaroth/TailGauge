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

    ProviderService {
        id: service
        activeProviderId: Plasmoid.configuration.activeProvider
        onProviderChanged: function (id) {
            Plasmoid.configuration.activeProvider = id
            Plasmoid.configuration.writeConfig()
        }
        refreshIntervalSec: Plasmoid.configuration.refreshIntervalSec
        version: Plasmoid.metaData.version
        // An open popup is worth polling for; a closed one rides the watcher.
        attentive: root.expanded
    }

    Plasmoid.icon: "network-vpn"
    Plasmoid.status: service.active ? PlasmaCore.Types.ActiveStatus : PlasmaCore.Types.PassiveStatus

    // The provider the panel is driving, so the tooltip never names the wrong one.
    readonly property string providerLabel: Model.providerLabel(service.snapshot())
    readonly property var barState: Model.barState(service.snapshot(), function (text) { return text })

    toolTipMainText: service.installed ? (service.selfName || providerLabel) : providerLabel
    toolTipSubText: {
        // Every installed provider's state, not just the one being shown.
        var lines = root.barState.tooltip.slice()
        if (service.lastError !== "") lines.push(service.lastError)
        if (!service.installed || !service.active) return lines.join("\n")
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
