import QtQuick
import org.kde.plasma.plasmoid
import org.kde.plasma.core as PlasmaCore

PlasmoidItem {
    id: root

    readonly property alias tailscale: service
    readonly property bool showStatusInPanel: Plasmoid.configuration.showStatusInPanel

    readonly property var recentMullvadRegions: {
        var stored = Plasmoid.configuration.recentMullvadRegions
        return stored instanceof Array ? stored : []
    }

    // The chosen region to the front, the rest in the order they were, capped.
    function persistRecentMullvad(region) {
        if (String(region || "") === "") return
        var next = [String(region)]
        var recent = root.recentMullvadRegions
        for (var i = 0; i < recent.length && next.length < 5; i++) {
            var existing = String(recent[i] || "")
            if (existing !== "" && existing !== region && next.indexOf(existing) === -1)
                next.push(existing)
        }
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

    // The header already names what is being driven, and falls back to the
    // provider's own name when there is no machine to name instead.
    readonly property var barState: service.panel.bar

    toolTipMainText: service.panel.header.title
    toolTipSubText: {
        // Every installed provider's state, not just the one being shown.
        var lines = root.barState.tooltip.slice()
        if (service.lastError !== "") lines.push(service.lastError)
        if (!service.installed || !service.active) return lines.join("\n")
        var exit = activeExitNodeName()
        if (exit !== "") lines.push(i18n("Exit node: %1", exit))
        return lines.join("\n")
    }

    // The switcher section is the drivable providers, in order, so the next one
    // is the row after the current one.
    function cycleProvider() {
        var sections = service.panel.sections
        for (var s = 0; s < sections.length; s++) {
            if (sections[s].id !== "providers") continue
            var rows = sections[s].rows
            if (rows.length < 2) return
            for (var i = 0; i < rows.length; i++)
                if (rows[i].current) return service.switchProvider(rows[(i + 1) % rows.length].payload)
            service.switchProvider(rows[0].payload)
            return
        }
    }

    // The node in use is the current row of the exit-node section, which the
    // binary already worked out.
    function activeExitNodeName() {
        var sections = service.panel.sections
        for (var s = 0; s < sections.length; s++) {
            if (sections[s].id !== "exitNodes") continue
            var rows = sections[s].rows
            for (var i = 0; i < rows.length; i++)
                if (rows[i].kind === "exitNode" && rows[i].current) return String(rows[i].label)
            return ""
        }
        return ""
    }

    Plasmoid.contextualActions: [
        PlasmaCore.Action {
            // Named by the binary, so this cannot say Tailscale while the
            // panel is driving something else.
            text: service.panel.header.toggleHint
            icon.name: "network-vpn"
            enabled: service.installed
            onTriggered: service.toggleTailscale()
        },
        PlasmaCore.Action {
            text: i18n("Refresh")
            icon.name: "view-refresh"
            enabled: service.installed
            onTriggered: service.refresh()
        }
    ]

    compactRepresentation: CompactRep {}
    fullRepresentation: FullRep {}
}
