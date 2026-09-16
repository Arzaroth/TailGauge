pragma Singleton
import QtQuick

// Every command a frontend started, so a harness can answer one without the
// frontend having to hand it over.
QtObject {
    property var running: []

    function started(proc) { running.push(proc) }

    /// The running command containing `needle`, or null.
    function find(needle) {
        for (var i = running.length - 1; i >= 0; i--) {
            var proc = running[i]
            var line = proc.commandLine !== undefined ? proc.commandLine : proc.command.join(" ")
            if (line.indexOf(needle) !== -1) return proc
        }
        return null
    }

    property var detachedCommands: []

    function detached(argv) { detachedCommands.push(argv.join(" ")) }

    function clear() { running = []; detachedCommands = [] }
}
