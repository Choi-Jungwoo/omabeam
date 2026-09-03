import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import qs.Ui
import qs.Commons

Panel {
    id: root
    moduleName: "omabeam"

    property var anchorItem: null
    property var hostWidget: null
    readonly property var barIdentity: hostWidget || root
    readonly property var service: bar && bar.shell ? bar.shell.serviceFor("omabeam") : null
    readonly property bool serviceReady: service != null

    readonly property string qrSource: {
        if (!service || !service.qrPng) return ""
        return "file://" + service.qrPng + "?r=" + service.revision
    }

    readonly property color fg: bar ? bar.foreground : Color.foreground
    readonly property color bg: bar ? bar.background : Color.background
    readonly property color accent: Color.accent
    readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

    function open() { root.controller.show() }
    function close() { root.controller.hide() }
    function toggle() { if (root.opened) root.close(); else root.open() }
    function closeForPopoutSwitch() { root.controller.hide() }
    readonly property bool popoutSwitchClosing: false
    function switchPanel(dir) {
        if (root.bar && typeof root.bar.switchPanelFrom === "function")
            return root.bar.switchPanelFrom(root.barIdentity, dir)
        return false
    }
    function scrollBy(dy) {
        if (!flick || flick.contentHeight <= flick.height) return
        flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, flick.contentY + dy))
    }

    TextEdit { id: clipProxy; visible: false; readOnly: true }
    function copyText(t) {
        clipProxy.text = String(t || "")
        clipProxy.selectAll()
        clipProxy.copy()
        clipProxy.deselect()
    }

    onOpenedChanged: {
        if (opened && service) service.beam("")
    }

    KeyboardPanel {
        id: panel
        anchorItem: root.anchorItem
        owner: root.barIdentity
        bar: root.bar
        open: root.opened
        contentWidth: panel.fittedContentWidth(360)
        contentHeight: panel.fittedContentHeight(contentCol.implicitHeight + 24, 520)

        PanelKeyCatcher {
            id: keyCatcher
            anchors.fill: parent
            onCloseRequested: root.close()
            onTabRequested: function(dir) { root.switchPanel(dir) }
            onMoveRequested: function(dx, dy) { if (dy !== 0) root.scrollBy(-dy * 24) }

            Flickable {
                id: flick
                anchors.fill: parent
                anchors.margins: Style.space(12)
                contentWidth: width
                contentHeight: contentCol.implicitHeight
                clip: true
                interactive: contentHeight > height

                Column {
                    id: contentCol
                    width: parent.width
                    spacing: Style.space(10)

                    Row {
                        width: parent.width
                        spacing: Style.space(8)
                        Text {
                            text: "OmaBeam"
                            color: root.fg
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.title
                            font.bold: true
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Text {
                            visible: service && service.busy
                            text: "● beaming"
                            color: root.accent
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.caption
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Item { width: Style.space(4); height: 1 }
                        Text {
                            visible: service && service.url
                            text: service.url
                            color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.45)
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.caption
                            elide: Text.ElideMiddle
                            width: parent.width - 120
                            anchors.verticalCenter: parent.verticalCenter
                        }
                    }

                    Rectangle {
                        width: parent.width
                        height: 280
                        radius: Style.space(8)
                        color: "white"
                        border.color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.10)
                        border.width: 1

                        Text {
                            visible: !service || service.busy
                            anchors.centerIn: parent
                            text: service && service.busy ? "Generating QR…" : "Opening…"
                            color: "#888"
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.body
                        }

                        Column {
                            visible: service && service.error && !service.busy
                            anchors.centerIn: parent
                            width: parent.width - Style.space(32)
                            spacing: Style.space(6)
                            Text {
                                width: parent.width
                                text: "⚠ " + (service ? service.error : "")
                                color: "#cc3333"
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.body
                                wrapMode: Text.Wrap
                                horizontalAlignment: Text.AlignHCenter
                            }
                            Text {
                                width: parent.width
                                text: "Copy something, then click Refresh."
                                color: "#888"
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.caption
                                horizontalAlignment: Text.AlignHCenter
                            }
                        }

                        Image {
                            id: qrImage
                            visible: service && !service.busy && !service.error && root.qrSource !== ""
                            anchors.centerIn: parent
                            width: 256
                            height: 256
                            fillMode: Image.PreserveAspectFit
                            cache: false
                            source: root.qrSource
                            asynchronous: true
                            smooth: false
                        }
                    }

                    Text {
                        visible: service && service.detail && !service.error
                        width: parent.width
                        text: {
                            if (!service) return ""
                            var k = String(service.kind || "")
                            var d = String(service.detail || "")
                            if (k === "text") return "Clipboard text · " + d.length + " chars"
                            if (k === "text-link") return "Text link · " + d
                            if (k === "file") return "File · " + d
                            if (k === "image") return "Image · " + d
                            return d
                        }
                        color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.75)
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                        elide: Text.ElideMiddle
                        maximumLineCount: 2
                        wrapMode: Text.Wrap
                    }

                    Row {
                        visible: service && service.url && !service.error
                        width: parent.width
                        spacing: Style.space(6)
                        Text {
                            width: parent.width - 70
                            text: service ? service.url : ""
                            color: root.accent
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.caption
                            elide: Text.ElideMiddle
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        Button {
                            text: "Copy"
                            anchors.verticalCenter: parent.verticalCenter
                            onClicked: root.copyText(service ? service.url : "")
                        }
                    }

                    RowLayout {
                        width: parent.width
                        spacing: Style.space(8)
                        Button {
                            text: "Refresh"
                            enabled: service && !service.busy
                            onClicked: if (service) service.refresh()
                        }
                        Button {
                            text: "Stop"
                            visible: service && service.url
                            enabled: service && !service.busy
                            onClicked: if (service) service.stopServing()
                        }
                        Item { Layout.fillWidth: true; height: 1 }
                        Text {
                            visible: service && service.kind === "text" && !service.url
                            text: "Scans directly"
                            color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.5)
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.caption
                            Layout.alignment: Qt.AlignVCenter
                        }
                    }

                    Text {
                        width: parent.width
                        text: "Scans once. File/image links serve on your LAN (port 61234) until you close or refresh. Nothing leaves your network."
                        color: Qt.rgba(root.fg.r, root.fg.g, root.fg.b, 0.45)
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                        wrapMode: Text.Wrap
                    }
                }
            }
        }
    }
}
