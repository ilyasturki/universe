import QtQuick
import "../core"
import "../core/Format.js" as Format
import "../sound"

FocusScope {
    id: dock

    readonly property var session: api.universe.currentSession
    readonly property bool sessionRunning: session != null && session.session_id !== undefined
    readonly property var game: sessionRunning ? api.allGames.byId(session.id) : null
    readonly property bool open: api.home.open
    readonly property bool paused: api.home.paused
    readonly property bool shown: open && !hidden

    property bool hidden: false
    property int index: 0
    property bool opened: false
    property int sub: 0
    property var vals: ({})
    property real elapsed: 0

    focus: true

    readonly property var row: [
        { id: "resume", icon: "play", label: "Resume", kind: "action" },
        { id: "home", icon: "grid", label: "Home", kind: "action" },
        { id: "game", icon: "gamepad", label: "Game", kind: "group", children: [
            { id: "details", icon: "info", label: "Details", kind: "action" },
            { id: "journal", icon: "book", label: "Journal", kind: "action" },
            { id: "recordings", icon: "film", label: "Recordings", kind: "action" },
            { id: "pause", icon: "snowflake", label: "Pause on HOME", kind: "toggle" },
            { id: "quit", icon: "power", label: "Quit", kind: "action", value: "asks first" } ] },
        "|",
        { id: "shot", icon: "camera", label: "Screenshot", kind: "action" },
        { id: "cap", icon: "video", label: "Capture", kind: "group", children: [
            { id: "rec", icon: "record", label: "Recording", kind: "info" },
            { id: "source", icon: "screen", label: "Source", kind: "value", options: ["screen", "window"], names: ["Screen", "Window"], later: true },
            { id: "cursor", icon: "cursor", label: "Cursor", kind: "toggle", later: true } ] },
        { id: "perf", icon: "pulse", label: "Performance", kind: "group", children: [
            { id: "hud", key: "mangohud", icon: "pulse", label: "MangoHud", kind: "toggle" },
            { id: "fps", key: "fps_limit", icon: "gauge", label: "FPS limit", kind: "value", options: [], names: [] },
            { id: "filter", key: "gamescope_filter", icon: "sliders", label: "Filter", kind: "value", options: ["", "linear", "nearest", "fsr", "nis", "pixel"], names: ["Default", "Linear", "Nearest", "FSR", "NIS", "Pixel"] } ] },
        { id: "sound", icon: "volume-up", label: "Sound", kind: "group", children: [
            { id: "vol", icon: "volume-up", label: "Volume", kind: "range" },
            { id: "mute", icon: "mute", label: "Mute", kind: "toggle" } ] }
    ]
    readonly property var buttons: row.filter(function(b) { return b !== "|"; })
    readonly property var current: buttons[Math.max(0, Math.min(buttons.length - 1, index))]
    readonly property var target: opened && current.kind === "group" ? current.children[sub] : current

    // Slot results are not bindings: reread on open, then patched by the change that was just made.
    function refresh() {
        var cap = api.universe.getSettings("capture", session.id) || {};
        var on = api.universe.modules().some(function(m) { return m.id === "capture" && m.enabled; });
        vals = {
            pause: api.home.pauseOnHome,
            rec: on && cap.enabled !== false,
            source: String(cap.source || "screen"),
            cursor: cap.cursor === true,
            hud: api.home.launchValue("mangohud") === "true",
            fps: api.home.launchValue("fps_limit") || "auto",
            fpsOptions: api.home.launchChoices("fps_limit"),
            hz: api.home.screenRefresh(),
            filter: api.home.launchValue("gamescope_filter"),
            vol: api.home.volumePercent,
            mute: api.home.muted
        };
    }

    // A copy: the same object assigned again is no change to the bindings.
    function patch(key, value) {
        var v = Object.assign({}, vals);
        v[key] = value;
        vals = v;
    }

    function options(item) {
        return item.id === "fps" ? (vals.fpsOptions || []) : item.options;
    }

    function shows(item) {
        var v = vals;
        switch (item.id) {
        case "pause": return v.pause ? "On" : "Off";
        case "hud": return v.hud ? "On" : "Off";
        case "rec": return v.rec ? "On · " + Format.clockTime(dock.elapsed) : "Off";
        case "source": return (v.source === "window" ? "Window" : "Screen") + " · next session";
        case "cursor": return (v.cursor ? "On" : "Off") + " · next session";
        case "fps": return v.fps === "auto" ? "Auto · " + (v.hz > 0 ? v.hz : "screen") : v.fps === "none" ? "None" : v.fps;
        case "filter": return item.names[Math.max(0, item.options.indexOf(v.filter))];
        case "vol": return v.vol + "%";
        case "mute": return v.mute ? "On" : "Off";
        }
        return item.value || "";
    }

    function isOn(item) {
        return vals[item.id] === true;
    }

    function step(item, dir) {
        if (item.kind === "range") {
            api.home.volume(dir > 0 ? "up" : "down", 0);
            Sound.tick();
            return;
        }
        if (item.kind !== "value")
            return;
        var opts = options(item);
        if (!opts.length)
            return;
        var next = opts[(Math.max(0, opts.indexOf(vals[item.id])) + dir + opts.length) % opts.length];
        if (item.key)
            api.home.setLaunchValue(item.key, next);
        else
            api.universe.setSetting("capture", session.id, item.id, next);
        Sound.tick();
        patch(item.id, next);
    }

    function flip(item) {
        Sound.enter();
        if (item.id === "pause")
            api.home.setPauseOnHome(!vals.pause);
        else if (item.id === "hud") {
            api.home.setLaunchValue("mangohud", vals.hud ? "false" : "true");
            patch("hud", !vals.hud);
        } else if (item.id === "cursor") {
            api.universe.setSetting("capture", session.id, "cursor", vals.cursor ? "false" : "true");
            patch("cursor", !vals.cursor);
        } else if (item.id === "mute")
            api.home.volume("mute", 0);
    }

    function act(item) {
        switch (item.id) {
        case "resume":
            Sound.cancel();
            api.home.closeDock();
            break;
        case "home":
            Sound.cancel();
            api.home.toLauncher();
            break;
        case "details":
        case "journal":
        case "recordings":
            Sound.enter();
            api.home.toLauncher(item.id);
            break;
        case "shot":
            Sound.enter();
            hidden = true;
            shotTimer.restart();
            break;
        case "quit":
            confirm.ask({ message: "Quit " + (dock.session ? dock.session.title : "the game") + "?",
                          detail: "Unsaved progress will be lost.", yes: "Quit", no: "Keep playing" },
                        function(yes) { if (yes) api.home.stop(); });
            break;
        }
    }

    function select() {
        var t = target;
        if (opened) {
            if (t.kind === "toggle")
                flip(t);
            else if (t.kind === "action") {
                opened = false;
                act(t);
            } else if (t.kind !== "info") {
                Sound.cancel();
                opened = false;
            } else
                Sound.edge();
            return;
        }
        if (t.kind === "group") {
            Sound.enter();
            refresh();
            sub = 0;
            opened = true;
        } else
            act(t);
    }

    function back() {
        Sound.cancel();
        if (opened)
            opened = false;
        else
            api.home.closeDock();
    }

    function openShots() {
        if (hidden)
            return;
        Sound.enter();
        opened = false;
        shots.open = true;
    }

    onOpenChanged: {
        if (open) {
            index = 0;
            opened = false;
            hidden = false;
            shots.open = false;
            refresh();
            api.home.volume("get", 0);
            Sound.panel();
            forceActiveFocus();
        } else
            shots.open = false;
    }

    Connections {
        target: api.home
        function onVolumeChanged() {
            patch("vol", api.home.volumePercent);
            patch("mute", api.home.muted);
        }
        function onChanged() {
            if (dock.open && dock.vals.pause !== api.home.pauseOnHome)
                patch("pause", api.home.pauseOnHome);
        }
        function onScreenshotTaken(path) {
            if (!dock.open)
                return;
            dock.hidden = false;
            toast.show(path ? "Screenshot saved" : "Screenshot failed");
        }
    }

    Timer {
        id: shotTimer
        interval: Theme.durDismiss + 60
        onTriggered: api.home.screenshot()
    }

    Timer {
        running: dock.shown
        interval: 1000
        repeat: true
        triggeredOnStart: true
        onTriggered: {
            var started = dock.session && dock.session.started_at ? Date.parse(dock.session.started_at) : NaN;
            dock.elapsed = isNaN(started) ? 0 : (Date.now() - started) / 1000;
        }
    }

    Item {
        id: band

        anchors.fill: parent
        opacity: dock.shown ? 1.0 : 0.0
        visible: opacity > 0.001

        Behavior on opacity {
            SequentialAnimation {
                NumberAnimation { duration: dock.shown ? Theme.durBase : Theme.durDismiss; easing.type: Easing.OutCubic }
                ScriptAction { script: if (!dock.open) api.home.dockClosed(); }
            }
        }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.60; color: Qt.rgba(0.055, 0.059, 0.075, 0.0) }
                GradientStop { position: 0.82; color: Qt.rgba(0.055, 0.059, 0.075, 0.86) }
                GradientStop { position: 1.00; color: Qt.rgba(0.055, 0.059, 0.075, 0.95) }
            }
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(Theme.edgeMargin)
            anchors.top: parent.top
            anchors.topMargin: Theme.dp(40)
            spacing: Theme.dp(26)

            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(10)
                visible: dock.vals.rec === true

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(13)
                    height: width
                    radius: width / 2
                    color: "#e5484d"

                    SequentialAnimation on opacity {
                        running: dock.shown
                        loops: Animation.Infinite
                        PauseAnimation { duration: 600 }
                        PropertyAction { value: 0.25 }
                        PauseAnimation { duration: 600 }
                        PropertyAction { value: 1.0 }
                    }
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "REC " + Format.clockTime(dock.elapsed)
                    color: "#e5484d"
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(21)
                    font.letterSpacing: Theme.dp(1.2)
                }
            }

            PowerBadge {
                anchors.verticalCenter: parent.verticalCenter
                tint: Qt.rgba(0.949, 0.953, 0.961, 0.85)
                size: Theme.dp(24)
            }

            Item {
                width: clock.width
                height: clock.height
                anchors.verticalCenter: parent.verticalCenter

                Text {
                    x: Theme.dp(1)
                    y: Theme.dp(2)
                    text: clock.text
                    color: Qt.rgba(0, 0, 0, 0.5)
                    font: clock.font
                }

                Text {
                    id: clock
                    text: Theme.clock
                    color: Qt.rgba(0.949, 0.953, 0.961, 0.85)
                    font.family: Theme.sans
                    font.weight: Font.Medium
                    font.pixelSize: Theme.dp(26)
                }
            }
        }

        Row {
            id: card

            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(Theme.edgeMargin)
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.dp(120)
            spacing: Theme.dp(22)

            RoundedMask {
                width: Theme.dp(128)
                height: Theme.dp(128)
                radius: Theme.dp(16)
                anchors.verticalCenter: parent.verticalCenter

                Rectangle {
                    anchors.fill: parent
                    color: Theme.cardBase
                }

                Image {
                    anchors.fill: parent
                    source: dock.game ? (dock.game.assets.square.toString() ? dock.game.assets.square : dock.game.assets.boxFront) : ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    sourceSize.width: 400
                }

                Text {
                    anchors.centerIn: parent
                    visible: !dock.game || (!dock.game.assets.square.toString() && !dock.game.assets.boxFront.toString())
                    text: dock.session && dock.session.title ? dock.session.title.charAt(0) : ""
                    color: Theme.textMuted
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(48)
                }
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(6)

                Row {
                    spacing: Theme.dp(9)

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(10)
                        height: width
                        radius: width / 2
                        color: dock.paused ? "#f2b84b" : "#7ed957"

                        Behavior on color { ColorEase {} }
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: dock.paused ? "PAUSED" : "PLAYING"
                        color: Qt.rgba(0.949, 0.953, 0.961, 0.72)
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.pixelSize: Theme.dp(15)
                        font.letterSpacing: Theme.dp(1.5)
                    }
                }

                Text {
                    width: Theme.dp(300)
                    text: dock.session ? dock.session.title : ""
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(30)
                    font.letterSpacing: -Theme.dp(0.3)
                    lineHeight: 1.1
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                }

                Text {
                    text: {
                        var total = dock.game ? Format.playTime(dock.game.playTime) : "";
                        var now = Format.clockTime(dock.elapsed);
                        var session = dock.elapsed >= 3600 ? now.substring(0, now.length - 3).replace(":", " h ") : Math.max(1, Math.floor(dock.elapsed / 60)) + " min";
                        return (total ? total + " · " : "") + session + " this session";
                    }
                    color: Qt.rgba(0.949, 0.953, 0.961, 0.6)
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(18)
                }
            }
        }

        Row {
            id: buttonsRow

            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(Theme.edgeMargin)
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.dp(120)
            spacing: Theme.dp(18)
            transform: Translate { y: dock.shown ? 0 : Theme.dp(16)
                                   Behavior on y { Ease { duration: dock.shown ? Theme.durBase : Theme.durDismiss } } }

            Repeater {
                model: dock.row

                delegate: Item {
                    id: slot

                    readonly property bool separator: modelData === "|"
                    readonly property int position: dock.row.slice(0, index).filter(function(b) { return b !== "|"; }).length
                    readonly property bool focused: !separator && position === dock.index

                    width: separator ? Theme.dp(17) : Theme.dp(70)
                    height: Theme.dp(70)

                    Rectangle {
                        visible: slot.separator
                        anchors.centerIn: parent
                        width: 1
                        height: Theme.dp(50)
                        color: Theme.surfaceBorder
                    }

                    Rectangle {
                        id: button

                        visible: !slot.separator
                        anchors.fill: parent
                        radius: width / 2
                        color: slot.focused ? Theme.text : Theme.surface
                        border.width: 1
                        border.color: slot.focused ? Theme.text : Theme.surfaceBorder

                        Behavior on color { ColorEase {} }

                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: -Theme.dp(9.7)
                            radius: width / 2
                            color: "transparent"
                            border.width: Theme.dp(2.7)
                            border.color: Qt.rgba(1, 1, 1, 0.95)
                            opacity: slot.focused ? 1.0 : 0.0
                            antialiasing: true

                            Behavior on opacity { Ease { duration: Theme.durQuick } }
                        }

                        MenuGlyph {
                            anchors.centerIn: parent
                            width: Theme.dp(30)
                            height: width
                            kind: slot.separator ? "" : modelData.icon
                            tint: slot.focused ? Theme.onLight : Qt.rgba(0.949, 0.953, 0.961, 0.92)
                        }
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.top
                        anchors.bottomMargin: Theme.dp(16)
                        text: slot.separator ? "" : modelData.label
                        color: Theme.text
                        font.family: Theme.sans
                        font.weight: Font.Medium
                        font.pixelSize: Theme.dp(20)
                        opacity: slot.focused && !dock.opened ? 1.0 : 0.0

                        Behavior on opacity { Ease { duration: Theme.durQuick } }
                    }
                }
            }
        }

        Rectangle {
            id: pop

            readonly property Item anchorButton: buttonsRow.children[dock.row.indexOf(dock.current)] || null
            readonly property real anchorX: anchorButton ? buttonsRow.x + anchorButton.x : 0

            visible: opacity > 0.01
            opacity: dock.opened && dock.current.kind === "group" ? 1.0 : 0.0
            width: Theme.dp(440)
            height: rows.height + Theme.dp(34)
            radius: Theme.dp(24)
            color: "#1b1d24"
            border.width: 1
            border.color: Theme.surfaceBorder
            x: Math.min(anchorX, band.width - Theme.dp(Theme.edgeMargin) - width)
            y: buttonsRow.y - Theme.dp(28) - height + (dock.opened ? 0 : Theme.dp(12))

            Behavior on opacity { Ease {} }
            Behavior on y { Ease {} }

            Column {
                id: rows

                anchors.top: parent.top
                anchors.topMargin: Theme.dp(17)
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.leftMargin: Theme.dp(17)
                anchors.rightMargin: Theme.dp(17)
                spacing: Theme.dp(2)

                Repeater {
                    model: dock.current.kind === "group" ? dock.current.children : []

                    delegate: Rectangle {
                        id: line

                        readonly property bool focused: dock.opened && index === dock.sub
                        readonly property bool checked: modelData.kind === "toggle" && dock.isOn(modelData)
                        readonly property color ink: focused ? Theme.onLight : modelData.id === "quit" ? Qt.rgba(0.949, 0.953, 0.961, 0.7) : Theme.text

                        width: rows.width
                        height: Theme.dp(58)
                        radius: Theme.dp(16)
                        color: focused ? Theme.text : "transparent"

                        Behavior on color { ColorEase {} }

                        MenuGlyph {
                            id: lineGlyph
                            anchors.left: parent.left
                            anchors.leftMargin: Theme.dp(16)
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(25)
                            height: width
                            kind: modelData.icon
                            tint: line.ink
                        }

                        Text {
                            anchors.left: lineGlyph.right
                            anchors.leftMargin: Theme.dp(16)
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.label
                            color: line.ink
                            font.family: Theme.sans
                            font.weight: Font.Medium
                            font.pixelSize: Theme.dp(21)
                        }

                        SettingsToggle {
                            visible: modelData.kind === "toggle"
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(14)
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(50)
                            height: Theme.dp(28)
                            on: line.checked
                            focused: line.focused
                        }

                        Row {
                            visible: modelData.kind === "range"
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(16)
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: Theme.dp(12)

                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: Theme.dp(120)
                                height: Theme.dp(8)
                                radius: height / 2
                                color: line.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.18) : Qt.rgba(1, 1, 1, 0.14)

                                Rectangle {
                                    width: parent.width * Math.max(0, Math.min(100, dock.vals.vol || 0)) / 100
                                    height: parent.height
                                    radius: height / 2
                                    color: line.focused ? Theme.onLight : Theme.text

                                    Behavior on width { Ease { duration: Theme.durQuick } }
                                }
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                width: Theme.dp(46)
                                horizontalAlignment: Text.AlignRight
                                text: (dock.vals.vol || 0) + "%"
                                color: line.ink
                                font.family: Theme.sans
                                font.pixelSize: Theme.dp(19)
                            }
                        }

                        Text {
                            visible: modelData.kind !== "toggle" && modelData.kind !== "range"
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(16)
                            anchors.verticalCenter: parent.verticalCenter
                            text: (modelData.kind === "value" && line.focused ? "◀  " : "") + dock.shows(modelData) + (modelData.kind === "value" && line.focused ? "  ▶" : "")
                            color: line.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.6) : Qt.rgba(0.949, 0.953, 0.961, 0.5)
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(19)
                        }
                    }
                }
            }
        }
    }

    DockShots {
        id: shots
        objectName: "dockShots"
        width: parent.width
        height: parent.height
        z: 2
        session: dock.session
        onCloseRequested: {
            open = false;
            dock.forceActiveFocus();
        }
    }

    ConfirmDialog {
        id: confirm
        anchors.fill: parent
        z: 3
        onClosed: dock.forceActiveFocus()
    }

    Toast {
        id: toast
        z: 5
    }

    Keys.onPressed: function(event) {
        if (!dock.open || confirm.open) {
            event.accepted = dock.open;
            return;
        }
        event.accepted = true;
        if (event.isAutoRepeat && event.key !== Qt.Key_Left && event.key !== Qt.Key_Right)
            return;
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            var dir = event.key === Qt.Key_Left ? -1 : 1;
            if (opened) {
                step(target, dir);
                return;
            }
            index = Sound.stepped(index, dir, buttons.length);
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            if (opened && current.kind === "group")
                sub = Sound.stepped(sub, event.key === Qt.Key_Up ? -1 : 1, current.children.length);
            else if (event.key === Qt.Key_Down)
                openShots();
            else
                Sound.edge();
        } else if (api.keys.isAccept(event)) {
            select();
        } else if (api.keys.isCancel(event)) {
            back();
        } else if (api.keys.isDetails(event)) {
            if (!hidden)
                act(buttons[3]);
        } else
            event.accepted = false;
    }
}
