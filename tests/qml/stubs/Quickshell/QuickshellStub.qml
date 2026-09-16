pragma Singleton
import QtQuick
import Harness

// Stands in for the Quickshell singleton. Nothing is spawned: a detached
// command is recorded like any other, so a harness can assert that each one
// went out rather than that the last one overwrote the others.
QtObject {
    function execDetached(argv) {
        Registry.detached(argv)
    }
}
