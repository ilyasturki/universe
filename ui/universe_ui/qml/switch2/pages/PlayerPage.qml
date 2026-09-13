import QtQuick
import QtMultimedia
import "../core"
import "../../core/Format.js" as Format
import "../sound"
import "../ui"

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property bool bare: true

    readonly property var store: api.screens.album
    readonly property var row: store.rows.filter(function(r) { return r.session === args.session; })[0] || null
    readonly property var frames: row ? store.frameMap[row.session] || null : null
    readonly property var game: row ? api.allGames.byId(row.gameId) : null

    property bool footerShown: true
    readonly property bool playing: player.playbackState === MediaPlayer.PlayingState
    readonly property bool stopped: player.playbackState === MediaPlayer.StoppedState
    readonly property bool failed: player.error !== MediaPlayer.NoError
    readonly property real duration: player.duration > 0 ? player.duration
                                   : frames && frames.duration > 0 ? frames.duration * 1000
                                   : row ? row.duration_s * 1000 : 0

    property bool scrubbing: false
    property real scrubPos: 0
    readonly property real shownPos: scrubbing ? scrubPos : player.position
    readonly property real stickX: api.pad.rightX
    readonly property real scrubSpeed: Math.max(60000, duration / 12)
    readonly property real seekStep: 10000

    readonly property var hints: [
        { glyph: "Start", label: footerShown ? "Hide Footer" : "Show Footer" },
        { glyph: "Y", label: playing ? "Pause" : "Play" },
        { glyph: "X", label: "Journal entry", dim: !(row && row.hasJournal) },
        { glyph: "B", label: "Back" },
        { glyph: "A", label: "Menu" }
    ]

    signal closeRequested()

    focus: true

    // args arrive after onCompleted (Loader.onLoaded), so play starts on the first row.
    property bool started: false
    onRowChanged: {
        if (row && !started) {
            started = true;
            store.select(row.session);
            player.source = row.url;
            player.play();
        }
    }

    function togglePlay() {
        if (!row || !row.url) {
            Sound.edge();
            return;
        }
        Sound.ok();
        if (playing)
            player.pause();
        else
            player.play();
        wake();
    }

    function seekBy(ms) {
        if (duration <= 0)
            return;
        if (!scrubbing) {
            scrubPos = player.position;
            scrubbing = true;
        }
        scrubPos = Math.max(0, Math.min(duration, scrubPos + ms));
        wake();
    }

    function commitSeek() {
        if (!scrubbing)
            return;
        scrubbing = false;
        if (stopped) {
            player.play();
            player.pause();
        }
        player.position = scrubPos;
    }

    function wake() {
        footer.awake = true;
        idleTimer.restart();
    }

    function leave() {
        player.stop();
        page.closeRequested();
    }

    function menu() {
        Sound.ok();
        var items = row && row.hasJournal ? [{ label: "Open journal entry", act: "journal" }] : [];
        items.push({ label: "Show file name", act: "name" });
        shell.pick({ title: row ? row.gameTitle + " · " + row.dateText : "", choices: items.map(function(i) { return i.label; }) }, function(i) {
            if (i < 0)
                return;
            if (items[i].act === "journal") {
                player.pause();
                shell.push("pages/ArticlePage.qml", { session: row.session, gameId: row.gameId });
            } else {
                shell.showToast(row ? row.path : "");
            }
        });
    }

    Timer {
        id: idleTimer
        interval: 2600
        onTriggered: footer.awake = false
    }

    Timer {
        id: commitTimer
        interval: 220
        onTriggered: page.commitSeek()
    }

    Timer {
        id: stickTimer
        interval: 16
        repeat: true
        running: page.activeFocus && page.stickX !== 0 && page.duration > 0
        onRunningChanged: {
            if (running) {
                commitTimer.stop();
                if (!page.scrubbing) {
                    page.scrubPos = player.position;
                    page.scrubbing = true;
                }
                page.wake();
            } else if (page.scrubbing) {
                commitTimer.restart();
            }
        }
        onTriggered: {
            var x = page.stickX;
            var speed = (0.12 + 0.88 * x * x) * page.scrubSpeed;
            page.scrubPos = Math.max(0, Math.min(page.duration, page.scrubPos + (x < 0 ? -1 : 1) * speed * interval / 1000));
        }
    }

    Keys.onPressed: function(event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        if (event.isAutoRepeat && !arrow)
            return;
        if (api.keys.isMenu(event)) {
            event.accepted = true;
            Sound.select();
            footerShown = !footerShown;
            wake();
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            Sound.back();
            leave();
        } else if (api.keys.isAccept(event)) {
            event.accepted = true;
            menu();
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            togglePlay();
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            if (row && row.hasJournal) {
                Sound.ok();
                player.pause();
                shell.push("pages/ArticlePage.qml", { session: row.session, gameId: row.gameId });
            } else {
                Sound.edge();
            }
        } else if (arrow) {
            event.accepted = true;
            if (!event.isAutoRepeat)
                Sound.tick();
            commitTimer.stop();
            seekBy(event.key === Qt.Key_Left ? -seekStep : seekStep);
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            event.accepted = true;
            wake();
        }
    }

    Keys.onReleased: function(event) {
        if (event.isAutoRepeat || !page.scrubbing || stickTimer.running)
            return;
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right)
            commitTimer.restart();
    }

    Rectangle {
        anchors.fill: parent
        color: "#000000"
    }

    Grid {
        id: mosaic
        anchors.fill: parent
        columns: 4
        visible: page.stopped || page.failed

        Repeater {
            model: 16

            Item {
                width: mosaic.width / 4
                height: mosaic.height / 4

                Image {
                    anchors.fill: parent
                    source: page.frames ? page.frames.frames[index] : ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    opacity: status === Image.Ready ? 1.0 : 0.0

                    Behavior on opacity {
                        NumberAnimation { duration: Theme.durFade; easing.type: Easing.OutCubic }
                    }
                }
            }
        }
    }

    VideoOutput {
        id: video
        anchors.fill: parent
        visible: !page.stopped && !page.failed
    }

    MediaPlayer {
        id: player
        videoOutput: video
        audioOutput: AudioOutput {}
        onErrorOccurred: function(error, message) { page.wake(); }
    }

    Column {
        anchors.centerIn: parent
        spacing: Theme.dp(12)
        visible: page.failed

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "This recording cannot be played."
            color: "#ffffff"
            font.family: Theme.sans
            font.pixelSize: Theme.dp(Theme.fontBody)
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            width: page.width - Theme.dp(400)
            horizontalAlignment: Text.AlignHCenter
            text: page.row ? page.row.path : ""
            color: "#b0b0b0"
            elide: Text.ElideMiddle
            font.family: Theme.sans
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: Theme.dp(120)
        height: width
        radius: width / 2
        color: Qt.rgba(0, 0, 0, 0.55)
        visible: !page.playing && !page.failed && !page.stopped
        opacity: visible ? 1.0 : 0.0

        Glyph {
            anchors.centerIn: parent
            anchors.horizontalCenterOffset: Theme.dp(4)
            width: Theme.dp(60)
            height: width
            kind: "play"
            tint: "#ffffff"
        }
    }

    Item {
        id: footer

        property bool awake: true
        readonly property bool shown: page.footerShown && (!page.playing || awake || page.scrubbing)
        readonly property real fraction: page.duration > 0 ? page.shownPos / page.duration : 0

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Theme.dp(220)
        opacity: shown ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.rgba(0, 0, 0, 0.0) }
                GradientStop { position: 1.0; color: Qt.rgba(0, 0, 0, 0.7) }
            }
        }

        Item {
            id: bar

            x: Theme.dp(72)
            width: parent.width - x * 2
            y: Theme.dp(98)
            height: Theme.dp(5)

            Rectangle {
                anchors.fill: parent
                radius: height / 2
                color: Qt.rgba(1, 1, 1, 0.35)
            }

            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.width * footer.fraction
                radius: height / 2
                color: Theme.accentStrong
            }

            Rectangle {
                id: knob
                x: parent.width * footer.fraction - width / 2
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(24)
                height: width
                radius: width / 2
                color: "#ffffff"
                scale: page.scrubbing ? 1.3 : 1.0

                Behavior on scale {
                    NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                }
            }

            Rectangle {
                id: peek

                readonly property int frame: Math.max(0, Math.min(15, Math.floor(footer.fraction * 16)))
                readonly property string source: page.frames ? page.frames.frames[frame] : ""

                width: Theme.dp(300)
                height: Math.round(width * 9 / 16)
                x: Math.max(0, Math.min(parent.width - width, knob.x + knob.width / 2 - width / 2))
                anchors.bottom: parent.top
                anchors.bottomMargin: Theme.dp(26)
                radius: Theme.dp(4)
                color: "#101010"
                border.width: Theme.dp(2)
                border.color: "#ffffff"
                opacity: page.scrubbing ? 1.0 : 0.0
                visible: opacity > 0.01

                Behavior on opacity {
                    NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                }

                Image {
                    anchors.fill: parent
                    anchors.margins: Theme.dp(2)
                    source: peek.source
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                }

                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: Theme.dp(8)
                    width: peekTime.implicitWidth + Theme.dp(20)
                    height: Theme.dp(34)
                    radius: Theme.dp(3)
                    color: Qt.rgba(0, 0, 0, 0.7)

                    Text {
                        id: peekTime
                        anchors.centerIn: parent
                        text: Format.clockTime(page.shownPos / 1000)
                        color: "#ffffff"
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                    }
                }
            }
        }

        Text {
            anchors.right: bar.right
            y: bar.y + Theme.dp(20)
            text: Format.clockTime(page.shownPos / 1000) + "  /  " + Format.clockTime(page.duration / 1000)
            color: "#ffffff"
            font.family: Theme.sans
            font.pixelSize: Theme.dp(Theme.fontTiny)
        }

        HintBar {
            anchors.left: parent.left
            anchors.right: parent.right
            y: footer.height - height + Theme.dp(4)
            hints: page.hints
            ink: "#ffffff"
            muted: "#6a6a6a"
            glyphFill: "#ffffff"
            glyphInk: "#000000"
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Theme.dp(4)
        color: Qt.rgba(1, 1, 1, 0.2)
        visible: !footer.shown && !page.stopped

        Rectangle {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: parent.width * footer.fraction
            color: Theme.accentStrong
        }
    }
}
