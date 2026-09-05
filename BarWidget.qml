import QtQuick
import Quickshell
import Quickshell.Io
import qs.Ui
import qs.Commons

BarWidget {
    id: root
    moduleName: "omabeam"

    readonly property var service: bar && bar.shell ? bar.shell.serviceFor("omabeam") : null
    readonly property string glyph: "󰐲"

    readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
    function open() { if (panelLoader.item) panelLoader.item.open() }
    function close() { if (panelLoader.item) panelLoader.item.close() }
    function togglePanel() { if (panelLoader.item) panelLoader.item.toggle() }

    readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false
    function closeForPopoutSwitch() { if (panelLoader.item) panelLoader.item.closeForPopoutSwitch() }

    function injectPanel() {
        var t = panelLoader.item
        if (!t) return
        if ("bar" in t) t.bar = root.bar
        if ("settings" in t) t.settings = root.settings
        if ("anchorItem" in t) t.anchorItem = button
        if ("hostWidget" in t) t.hostWidget = root
    }

    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight

    onBarChanged: injectPanel()
    onSettingsChanged: injectPanel()

    Loader {
        id: panelLoader
        active: true
        source: Qt.resolvedUrl("Panel.qml")
        visible: false
        onLoaded: {
            root.injectPanel()
            Qt.callLater(root.injectPanel)
        }
    }

    IpcHandler {
        target: "omabeam"
        function open(): void { root.open() }
        function close(): void { root.close() }
        function show(): void { root.open() }
        function hide(): void { root.close() }
        function toggle(): void { root.togglePanel() }
        function beam(): void { if (root.service) root.service.beam("") }
        function status(): void {
            var s = root.service
            console.log("omabeam status: opened=" + root.opened + " busy=" + (s ? s.busy : "no-service") + " serving=" + (s ? s.serving : "?") + " kind=" + (s ? s.kind : "?"))
        }
    }

    WidgetButton {
        id: button
        anchors.fill: parent
        bar: root.bar
        text: root.glyph
        tooltipText: service && service.busy ? "Beaming…" : (service && service.serving && service.url ? "Serving " + service.url : "OmaBeam — click to show QR")
        onPressed: function(b) {
            if (b === Qt.RightButton) return
            root.togglePanel()
        }
    }
}
