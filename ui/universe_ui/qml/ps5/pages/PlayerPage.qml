import QtQuick
import QtMultimedia
import "../core"
import "../../core/Format.js" as Format
import "../sound"
import "../ui"
import "../../ui" as Base
import "../ui/Removal.js" as Removal

// A recorded session full screen: the controls come up over its foot on any key and go once it plays on,
// the session's sixteen frames along the bar, its journal entry beside the title.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property bool strip: true
    signal closeRequested
    focus: true

    readonly property var store: api.screens.album
    readonly property var row: {
        var all = store.rows;
        return all.filter(function (r) {
            return r.session === args.session;
        })[0] || null;
    }
    readonly property var frames: row ? store.frameMap[row.session] || null : null
    readonly property var game: row ? api.allGames.byId(row.gameId) : args.gameId ? api.allGames.byId(args.gameId) : null
    // The session's journal entry, read once its row is known: its title and what comes next.
    property var entry: null

    property bool controlsShown: true
    readonly property bool playing: player.playbackState === MediaPlayer.PlayingState
    readonly property bool stopped: player.playbackState === MediaPlayer.StoppedState
    readonly property bool failed: player.error !== MediaPlayer.NoError
    readonly property real duration: player.duration > 0 ? player.duration : frames && frames.duration > 0 ? frames.duration * 1000 : row ? row.duration_s * 1000 : 0
    readonly property bool scrubbing: scrub.scrubbing
    readonly property real shownPos: scrub.shownPos
    readonly property real fraction: duration > 0 ? Math.max(0, Math.min(1, shownPos / duration)) : 0

    readonly property var hints: [
        {
            glyph: "Y",
            label: controlsShown ? "Hide Controls" : "Show Controls"
        },
        {
            glyph: "X",
            label: "Journal Entry",
            dim: !(row && row.hasJournal)
        },
        {
            glyph: "dpad",
            label: "Seek 10 s"
        },
        {
            glyph: "Start",
            label: "Options"
        },
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: playing ? "Pause" : "Play"
        }
    ]

    // Pushed cold (from the hub, the gallery): the album is asked for the game's rows once.
    property bool asked: false
    function ensure() {
        if (row || asked || !args.session)
            return;
        asked = true;
        if (args.gameId)
            store.load(args.gameId);
        else
            store.loadAll();
    }

    onArgsChanged: ensure()

    // args arrive after onCompleted (Loader.onLoaded): play starts on the first row found.
    property bool started: false
    onRowChanged: {
        if (!row || started)
            return;
        started = true;
        store.select(row.session);
        var line = (api.universe.journal(row.gameId) || []).filter(function (e) {
            return e.session === row.session;
        })[0];
        entry = line && (line.state || "written") === "written" ? line : null;
        player.source = row.url;
        player.play();
        wake();
    }

    function togglePlay() {
        if (!row || !row.url) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        if (playing)
            player.pause();
        else
            player.play();
        wake();
    }

    function openJournal() {
        if (!(row && row.hasJournal)) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        player.pause();
        shell.push("pages/ArticlePage.qml", {
            gameId: row.gameId,
            session: row.session
        });
    }

    function wake() {
        controls.awake = true;
        idleTimer.restart();
    }

    function leave() {
        player.stop();
        page.closeRequested();
    }

    function options() {
        if (!row) {
            Sound.play("edge");
            return;
        }
        var items = [
            {
                label: playing ? "Pause" : "Play",
                glyph: playing ? "pause" : "play",
                act: "play"
            }
        ];
        if (row.hasJournal)
            items.push({
                label: "Journal Entry",
                glyph: "journal",
                act: "journal"
            });
        items.push({
            label: "Show File Name",
            glyph: "file",
            act: "name"
        }, {
            label: "Delete Recording…",
            glyph: "trash",
            act: "remove",
            gap: true
        });
        shell.showMenu({
            title: row.gameTitle + " · " + row.dateText,
            items: items
        }, function (i) {
            if (i < 0)
                return;
            var act = items[i].act;
            if (act === "play")
                togglePlay();
            else if (act === "journal")
                openJournal();
            else if (act === "name")
                shell.showToast(row.path);
            else {
                player.pause();
                Removal.recording(shell, api.screens, row, function () {
                    page.leave();
                });
            }
        });
    }

    function stamp(ms) {
        var s = Math.max(0, Math.floor(ms / 1000));
        var h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = s % 60;
        var two = function (n) {
            return ("0" + n).slice(-2);
        };
        return (h > 0 ? h + ":" : "") + two(m) + ":" + two(r);
    }

    Timer {
        id: idleTimer
        interval: 2600
        onTriggered: controls.awake = false
    }

    Base.Scrubber {
        id: scrub
        player: player
        duration: page.duration
        active: page.activeFocus
        onWoke: page.wake()
    }

    Keys.onPressed: function (event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        if (event.isAutoRepeat && !arrow)
            return;
        event.accepted = true;
        if (arrow) {
            if (!event.isAutoRepeat)
                Sound.play("tick");
            scrub.seekBy(event.key === Qt.Key_Left ? -scrub.step : scrub.step);
        } else if (api.keys.isAccept(event)) {
            togglePlay();
        } else if (api.keys.isCancel(event)) {
            Sound.play("back");
            leave();
        } else if (api.keys.isMenu(event)) {
            options();
        } else if (api.keys.isFilters(event)) {
            Sound.play("select");
            controlsShown = !controlsShown;
            wake();
        } else if (api.keys.isDetails(event)) {
            openJournal();
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            wake();
        } else {
            event.accepted = false;
        }
    }

    Keys.onReleased: function (event) {
        if (!event.isAutoRepeat && (event.key === Qt.Key_Left || event.key === Qt.Key_Right))
            scrub.release();
    }

    Rectangle {
        anchors.fill: parent
        color: "#000000"
    }

    // Not playing: the session laid out as its sixteen frames, as a contact sheet.
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
                    opacity: status === Image.Ready ? 0.55 : 0.0

                    Behavior on opacity {
                        NumberAnimation {
                            duration: Theme.durHero
                        }
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
        onErrorOccurred: function (error, message) {
            page.wake();
        }
    }

    Column {
        anchors.centerIn: parent
        spacing: Theme.dp(12)
        visible: page.failed || (page.row === null && page.asked)

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            text: page.row === null ? "This recording is gone." : "This recording cannot be played."
            font.pixelSize: Theme.dp(Theme.fontTitle)
        }

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            width: page.width - Theme.dp(400)
            horizontalAlignment: Text.AlignHCenter
            visible: page.row !== null
            text: page.row ? page.row.path : ""
            color: Theme.textSecondary
            elide: Text.ElideMiddle
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }

    Rectangle {
        anchors.centerIn: parent
        width: Theme.dp(128)
        height: width
        radius: width / 2
        color: Qt.rgba(0, 0, 0, 0.5)
        border.width: Theme.dp(3)
        border.color: Qt.rgba(1, 1, 1, 0.85)
        opacity: !page.playing && !page.failed && page.row !== null ? 1.0 : 0.0
        scale: page.playing ? 0.85 : 1.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durFocus
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: Theme.durFocus
                easing.type: Easing.OutCubic
            }
        }

        Glyph {
            anchors.centerIn: parent
            anchors.horizontalCenterOffset: Theme.dp(4)
            width: Theme.dp(54)
            height: width
            kind: "play"
        }
    }

    Item {
        id: controls

        property bool awake: true
        readonly property bool shown: page.controlsShown && (!page.playing || awake || page.scrubbing)
        // The shell's hint strip sits under the controls.
        readonly property real floor: Theme.dp(76)

        anchors.fill: parent
        opacity: shown ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            height: Theme.dp(220)
            gradient: Gradient {
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(0, 0, 0, 0.7)
                }
                GradientStop {
                    position: 1.0
                    color: Qt.rgba(0, 0, 0, 0)
                }
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: Theme.dp(430)
            gradient: Gradient {
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(0, 0, 0, 0)
                }
                GradientStop {
                    position: 0.55
                    color: Qt.rgba(0, 0, 0, 0.62)
                }
                GradientStop {
                    position: 1.0
                    color: Qt.rgba(0, 0, 0, 0.85)
                }
            }
        }

        Row {
            x: Theme.dp(48)
            y: Theme.dp(48)
            spacing: Theme.dp(44)

            TileArt {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(Theme.headerIcon)
                height: width
                radius: Theme.dp(12)
                game: page.game
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(4)

                Label {
                    text: page.row ? page.row.gameTitle : page.game ? page.game.title : ""
                    font.weight: Font.Light
                    font.pixelSize: Theme.dp(Theme.fontTitle)
                }

                Label {
                    text: page.row ? page.row.dateText + "  ·  " + page.row.durationText + "  ·  " + page.row.sizeText : ""
                    color: Theme.textSecondary
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }
            }
        }

        Rectangle {
            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(Theme.columnRight)
            y: Theme.dp(48)
            width: Math.min(Theme.dp(620), parent.width * 0.36)
            height: nextColumn.height + Theme.dp(40)
            radius: Theme.dp(Theme.radiusCard)
            color: Theme.glass
            border.width: 1
            border.color: Theme.glassEdge
            visible: page.entry !== null

            Column {
                id: nextColumn
                x: Theme.dp(22)
                y: Theme.dp(20)
                width: parent.width - Theme.dp(44)
                spacing: Theme.dp(8)

                Row {
                    spacing: Theme.dp(10)

                    Glyph {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(24)
                        height: width
                        kind: "journal"
                        tint: Theme.textSecondary
                    }

                    Label {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Journal"
                        color: Theme.textSecondary
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                    }
                }

                Label {
                    width: parent.width
                    text: page.entry ? page.entry.title || "" : ""
                    elide: Text.ElideRight
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(26)
                }

                Label {
                    width: parent.width
                    visible: text !== ""
                    text: page.entry && page.entry.next_up ? "Next up: " + page.entry.next_up : ""
                    textFormat: Text.PlainText
                    color: Theme.textSecondary
                    wrapMode: Text.WordWrap
                    maximumLineCount: 3
                    elide: Text.ElideRight
                    lineHeight: 1.2
                    font.pixelSize: Theme.dp(Theme.fontTiny)
                }
            }
        }

        Item {
            id: panel

            x: Theme.dp(Theme.edge)
            width: parent.width - x - Theme.dp(Theme.columnRight)
            height: Theme.dp(220)
            anchors.bottom: parent.bottom
            anchors.bottomMargin: controls.floor + Theme.dp(20)

            // The sixteen frames along the bar, the one under the play head lit.
            Row {
                id: film

                readonly property real cell: (panel.width - spacing * 15) / 16

                width: panel.width
                height: Math.round(cell * 9 / 16)
                spacing: Theme.dp(4)
                visible: page.frames !== null

                Repeater {
                    model: 16

                    Item {
                        readonly property bool here: Math.min(15, Math.floor(page.fraction * 16)) === index

                        width: film.cell
                        height: film.height

                        Rectangle {
                            anchors.fill: parent
                            color: Theme.artShade
                        }

                        Image {
                            anchors.fill: parent
                            source: page.frames ? page.frames.frames[index] : ""
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            opacity: status === Image.Ready ? (parent.here ? 1.0 : 0.55) : 0.0
                        }

                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: -Theme.dp(3)
                            color: "transparent"
                            border.width: Theme.dp(2)
                            border.color: Theme.ring
                            visible: parent.here
                        }
                    }
                }
            }

            Item {
                id: bar

                y: film.visible ? film.height + Theme.dp(34) : Theme.dp(60)
                width: panel.width
                height: page.scrubbing ? Theme.dp(8) : Theme.dp(5)

                Behavior on height {
                    NumberAnimation {
                        duration: Theme.durFocus
                    }
                }

                Rectangle {
                    anchors.fill: parent
                    radius: height / 2
                    color: Qt.rgba(1, 1, 1, 0.25)
                }

                Rectangle {
                    width: parent.width * page.fraction
                    height: parent.height
                    radius: height / 2
                    color: Theme.text
                }

                Rectangle {
                    id: knob
                    x: parent.width * page.fraction - width / 2
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(22)
                    height: width
                    radius: width / 2
                    color: Theme.text
                    scale: page.scrubbing ? 1.4 : 1.0

                    Behavior on scale {
                        NumberAnimation {
                            duration: Theme.durFocus
                            easing.type: Easing.OutCubic
                        }
                    }
                }

                // A tap on the bar seeks there; the strip is taller than the bar so a finger can hit it.
                Item {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    height: Theme.dp(44)

                    TapHandler {
                        onTapped: function (point) {
                            scrub.seekTo(point.position.x / parent.width * page.duration);
                        }
                    }
                }

                Base.RoundedMask {
                    id: peek

                    readonly property int frame: Math.max(0, Math.min(15, Math.floor(page.fraction * 16)))

                    width: Theme.dp(300)
                    height: Math.round(width * 9 / 16)
                    radius: Theme.dp(6)
                    x: Math.max(0, Math.min(parent.width - width, knob.x + knob.width / 2 - width / 2))
                    anchors.bottom: parent.top
                    anchors.bottomMargin: Theme.dp(24) + (film.visible ? film.height + Theme.dp(10) : 0)
                    opacity: page.scrubbing ? 1.0 : 0.0
                    visible: opacity > 0.01

                    Behavior on opacity {
                        NumberAnimation {
                            duration: Theme.durFocus
                        }
                    }

                    Rectangle {
                        anchors.fill: parent
                        color: Theme.artShade
                    }

                    Image {
                        anchors.fill: parent
                        source: page.frames ? page.frames.frames[peek.frame] : ""
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                    }

                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: Theme.dp(8)
                        width: peekTime.implicitWidth + Theme.dp(20)
                        height: Theme.dp(34)
                        radius: Theme.dp(4)
                        color: Qt.rgba(0, 0, 0, 0.72)

                        Label {
                            id: peekTime
                            anchors.centerIn: parent
                            text: page.stamp(page.shownPos)
                            font.pixelSize: Theme.dp(Theme.fontTiny)
                        }
                    }
                }
            }

            PillButton {
                id: playButton
                anchors.left: parent.left
                anchors.top: bar.bottom
                anchors.topMargin: Theme.dp(26)
                width: Theme.dp(64)
                height: Theme.dp(64)
                round: true
                glyph: page.playing ? "pause" : "play"
                focused: true
                onPicked: page.togglePlay()
            }

            Label {
                anchors.left: playButton.right
                anchors.leftMargin: Theme.dp(28)
                anchors.verticalCenter: playButton.verticalCenter
                text: page.stamp(page.shownPos) + "  /  " + page.stamp(page.duration)
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Label {
                anchors.right: parent.right
                anchors.verticalCenter: playButton.verticalCenter
                visible: !page.playing && !page.stopped
                text: "Paused"
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }

    // Controls hidden: a hairline of progress along the foot, as the console's player keeps.
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: controls.floor
        height: Theme.dp(4)
        color: Qt.rgba(1, 1, 1, 0.18)
        visible: !controls.shown && !page.stopped

        Rectangle {
            width: parent.width * page.fraction
            height: parent.height
            color: Theme.text
        }
    }
}
