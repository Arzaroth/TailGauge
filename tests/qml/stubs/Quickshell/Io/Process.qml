import QtQuick
import Harness

// Stands in for Quickshell's Process. Nothing is spawned: the harness answers
// by command line, so what is under test is what the frontend does with an
// answer rather than how the answer arrived. The binary's own end-to-end tests
// cover the spawning.
QtObject {
    id: proc
    property var command: []
    property bool running: false
    property var stdout: null
    property var stderr: null
    signal exited(int exitCode, int signalCode)

    onRunningChanged: if (running) Registry.started(proc)

    /// What the harness calls to answer a command that was started.
    function answer(out, err, code) {
        if (stdout) { stdout.text = out; stdout.streamFinished() }
        if (stderr) { stderr.text = err; stderr.streamFinished() }
        running = false
        exited(code, 0)
    }
}
