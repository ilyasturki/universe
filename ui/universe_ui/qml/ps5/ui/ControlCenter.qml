import QtQuick
import "../core"
import "../sound"
import "../../core" as Base
import "../../core/Format.js" as Format
import "../../ui" as Reprise
import "../../ui/Controls.js" as Controls
import "../pages/Home.js" as Home

// The Control Center over the running game (the overlay window's look): the game's cards in a row,
// the icon bar under them, a panel over the bar for each icon, a card grown to fill for trophies and captures.
FocusScope {
    id: cc

    // A launch the core has not made a session of yet stands in: its id and title alone.
    readonly property var session: api.universe.currentSession || api.home.pending
    readonly property var game: session ? api.allGames.byId(session.id) : null
    readonly property bool open: api.home.open
    readonly property bool paused: api.home.paused
    readonly property bool loading: api.home.loading
    property bool hidden: false
    readonly property bool shown: open && !hidden

    // "cards", "bar", "panel" or "sheet"
    property string zone: "cards"
    property int card: 0
    property int icon: 0
    property int row: 0
    // The card grown to fill: "trophies" or "captures".
    property string sheet: ""
    property bool viewing: false
    property int shot: 0
    property var vals: ({})
    property real elapsed: 0

    focus: true

    // The source's switch (GOG's `achievements`), read on open: off, the game keeps its list but the dock shows none.
    property bool tracksAchievements: true
    readonly property bool listsAchievements: tracksAchievements && game !== null && game.achievementsTotal > 0
    readonly property var shots: api.screens.shots.rows
    readonly property var trophyRows: api.screens.dockAchievements.rows
    property var nextUp: null

    readonly property var cards: {
        if (!session)
            return [];
        var out = [
            {
                id: "game",
                caption: loading ? "Loading" : paused ? "Paused" : "Playing",
                title: session.title || "",
                body: loading ? "" : "For " + Format.clockTime(elapsed),
                image: game ? Home.art(game).source : ""
            }
        ];
        if (loading)
            return out;
        out.push({
            id: "hub",
            badge: "grid",
            caption: "Game Hub",
            title: "Details and more"
        });
        if (listsAchievements)
            out.push({
                id: "trophies",
                badge: "trophy",
                caption: "Earned " + game.achievementsUnlocked + "/" + game.achievementsTotal,
                title: "Trophies",
                progress: game.achievementsUnlocked / game.achievementsTotal
            });
        out.push({
            id: "captures",
            badge: "capture",
            caption: shots.length > 0 ? "Recently created" : "Media Gallery",
            title: shots.length > 0 ? "New screenshot" : "No screenshots yet",
            image: shots.length > 0 ? shots[0].url : ""
        });
        if (nextUp)
            out.push({
                id: "journal",
                badge: "journal",
                caption: "Next up",
                title: nextUp.title || "From your journal",
                body: nextUp.next_up
            });
        return out;
    }

    readonly property var homeIcon: ({
            id: "home",
            glyph: "home",
            label: "Home"
        })
    readonly property var powerIcon: ({
            id: "power",
            glyph: "power",
            label: "Power",
            panel: true
        })
    readonly property var icons: loading ? [homeIcon, powerIcon] : [homeIcon,
        {
            id: "game",
            glyph: "gamepad",
            label: session ? session.title : "Game",
            panel: true
        },
        {
            id: "shot",
            glyph: "capture",
            label: "Screenshot"
        }
    ].concat(listsAchievements ? [
        {
            id: "trophies",
            glyph: "trophy",
            label: "Trophies"
        }
    ] : []).concat([
        {
            id: "perf",
            glyph: "pulse",
            label: "Performance",
            panel: true
        },
        {
            id: "sound",
            glyph: "sound",
            label: "Sound",
            panel: true
        }
    ]).concat(api.system.controls.length > 0 ? [
        {
            id: "system",
            glyph: "bolt",
            label: "System",
            panel: true
        }
    ] : []).concat([powerIcon])
    readonly property var current: icons[Math.max(0, Math.min(icon, icons.length - 1))]

    // Gamescope sharpens only through FSR and NIS: the row comes with them.
    readonly property bool sharpens: vals.filter === "fsr" || vals.filter === "nis"

    readonly property var panelRows: {
        var id = current ? current.id : "";
        if (id === "game")
            return [
                {
                    id: "resume",
                    glyph: "play",
                    label: "Resume",
                    kind: "action"
                },
                {
                    id: "details",
                    glyph: "info",
                    label: "Game Hub",
                    kind: "action"
                },
                {
                    id: "pause",
                    glyph: "snowflake",
                    label: "Pause on HOME",
                    kind: "toggle"
                },
                {
                    id: "quit",
                    glyph: "stop",
                    label: "Close Game",
                    kind: "action"
                }
            ];
        if (id === "perf")
            return [
                {
                    id: "hud",
                    key: "mangohud",
                    glyph: "pulse",
                    label: "MangoHud",
                    kind: "toggle"
                },
                {
                    id: "fps",
                    key: "fps_limit",
                    glyph: "gauge",
                    label: "FPS limit",
                    kind: "value",
                    options: [],
                    names: []
                },
                {
                    id: "filter",
                    key: "gamescope_filter",
                    glyph: "sliders",
                    label: "Filter",
                    kind: "value",
                    options: ["", "linear", "nearest", "fsr", "nis", "pixel"],
                    names: ["Default", "Linear", "Nearest", "FSR", "NIS", "Pixel"]
                }
            ].concat(sharpens ? [
                {
                    id: "sharp",
                    key: "gamescope_sharpness",
                    glyph: "sun",
                    label: "Sharpness",
                    kind: "value",
                    options: ["", "0", "2", "5", "10", "15", "20"],
                    names: ["Default", "0 · sharpest", "2", "5", "10", "15", "20 · softest"]
                }
            ] : []);
        if (id === "sound")
            return [
                {
                    id: "vol",
                    glyph: "sound",
                    label: "Volume",
                    kind: "range"
                },
                {
                    id: "output",
                    glyph: "headphones",
                    label: "Output",
                    kind: "value"
                }
            ];
        if (id === "system")
            return api.system.controls.map(function (c) {
                return {
                    id: "sys_" + c.id,
                    sys: c.id,
                    glyph: "sliders",
                    label: c.label,
                    kind: c.kind === "toggle" ? "toggle" : "value"
                };
            });
        if (id === "power") {
            var can = api.system.actions;
            var out = [
                {
                    id: "quit",
                    glyph: "stop",
                    label: "Close Game",
                    kind: "action"
                }
            ];
            if (can.indexOf("suspend") >= 0)
                out.push({
                    id: "suspend",
                    glyph: "power",
                    label: "Enter Rest Mode",
                    kind: "action"
                });
            if (can.indexOf("power_off") >= 0)
                out.push({
                    id: "power_off",
                    glyph: "ring",
                    label: "Turn Off",
                    kind: "action"
                });
            if (can.indexOf("reboot") >= 0)
                out.push({
                    id: "reboot",
                    glyph: "restart",
                    label: "Restart",
                    kind: "action"
                });
            return out;
        }
        return [];
    }
    readonly property var target: panelRows[Math.max(0, Math.min(row, panelRows.length - 1))] || null

    // Slot results are not bindings: reread on open, then patched by the change that was just made.
    function refresh() {
        tracksAchievements = !game || (api.universe.sourceSettingsOf(game.source, game.id) || {}).achievements !== false;
        var v = {
            pause: api.home.pauseOnHome,
            hud: api.home.launchValue("mangohud") === "true",
            fps: api.home.launchValue("fps_limit") || "auto",
            fpsOptions: api.home.launchChoices("fps_limit"),
            hz: api.home.screenRefresh(),
            filter: api.home.launchValue("gamescope_filter"),
            sharp: api.home.launchValue("gamescope_sharpness"),
            vol: api.home.volumePercent,
            mute: api.home.muted,
            output: outputApply.running ? vals.output : currentOutput()
        };
        api.system.controls.forEach(function (c) {
            v["sys_" + c.id] = systemApply.running ? vals["sys_" + c.id] : c.value;
        });
        vals = v;
        var cap = api.universe.getSettings("capture", session ? session.id : "") || {};
        var on = api.universe.modules().some(function (m) {
            return m.id === "capture" && m.enabled;
        });
        recording = on && cap.enabled !== false;
        nextUp = session && session.id ? (api.universe.journal(session.id) || []).filter(function (e) {
            return e.next_up;
        })[0] || null : null;
    }

    property bool recording: false

    function currentOutput() {
        var on = api.home.outputs.filter(function (o) {
            return o.current;
        })[0];
        return on ? on.id : "";
    }

    // Two cards can both call theirs "HDMI / DisplayPort": the device tells them apart.
    function outputName(id) {
        var outs = api.home.outputs;
        var o = outs.filter(function (x) {
            return x.id === id;
        })[0];
        if (!o)
            return "None";
        var twin = outs.some(function (x) {
            return x.id !== o.id && x.label === o.label;
        });
        return twin && o.device ? o.label + " · " + o.device : o.label;
    }

    // A copy: the same object assigned again is no change to the bindings.
    function patch(key, value) {
        var v = Object.assign({}, vals);
        v[key] = value;
        vals = v;
    }

    function options(item) {
        if (item.sys)
            return Controls.values(api.system.control(item.sys));
        if (item.id === "output")
            return api.home.outputs.map(function (o) {
                return o.id;
            });
        return item.id === "fps" ? (vals.fpsOptions || []) : item.options;
    }

    function shows(item) {
        var v = vals;
        if (item.sys) {
            var control = api.system.control(item.sys);
            return control ? Controls.label(control, v[item.id]) : "";
        }
        switch (item.id) {
        case "fps":
            return v.fps === "auto" ? "Auto · " + (v.hz > 0 ? v.hz : "screen") : v.fps === "none" ? "None" : v.fps;
        case "filter":
        case "sharp":
            return item.names[Math.max(0, item.options.indexOf(v[item.id]))];
        case "vol":
            return v.mute ? "Muted" : v.vol + "%";
        case "output":
            return outputName(v.output);
        }
        return "";
    }

    function isOn(item) {
        return item.sys ? vals[item.id] === "on" : vals[item.id] === true;
    }

    function step(item, dir) {
        if (item.kind === "range") {
            api.home.volume(dir > 0 ? "up" : "down", 0);
            Sound.play("tick");
            return;
        }
        if (item.kind !== "value") {
            Sound.play("edge");
            return;
        }
        // Held at either end, not wrapped: from the highest power limit a step up is no jump to the lowest.
        if (item.sys) {
            var was = vals[item.id];
            var to = Controls.stepped(api.system.control(item.sys), was, dir);
            if (to === was) {
                Sound.play("edge");
                return;
            }
            Sound.play("tick");
            patch(item.id, to);
            systemApply.restart();
            return;
        }
        var opts = options(item);
        if (!opts.length)
            return;
        var next = opts[(Math.max(0, opts.indexOf(vals[item.id])) + dir + opts.length) % opts.length];
        if (item.id === "output")
            outputApply.restart();
        else
            api.home.setLaunchValue(item.key, next);
        Sound.play("tick");
        patch(item.id, next);
    }

    function flip(item) {
        Sound.play("select");
        if (item.sys) {
            var on = vals[item.id] !== "on";
            patch(item.id, on ? "on" : "off");
            api.system.set(item.sys, on ? "on" : "off");
        } else if (item.id === "pause")
            api.home.setPauseOnHome(!vals.pause);
        else if (item.id === "hud") {
            api.home.setLaunchValue("mangohud", vals.hud ? "false" : "true");
            patch("hud", !vals.hud);
        } else if (item.id === "vol")
            api.home.volume("mute", 0);
    }

    function close() {
        Sound.play("back");
        api.home.closeDock();
    }

    function act(id) {
        switch (id) {
        case "resume":
            close();
            break;
        case "home":
            Sound.play("home");
            api.home.toLauncher();
            break;
        case "details":
        case "hub":
            Sound.play("ok");
            api.home.toLauncher("details");
            break;
        case "journal":
            Sound.play("ok");
            api.home.toLauncher("journal");
            break;
        case "shot":
            Sound.play("ok");
            hidden = true;
            shotTimer.restart();
            break;
        case "trophies":
        case "captures":
            openSheet(id);
            break;
        case "quit":
            confirm.show({
                message: "Close " + (cc.session ? cc.session.title : "the game") + "?",
                detail: "Unsaved progress will be lost.",
                buttons: ["Keep Playing", "Close Game"],
                danger: 1,
                index: 0
            }, function (i) {
                cc.forceActiveFocus();
                if (i === 1)
                    api.home.stop();
            });
            break;
        case "suspend":
            Sound.play("ok");
            Base.Notices.show("Entering rest mode…", "power");
            api.system.run("suspend");
            break;
        case "power_off":
        case "reboot":
            var restart = id === "reboot";
            confirm.show({
                message: restart ? "Restart the machine?" : "Turn off the machine?",
                detail: (cc.session ? cc.session.title + " will be closed. " : "") + "Unsaved progress will be lost.",
                buttons: ["Cancel", restart ? "Restart" : "Turn Off"],
                danger: 1,
                index: 0
            }, function (i) {
                cc.forceActiveFocus();
                if (i !== 1)
                    return;
                Base.Notices.show(restart ? "Restarting…" : "Turning off…", "power");
                api.system.run(id);
            });
            break;
        }
    }

    function openSheet(which) {
        if (hidden || loading)
            return;
        Sound.play("open");
        sheet = which;
        shot = 0;
        viewing = false;
        if (which === "trophies" && session)
            api.screens.dockAchievements.load(session.id);
        else if (session)
            api.screens.shots.load(session.id);
        var i = cards.map(function (c) {
            return c.id;
        }).indexOf(which);
        sheetFrom = i >= 0 ? cardRect(i) : Qt.rect(width / 2, height / 2, 0, 0);
        zone = "sheet";
    }

    function closeSheet() {
        Sound.play("back");
        zone = "cards";
        sheet = "";
        viewing = false;
    }

    function select() {
        if (zone === "cards") {
            var c = cards[card];
            if (c)
                act(c.id === "game" ? "resume" : c.id);
            return;
        }
        if (zone === "bar") {
            if (current.panel) {
                Sound.play("open");
                refresh();
                row = 0;
                zone = "panel";
            } else
                act(current.id);
            return;
        }
        if (zone === "panel" && target) {
            if (target.kind === "toggle" || target.kind === "range")
                flip(target);
            else if (target.kind === "action") {
                zone = "bar";
                act(target.id);
            } else
                Sound.play("edge");
        }
    }

    // ---- The row of cards: the focused one grows upward, the rest reflow ----

    readonly property real cardW: Theme.dp(358)
    readonly property real cardH: Theme.dp(398)
    readonly property real bigW: Theme.dp(440)
    readonly property real bigH: Theme.dp(528)
    readonly property real cardGap: Theme.dp(17)
    readonly property real cardFloor: Theme.dp(888)
    readonly property real rowX: Theme.dp(80)
    property rect sheetFrom: Qt.rect(0, 0, 0, 0)

    function grown(i) {
        return zone === "cards" && i === card;
    }

    function cardX(i) {
        var x = rowX;
        for (var k = 0; k < i; k++)
            x += (grown(k) ? bigW : cardW) + cardGap;
        return x;
    }

    function cardRect(i) {
        var w = grown(i) ? bigW : cardW, h = grown(i) ? bigH : cardH;
        return Qt.rect(cardX(i), cardFloor - h, w, h);
    }

    Timer {
        id: shotTimer
        interval: Theme.durClose + 60
        onTriggered: api.home.screenshot()
    }

    // Stepping through the outputs switches once the cursor rests: each switch can change a card's profile.
    Timer {
        id: outputApply
        interval: 500
        onTriggered: {
            if (cc.vals.output !== cc.currentOutput())
                api.home.setOutput(cc.vals.output);
        }
    }

    // A power limit or a clock is written once the cursor rests: each write may go through SteamOS's helper.
    Timer {
        id: systemApply
        interval: 400
        onTriggered: {
            api.system.controls.forEach(function (c) {
                var want = cc.vals["sys_" + c.id];
                if (want !== undefined && want !== c.value)
                    api.system.set(c.id, want);
            });
        }
    }

    Timer {
        running: cc.shown
        interval: 1000
        repeat: true
        triggeredOnStart: true
        onTriggered: {
            var started = cc.session && cc.session.started_at ? Date.parse(cc.session.started_at) : NaN;
            cc.elapsed = isNaN(started) ? 0 : (Date.now() - started) / 1000;
        }
    }

    onOpenChanged: {
        if (open) {
            zone = "cards";
            card = 0;
            icon = 0;
            row = 0;
            sheet = "";
            viewing = false;
            hidden = false;
            refresh();
            if (session && session.id)
                api.screens.shots.load(session.id);
            api.home.volume("get", 0);
            api.home.loadOutputs();
            Sound.play("panel");
            entrance.restart();
            forceActiveFocus();
        } else {
            entrance.stop();
            api.screens.dockAchievements.unload();
        }
    }

    // The row shrinks or grows in place: the cursor goes back to the first card.
    onLoadingChanged: {
        card = 0;
        icon = 0;
        if (zone === "panel" || zone === "sheet")
            zone = "cards";
    }

    Connections {
        target: api.home
        function onVolumeChanged() {
            cc.patch("vol", api.home.volumePercent);
            cc.patch("mute", api.home.muted);
        }
        function onOutputsChanged() {
            if (!outputApply.running)
                cc.patch("output", cc.currentOutput());
        }
        function onChanged() {
            if (cc.open && cc.vals.pause !== api.home.pauseOnHome)
                cc.patch("pause", api.home.pauseOnHome);
        }
        function onScreenshotTaken(path) {
            if (!cc.open)
                return;
            cc.hidden = false;
            if (path) {
                Base.Notices.show("Screenshot saved", "shot");
                if (cc.session && cc.session.id)
                    api.screens.shots.load(cc.session.id);
            } else
                Base.Notices.fail("Screenshot failed", "shot");
        }
    }

    // ---- The picture ----

    // One clock from the open: the dim and the first two cards at once, then a card every 230 ms, each fading in 100 ms.
    property real clock: 0
    readonly property real dimIn: Math.min(1, clock / Theme.durDim)

    function appearOf(i) {
        var delay = i < 2 ? 0 : (i - 1) * Theme.cardStagger;
        return Math.max(0, Math.min(1, (clock - delay) / Theme.durCard));
    }

    NumberAnimation {
        id: entrance
        target: cc
        property: "clock"
        from: 0
        to: 2000
        duration: 2000
    }

    Item {
        id: band

        anchors.fill: parent
        opacity: cc.shown ? 1.0 : 0.0
        visible: opacity > 0.001

        Behavior on opacity {
            SequentialAnimation {
                NumberAnimation {
                    duration: cc.shown ? Theme.durDim : Theme.durClose
                    easing.type: Easing.OutCubic
                }
                ScriptAction {
                    script: if (!cc.open)
                        api.home.dockClosed()
                }
            }
        }

        // A tap on the game above, or a right click anywhere, is B.
        TapHandler {
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onTapped: {
                if (cc.zone === "sheet")
                    cc.closeSheet();
                else
                    cc.close();
            }
        }

        Rectangle {
            anchors.fill: parent
            opacity: cc.dimIn
            gradient: Gradient {
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(0, 0, 0, 0)
                }
                GradientStop {
                    position: 0.4
                    color: Qt.rgba(0, 0, 0, 0.12)
                }
                GradientStop {
                    position: 0.78
                    color: Qt.rgba(0.02, 0.02, 0.03, 0.78)
                }
                GradientStop {
                    position: 1.0
                    color: Qt.rgba(0.02, 0.02, 0.03, 0.92)
                }
            }
        }

        Row {
            id: status
            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(87)
            y: Theme.dp(Theme.barY) - height / 2
            spacing: Theme.dp(30)
            opacity: cc.dimIn

            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(10)
                visible: cc.recording

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(12)
                    height: width
                    radius: width / 2
                    color: "#ef5a5a"
                }

                Label {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "REC " + Format.clockTime(cc.elapsed)
                    color: "#ef5a5a"
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(Theme.fontTiny)
                }
            }

            Reprise.PowerBadge {
                anchors.verticalCenter: parent.verticalCenter
                tint: Qt.rgba(1, 1, 1, 0.85)
                size: Theme.dp(26)
                fontFamily: Theme.sans
                fontWeight: Font.Normal
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: Theme.clock
                font.weight: Font.Light
                font.pixelSize: Theme.dp(Theme.fontClock)
            }
        }

        Item {
            id: cardRow
            anchors.fill: parent
            opacity: cc.zone === "sheet" ? 0.0 : 1.0
            visible: opacity > 0.01

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.durCard * 2
                }
            }

            Repeater {
                model: cc.cards

                HubCard {
                    readonly property rect at: cc.cardRect(index)

                    x: at.x
                    y: at.y
                    width: at.width
                    height: at.height
                    opacity: cc.appearOf(index)
                    image: modelData.image || ""
                    badge: modelData.badge || ""
                    caption: modelData.caption || ""
                    title: modelData.title || ""
                    body: modelData.body || ""
                    progress: modelData.progress !== undefined ? modelData.progress : -1
                    focused: cc.zone === "cards" && index === cc.card && cc.activeFocus
                    onPicked: {
                        if (cc.zone === "cards" && cc.card === index) {
                            cc.select();
                            return;
                        }
                        Sound.play("tick");
                        cc.zone = "cards";
                        cc.card = index;
                    }

                    Behavior on x {
                        NumberAnimation {
                            duration: Theme.durReflow
                            easing.type: Easing.OutCubic
                        }
                    }
                    Behavior on y {
                        NumberAnimation {
                            duration: Theme.durReflow
                            easing.type: Easing.OutCubic
                        }
                    }
                    Behavior on width {
                        NumberAnimation {
                            duration: Theme.durReflow
                            easing.type: Easing.OutCubic
                        }
                    }
                    Behavior on height {
                        NumberAnimation {
                            duration: Theme.durReflow
                            easing.type: Easing.OutCubic
                        }
                    }
                }
            }
        }

        // The icon bar at the very bottom; the focused icon on a white disc, its name over it.
        Row {
            id: bar

            readonly property real pitch: Theme.dp(112)

            anchors.horizontalCenter: parent.horizontalCenter
            y: Theme.dp(1012) - height / 2
            opacity: cc.dimIn
            spacing: 0

            Repeater {
                model: cc.icons

                Item {
                    id: slot

                    readonly property bool focused: (cc.zone === "bar" || cc.zone === "panel") && index === cc.icon

                    width: bar.pitch
                    height: Theme.dp(60)

                    Rectangle {
                        anchors.centerIn: parent
                        width: Theme.dp(58)
                        height: width
                        radius: width / 2
                        color: "#ffffff"
                        opacity: slot.focused ? 1.0 : 0.0
                        scale: slot.focused ? 1.0 : 0.8

                        Behavior on opacity {
                            NumberAnimation {
                                duration: Theme.durQuick
                            }
                        }
                        Behavior on scale {
                            NumberAnimation {
                                duration: Theme.durQuick
                                easing.type: Easing.OutCubic
                            }
                        }
                    }

                    TileArt {
                        anchors.centerIn: parent
                        visible: modelData.id === "game" && cc.game !== null
                        width: Theme.dp(slot.focused ? 46 : 40)
                        height: width
                        game: cc.game
                        radius: Theme.dp(6)
                    }

                    Glyph {
                        anchors.centerIn: parent
                        visible: !(modelData.id === "game" && cc.game !== null)
                        width: Theme.dp(34)
                        height: width
                        kind: modelData.glyph
                        tint: slot.focused ? Theme.onLight : Theme.text
                    }

                    Label {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.top
                        anchors.bottomMargin: Theme.dp(6)
                        text: modelData.label
                        opacity: slot.focused && cc.zone === "bar" ? 1.0 : 0.0
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                        width: Math.min(implicitWidth, Theme.dp(360))
                        elide: Text.ElideRight

                        Behavior on opacity {
                            NumberAnimation {
                                duration: Theme.durQuick
                            }
                        }
                    }

                    Touch {
                        current: slot.focused
                        onPicked: {
                            Sound.play("tick");
                            cc.zone = "bar";
                            cc.icon = index;
                        }
                    }
                }
            }
        }

        // An icon's panel, over the bar above the icon.
        Rectangle {
            id: panel

            readonly property Item anchorIcon: bar.children[cc.icon] || null
            readonly property real anchorX: anchorIcon ? bar.x + anchorIcon.x + anchorIcon.width / 2 : parent.width / 2

            visible: opacity > 0.01
            opacity: cc.zone === "panel" ? 1.0 : 0.0
            width: Theme.dp(600)
            height: rowsColumn.height + Theme.dp(32)
            radius: Theme.dp(Theme.radiusCard + 2)
            color: Qt.rgba(0.09, 0.1, 0.13, 0.97)
            border.width: 1
            border.color: Theme.glassEdge
            x: Math.max(Theme.dp(40), Math.min(anchorX - width / 2, parent.width - width - Theme.dp(40)))
            y: bar.y - Theme.dp(34) - height + (cc.zone === "panel" ? 0 : Theme.dp(14))

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.durChrome
                }
            }
            Behavior on y {
                NumberAnimation {
                    duration: Theme.durChrome
                    easing.type: Easing.OutCubic
                }
            }

            TapHandler {}

            Column {
                id: rowsColumn
                x: Theme.dp(16)
                y: Theme.dp(16)
                width: parent.width - Theme.dp(32)
                spacing: Theme.dp(2)

                Label {
                    x: Theme.dp(12)
                    height: Theme.dp(52)
                    verticalAlignment: Text.AlignVCenter
                    text: cc.current ? cc.current.label : ""
                    color: Theme.textSecondary
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }

                Repeater {
                    model: cc.zone === "panel" ? cc.panelRows : []

                    Item {
                        id: line

                        readonly property bool focused: index === cc.row
                        readonly property bool checked: modelData.kind === "toggle" && cc.isOn(modelData)

                        width: rowsColumn.width
                        height: Theme.dp(70)

                        Rectangle {
                            anchors.fill: parent
                            radius: Theme.dp(Theme.radiusRow)
                            color: line.focused ? Theme.focusFill : "transparent"
                            border.width: line.focused ? Theme.dp(Theme.ringLine) : 0
                            border.color: Theme.ringSoft
                        }

                        Glyph {
                            id: lineGlyph
                            x: Theme.dp(18)
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(30)
                            height: width
                            kind: modelData.glyph
                        }

                        Label {
                            anchors.left: lineGlyph.right
                            anchors.leftMargin: Theme.dp(18)
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.label
                        }

                        Toggle {
                            visible: modelData.kind === "toggle"
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(18)
                            anchors.verticalCenter: parent.verticalCenter
                            on: line.checked
                        }

                        Row {
                            visible: modelData.kind === "range"
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(18)
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: Theme.dp(14)

                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: Theme.dp(170)
                                height: Theme.dp(6)
                                radius: height / 2
                                color: Qt.rgba(1, 1, 1, 0.18)

                                Rectangle {
                                    width: cc.vals.mute ? 0 : parent.width * Math.max(0, Math.min(100, cc.vals.vol || 0)) / 100
                                    height: parent.height
                                    radius: height / 2
                                    color: Theme.text

                                    Behavior on width {
                                        NumberAnimation {
                                            duration: Theme.durQuick
                                        }
                                    }
                                }
                            }

                            Label {
                                anchors.verticalCenter: parent.verticalCenter
                                width: Theme.dp(80)
                                horizontalAlignment: Text.AlignRight
                                text: cc.shows(modelData)
                                color: Theme.textSecondary
                                font.pixelSize: Theme.dp(Theme.fontSmall)
                            }
                        }

                        Label {
                            visible: modelData.kind === "value"
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(18)
                            anchors.verticalCenter: parent.verticalCenter
                            text: (line.focused ? "‹  " : "") + cc.shows(modelData) + (line.focused ? "  ›" : "")
                            color: Theme.textSecondary
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }

                        Touch {
                            direct: true
                            onPicked: cc.row = index
                        }
                    }
                }
            }
        }

        // A card grown to fill the row's place: this game's trophies, or its captures.
        Rectangle {
            id: sheetCard

            readonly property bool out: cc.zone === "sheet"
            readonly property rect full: Qt.rect(cc.rowX, Theme.dp(150), cc.width - cc.rowX * 2, Theme.dp(760))

            x: out ? full.x : cc.sheetFrom.x
            y: out ? full.y : cc.sheetFrom.y
            width: out ? full.width : cc.sheetFrom.width
            height: out ? full.height : cc.sheetFrom.height
            radius: Theme.dp(Theme.radiusCard + 2)
            color: Qt.rgba(0.08, 0.09, 0.12, 0.97)
            border.width: 1
            border.color: Theme.glassEdge
            opacity: out ? 1.0 : 0.0
            visible: opacity > 0.01
            clip: true

            Behavior on x {
                NumberAnimation {
                    duration: 250
                    easing.type: Easing.OutCubic
                }
            }
            Behavior on y {
                NumberAnimation {
                    duration: 250
                    easing.type: Easing.OutCubic
                }
            }
            Behavior on width {
                NumberAnimation {
                    duration: 250
                    easing.type: Easing.OutCubic
                }
            }
            Behavior on height {
                NumberAnimation {
                    duration: 250
                    easing.type: Easing.OutCubic
                }
            }
            Behavior on opacity {
                NumberAnimation {
                    duration: 120
                }
            }

            TapHandler {}

            Item {
                anchors.fill: parent
                anchors.margins: Theme.dp(32)
                opacity: sheetCard.out && sheetCard.width > sheetCard.full.width * 0.9 ? 1.0 : 0.0

                Behavior on opacity {
                    NumberAnimation {
                        duration: 150
                    }
                }

                Row {
                    id: sheetHead
                    spacing: Theme.dp(16)

                    Glyph {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(36)
                        height: width
                        kind: cc.sheet === "trophies" ? "trophy" : "capture"
                    }

                    Label {
                        anchors.verticalCenter: parent.verticalCenter
                        text: cc.sheet === "trophies" ? "Trophies" : "Captures"
                        font.weight: Font.Light
                        font.pixelSize: Theme.dp(Theme.fontTitle + 6)
                    }

                    Label {
                        anchors.verticalCenter: parent.verticalCenter
                        text: cc.sheet === "trophies" ? api.screens.dockAchievements.unlocked + "/" + api.screens.dockAchievements.total : Format.plural(cc.shots.length, "screenshot", "screenshots")
                        color: Theme.textSecondary
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }
                }

                ListView {
                    id: trophyList

                    anchors.top: sheetHead.bottom
                    anchors.topMargin: Theme.dp(24)
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    visible: cc.sheet === "trophies"
                    model: cc.sheet === "trophies" ? cc.trophyRows : []
                    currentIndex: cc.shot
                    clip: true
                    interactive: false
                    highlightRangeMode: ListView.ApplyRange
                    preferredHighlightBegin: 0
                    preferredHighlightEnd: height
                    highlightMoveDuration: Theme.durScroll

                    delegate: Item {
                        id: trophy

                        readonly property bool focused: index === cc.shot

                        width: trophyList.width
                        height: Theme.dp(96)

                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: Theme.dp(2)
                            radius: Theme.dp(Theme.radiusRow)
                            color: trophy.focused ? Theme.focusFill : "transparent"
                            border.width: trophy.focused ? Theme.dp(Theme.ringLine) : 0
                            border.color: Theme.ringSoft
                        }

                        Reprise.AchievementBadge {
                            id: badge
                            x: Theme.dp(14)
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(64)
                            height: width
                            icon: modelData.icon
                            opacity: modelData.unlocked ? 1.0 : 0.45
                        }

                        Column {
                            anchors.left: badge.right
                            anchors.leftMargin: Theme.dp(20)
                            anchors.right: meta.left
                            anchors.rightMargin: Theme.dp(20)
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: Theme.dp(4)

                            Label {
                                width: parent.width
                                text: modelData.name
                                elide: Text.ElideRight
                                color: modelData.unlocked ? Theme.text : Theme.textSecondary
                            }

                            Label {
                                width: parent.width
                                text: modelData.description
                                elide: Text.ElideRight
                                color: Theme.textMuted
                                font.pixelSize: Theme.dp(Theme.fontSmall)
                            }
                        }

                        Column {
                            id: meta
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(20)
                            anchors.verticalCenter: parent.verticalCenter

                            Label {
                                anchors.right: parent.right
                                text: modelData.unlocked ? modelData.dateText : "Locked"
                                color: Theme.textSecondary
                                font.pixelSize: Theme.dp(Theme.fontSmall)
                            }

                            Label {
                                anchors.right: parent.right
                                visible: modelData.rarityText !== ""
                                text: modelData.rarityText
                                color: Theme.textMuted
                                font.pixelSize: Theme.dp(Theme.fontTiny)
                            }
                        }
                    }
                }

                GridView {
                    id: shotGrid

                    readonly property int columns: 4

                    anchors.top: sheetHead.bottom
                    anchors.topMargin: Theme.dp(24)
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    visible: cc.sheet === "captures"
                    model: cc.sheet === "captures" ? cc.shots : []
                    cellWidth: width / columns
                    cellHeight: cellWidth * 9 / 16 + Theme.dp(20)
                    currentIndex: cc.shot
                    clip: true
                    interactive: false
                    highlightRangeMode: GridView.ApplyRange
                    preferredHighlightBegin: 0
                    preferredHighlightEnd: height
                    highlightMoveDuration: Theme.durScroll

                    delegate: Item {
                        width: shotGrid.cellWidth
                        height: shotGrid.cellHeight

                        HubCard {
                            anchors.fill: parent
                            anchors.margins: Theme.dp(10)
                            anchors.bottomMargin: Theme.dp(20)
                            image: modelData.url
                            caption: modelData.dateText
                            focused: index === cc.shot && !cc.viewing
                            onPicked: {
                                if (cc.shot === index) {
                                    cc.viewing = true;
                                    return;
                                }
                                Sound.play("tick");
                                cc.shot = index;
                            }
                        }
                    }
                }

                Label {
                    anchors.centerIn: parent
                    visible: cc.sheet === "captures" ? cc.shots.length === 0 : cc.trophyRows.length === 0
                    text: cc.sheet === "captures" ? "No screenshots of this game yet. X takes one." : api.screens.dockAchievements.loading ? "Loading…" : "No trophies listed."
                    color: Theme.textSecondary
                }
            }
        }

        // A screenshot looked at: fit to the screen over the dim.
        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0, 0, 0, 0.92)
            opacity: cc.viewing ? 1.0 : 0.0
            visible: opacity > 0.01

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.durChrome
                }
            }

            Image {
                anchors.fill: parent
                anchors.margins: Theme.dp(60)
                source: cc.viewing && cc.shots[cc.shot] ? cc.shots[cc.shot].url : ""
                fillMode: Image.PreserveAspectFit
                asynchronous: true
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: Theme.dp(28)
                text: cc.shots[cc.shot] ? cc.shots[cc.shot].dateText + "  ·  " + (cc.shot + 1) + "/" + cc.shots.length : ""
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }

    Toast {
        y: Theme.dp(110)
        z: 5
    }

    Dialog {
        id: confirm
        anchors.fill: parent
        z: 6
    }

    Keys.onPressed: function (event) {
        if (!cc.open || confirm.open) {
            event.accepted = cc.open;
            return;
        }
        event.accepted = true;
        var horizontal = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        if (event.isAutoRepeat && !horizontal && !vertical)
            return;
        var d = event.key === Qt.Key_Left || event.key === Qt.Key_Up ? -1 : 1;
        if (cc.hidden)
            return;
        if (api.keys.isDetails(event)) {
            if (!cc.loading)
                cc.act("shot");
            else
                Sound.play("edge");
            return;
        }
        if (cc.zone === "sheet") {
            if (cc.viewing) {
                if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                    Sound.play("back");
                    cc.viewing = false;
                } else if (horizontal)
                    cc.shot = Sound.stepped(cc.shot, d, cc.shots.length);
                return;
            }
            if (api.keys.isCancel(event))
                cc.closeSheet();
            else if (cc.sheet === "trophies" && vertical)
                cc.shot = Sound.stepped(cc.shot, d, cc.trophyRows.length);
            else if (cc.sheet === "captures" && horizontal)
                cc.shot = Sound.stepped(cc.shot, d, cc.shots.length);
            else if (cc.sheet === "captures" && vertical) {
                var next = cc.shot + d * shotGrid.columns;
                if (next >= 0 && next < cc.shots.length) {
                    Sound.play("tick");
                    cc.shot = next;
                } else
                    Sound.play("edge");
            } else if (api.keys.isAccept(event) && cc.sheet === "captures" && cc.shots.length > 0) {
                Sound.play("ok");
                cc.viewing = true;
            } else if (api.keys.isFilters(event) && cc.sheet === "captures" && cc.shots[cc.shot]) {
                var shotRow = cc.shots[cc.shot];
                confirm.show({
                    message: "Remove this screenshot?",
                    detail: shotRow.dateText + ". The picture goes to the trash" + (shotRow.hasJournal ? "; its journal entry keeps the rest." : "."),
                    buttons: ["Keep It", "Trash Screenshot"],
                    danger: 1,
                    index: 0
                }, function (i) {
                    cc.forceActiveFocus();
                    if (i === 1)
                        api.screens.shots.remove(shotRow.gameId, shotRow.name);
                });
            } else
                Sound.play("edge");
            return;
        }
        if (api.keys.isCancel(event)) {
            if (cc.zone === "panel") {
                Sound.play("back");
                cc.zone = "bar";
            } else
                cc.close();
            return;
        }
        if (api.keys.isAccept(event)) {
            cc.select();
            return;
        }
        if (cc.zone === "cards") {
            if (horizontal)
                cc.card = Sound.stepped(cc.card, d, cc.cards.length);
            else if (event.key === Qt.Key_Down) {
                Sound.play("tick");
                cc.zone = "bar";
            } else
                Sound.play("edge");
        } else if (cc.zone === "bar") {
            if (horizontal)
                cc.icon = Sound.stepped(cc.icon, d, cc.icons.length);
            else if (event.key === Qt.Key_Up) {
                Sound.play("tick");
                cc.zone = "cards";
            } else
                Sound.play("edge");
        } else if (cc.zone === "panel") {
            if (vertical)
                cc.row = Sound.stepped(cc.row, d, cc.panelRows.length);
            else if (horizontal && cc.target)
                cc.step(cc.target, d);
        }
    }
}
