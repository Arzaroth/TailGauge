import QtQuick
import Harness

// The Plasma service, loaded in a plain QML runtime against a recorded panel.
// Plasma is the one that goes out through a shell, so what this covers on top
// of the Omarchy harness is the quoting: a command line the engine keys a
// source by, with a JSON blob interpolated into it.
Item {
    Check { id: check; name: "plasma" }
    property var service: null

    Component.onCompleted: {
        service = check.load("../../plasma/org.tailgauge.plasmoid/contents/ui/ProviderService.qml", this)
        if (!service) return
        check.run("fixtures/panel.json", function (panelJson) {
            check.equal("no panel yet", service.panel.status.text, "Checking…")
            check.equal("nothing installed yet", service.installed, false)

            var asked = Registry.find("panel")
            check.ok("the service asked for a panel on its own", asked !== null)
            if (!asked) return

            // What crossed the shell: one `sh -c` with everything quoted inside it.
            var line = asked.commandLine
            check.ok("it goes through a shell", line.indexOf("sh -c ") === 0)
            check.ok("the binary is named", line.indexOf("tailgauge") !== -1)
            check.ok("the JSON survived the quoting", line.indexOf("activeProviderId") !== -1)
            check.ok("nothing unquoted broke out", line.indexOf("; rm ") === -1)

            asked.answer(panelJson, "", 0)

            check.equal("the panel landed", service.panel.header.title, "workstation")
            check.equal("installed follows the header", service.installed, true)
            check.equal("active follows the header", service.active, true)
            check.equal("selfName follows the header", service.selfName, "workstation")
            check.ok("the sections arrived", service.panel.sections.length === 7)

            // A click flips the local copy on the frame it was clicked.
            Registry.clear()
            service.down()
            check.equal("the click shows immediately", service.active, false)
            check.ok("and the command was run", Registry.find("ctl") !== null)

            // Output that is not a panel is reported, not drawn.
            Registry.clear()
            service.refresh()
            var broken = Registry.find("panel")
            if (broken) {
                broken.answer("not json at all", "", 0)
                check.ok("unreadable output is reported", service.lastError.length > 0)
                check.equal("and the last good panel stays", service.panel.header.title, "workstation")
            }
        })
    }
}
