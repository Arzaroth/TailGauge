import QtQuick
import Harness
import "../../omarchy/arzaroth.tailgauge" as Widget

// The Omarchy service, loaded in a plain QML runtime against a recorded panel.
// Nothing is spawned and nothing is drawn: what this covers is the half that
// used to be untestable - the bindings, the JSON the frontend reads back, and
// what it hands the binary on the next call.
Item {
    Check { id: check; name: "omarchy" }
    Widget.Service { id: service }

    Component.onCompleted: check.run("fixtures/panel.json", function (panelJson) {
        // Before any answer, the service says so rather than drawing nothing.
        check.equal("no panel yet", service.panel.status.text, "Checking…")
        check.equal("nothing installed yet", service.installed, false)

        var asked = Registry.find("panel --json")
        check.ok("the service asked for a panel on its own", asked !== null)
        if (!asked) return

        // What it asked with: the state only this side knows.
        var ui = JSON.parse(asked.command[asked.command.length - 1])
        check.equal("its own version is offered", typeof ui.version, "string")
        check.equal("no optimistic override yet", ui.active, undefined)

        asked.answer(panelJson, "", 0)

        // The answer arrived and the derived properties followed it.
        check.equal("the panel landed", service.panel.header.title, "workstation")
        check.equal("installed follows the header", service.installed, true)
        check.equal("active follows the header", service.active, true)
        check.equal("status follows the panel", service.statusText, "")
        check.ok("the sections arrived", service.panel.sections.length === 7)
        check.ok("the traversal arrived", service.panel.navigation.length > 1)

        // A click flips the local copy and says so on the next call.
        Registry.clear()
        service.down()
        check.equal("the click shows immediately", service.active, false)
        var again = Registry.find("panel --json")
        check.ok("the click asked again", again !== null)
        if (again) {
            var next = JSON.parse(again.command[again.command.length - 1])
            check.equal("the override is carried", next.active, false)
        }
        check.ok("and the command was run", Registry.find("ctl") !== null)

        // The daemon agreeing clears the override rather than fighting it.
        var reconcile = Registry.find("panel --json")
        var off = JSON.parse(panelJson)
        off.header.toggleChecked = false
        reconcile.answer(JSON.stringify(off), "", 0)
        check.equal("the override is cleared once agreed", service._desired, -1)

        // Output that is not a panel is reported, not drawn.
        Registry.clear()
        service.refresh()
        var broken = Registry.find("panel --json")
        if (broken) {
            broken.answer("not json at all", "", 0)
            check.ok("unreadable output is reported", service.lastError.length > 0)
            check.equal("and the last good panel stays", service.panel.header.title, "workstation")
        }
    })
}
