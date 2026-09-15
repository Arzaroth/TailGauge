import QtQuick

QtObject {
    property string path: ""
    property bool watchChanges: false
    property bool printErrors: true
    property string contents: ""
    signal loaded()
    signal loadFailed()
    function text() { return contents }
}
