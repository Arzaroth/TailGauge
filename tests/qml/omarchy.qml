import QtQuick
import Harness

// The Omarchy service, loaded in a plain QML runtime against a recorded panel.
// Nothing is spawned and nothing is drawn: what this covers is the half that
// used to be untestable - the bindings, the JSON the frontend reads back, and
// what it hands the binary on the next call.
Item {
    Check { id: check; name: "omarchy" }
    property var service: null

    // Answer the poll in flight, and the one queued behind it, so the next
    // step starts with nothing running.
    function settle(json) {
        for (var proc = Registry.find("panel --json"); proc && proc.running;
             proc = Registry.find("panel --json"))
            proc.answer(json, "", 0)
    }

    Component.onCompleted: {
        service = check.load("../../omarchy/arzaroth.tailgauge/Service.qml", this)
        if (!service) return
        check.run("fixtures/panel.json", function (panelJson) {
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
            settle(panelJson)
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

            // Every detached command goes out. They used to share one Process,
            // so a copy started while a send was in flight was dropped.
            Registry.clear()
            service.copyToClipboard("first")
            service.sendFile({id: "n1", HostName: "box"})
            service.openUrl("https://example.invalid")
            check.equal("each detached command went out",
                        Registry.detachedCommands.length, 3)
            check.ok("the clipboard one carries its text",
                     Registry.detachedCommands[0].indexOf("copy first") !== -1)
            check.ok("the send one carries its peer",
                     Registry.detachedCommands[1].indexOf("send --peer") !== -1)

            // Output that is not a panel is reported, not drawn.
            settle(panelJson)
            Registry.clear()
            service.refresh()
            var broken = Registry.find("panel --json")
            if (broken) {
                broken.answer("not json at all", "", 0)
                check.ok("unreadable output is reported", service.lastError.length > 0)
                check.equal("and the last good panel stays", service.panel.header.title, "workstation")
            }

            // A refresh queued behind the answer that settles a click asks
            // without the click: the daemon has agreed, so it decides again.
            settle(panelJson)
            Registry.clear()
            service.down()
            var settling = Registry.find("panel --json")
            service.refresh()
            var agreed = JSON.parse(panelJson)
            agreed.header.toggleChecked = false
            settling.answer(JSON.stringify(agreed), "", 0)
            var queued = Registry.find("panel --json")
            check.ok("the queued refresh went out", queued !== null && queued !== settling || (queued && queued.running))
            if (queued)
                check.equal("it no longer carries the override", JSON.parse(queued.command[queued.command.length - 1]).active, undefined)

            // A switch while a poll is in flight: that answer was asked for the
            // old provider, so it is not drawn, and the new one is asked for as
            // soon as it lands rather than on the next tick.
            settle(panelJson)
            Registry.clear()
            service.refresh()
            var before = Registry.find("panel --json")
            service.switchProvider({id: "netbird"})
            before.answer(panelJson, "", 0)
            check.equal("the old provider's answer is not drawn",
                        service.panel.header.title, "TailGauge")
            var after = Registry.find("panel --json")
            check.ok("the new provider is asked for at once", after && after.running)
            if (after && after.running)
                check.equal("asked with the new provider",
                            JSON.parse(after.command[after.command.length - 1]).activeProviderId,
                            "netbird")
        })
    }
}
