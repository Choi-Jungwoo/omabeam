import QtQuick
import Quickshell
import Quickshell.Io

// Long-running OmaBeam panel backend.
// Owns one `omabeam panel` Process at a time.
// That process prints ONE JSON line to stdout then either exits (text QR)
// or stays alive serving the file (tiny_http). The panel re-beams on every open
// so "latest QR" is always fresh without polling the clipboard.
Item {
    id: root
    visible: false
    width: 0
    height: 0

    property var shell: null
    property var manifest: null

    readonly property string pluginId: manifest && manifest.id ? String(manifest.id) : "omabeam"
    readonly property string pluginDir: manifest && manifest.__sourceDir ? String(manifest.__sourceDir) : ""

    readonly property string qrPng: {
        var r = Quickshell.env("XDG_RUNTIME_DIR")
        if (r && String(r).length > 0) return String(r) + "/omabeam-qr.png"
        return "/tmp/omabeam-qr.png"
    }

    // Public state read by BarWidget / Panel
    property string detail: ""
    property string kind: ""
    property string url: ""
    property string filename: ""
    property string contentType: ""
    property string error: ""
    property bool busy: false
    // Loading (waiting for beam JSON) vs serving (LAN HTTP alive).
    // busy clears on first JSON line; serving mirrors beamProc.running.
    readonly property bool serving: beamProc.running
    property int revision: 0
    property double lastBeamMs: 0
    property string pendingFile: ""

    function clearState() {
        detail = ""
        kind = ""
        url = ""
        filename = ""
        contentType = ""
        error = ""
    }

    function beam(filePath) {
        if (beamProc.running) beamProc.running = false
        clearState()
        busy = true
        pendingFile = filePath ? String(filePath) : ""
        var home = Quickshell.env("HOME")
        var cargoBin = home ? home + "/.cargo/bin/omabeam" : ""
        var bin = cargoBin
        if (pendingFile.length > 0)
            beamProc.command = ["sh", "-c", "exec \"$1\" panel --qr-file \"$2\" \"$3\" 2>&1 || exec \"$4\" panel --qr-file \"$2\" \"$3\" || exec omabeam panel --qr-file \"$2\" \"$3\"", "sh", bin, root.qrPng, pendingFile, pluginDir + "/target/debug/omabeam"]
        else
            beamProc.command = ["sh", "-c", "exec \"$1\" panel --qr-file \"$2\" 2>&1 || exec \"$3\" panel --qr-file \"$2\" || exec omabeam panel --qr-file \"$2\"", "sh", bin, root.qrPng, pluginDir + "/target/debug/omabeam"]
        beamProc.running = true
        lastBeamMs = Date.now()
    }

    function refresh() { beam("") }
    function stopServing() {
        if (beamProc.running) beamProc.running = false
    }

    function tryFallback(exitCode) {
        if (exitCode === 127 && pluginDir) {
            var dbg = pluginDir + "/target/debug/omabeam"
            console.warn("omabeam not in PATH, retrying with", dbg)
            clearState()
            busy = true
            if (pendingFile.length > 0)
                beamProc.command = ["sh", "-c", "exec \"$1\" panel --qr-file \"$2\" \"$3\"", "sh", dbg, root.qrPng, pendingFile]
            else
                beamProc.command = ["sh", "-c", "exec \"$1\" panel --qr-file \"$2\"", "sh", dbg, root.qrPng]
            beamProc.running = true
            return true
        }
        return false
    }

    function parseJsonLine(line) {
        line = String(line || "").trim()
        if (!line) return
        if (line.charAt(0) !== "{") {
            console.warn("omabeam: unexpected output:", line)
            return
        }
        var obj = null
        try { obj = JSON.parse(line) } catch (e) {
            console.warn("omabeam: bad JSON:", line, e)
            error = "Could not parse beam output"
            busy = false
            return
        }
        if (!obj || typeof obj !== "object") return
        var k = String(obj.kind || "")
        if (k === "error") {
            error = String(obj.detail || "Unknown error")
            kind = "error"
            detail = error
            busy = false
            return
        }
        kind = k
        detail = String(obj.detail || "")
        url = String(obj.url || "")
        filename = String(obj.filename || "")
        contentType = String(obj.contentType || "")
        error = ""
        revision++
        // Server-backed shares keep beamProc.running (serving) — busy is only
        // the wait for this first JSON line, so clear it here in all cases.
        busy = false
    }

    Process {
        id: beamProc
        stdinEnabled: false
        stdout: SplitParser {
            splitMarker: "\n"
            onRead: function(line) { root.parseJsonLine(line) }
        }
        stderr: SplitParser {
            splitMarker: "\n"
            onRead: function(line) {
                var t = String(line || "").trim()
                if (t) console.warn("omabeam stderr:", t)
            }
        }
        onExited: function(exitCode, exitStatus) {
            if (exitCode === 127 && root.tryFallback(exitCode)) return
            // Intentional stops (panel close, refresh, Stop button) kill a
            // successfully-beamed server: busy is already false, so don't
            // relabel that as a failure. Only a non-zero exit while still
            // waiting for the first JSON line is a real beam failure.
            if (exitCode !== 0 && !root.error && root.busy) {
                root.error = "Beam failed (exit " + exitCode + ")"
                root.kind = "error"
            }
            root.busy = false
        }
    }
}
