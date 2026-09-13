import QtQuick
import QtMultimedia
import "../core"
import "../core/Format.js" as Format
import "../sound"
import "../ui"

// A game's recorded sessions: the list on the left, the picked one playing on the right.
// Right or A hands focus to the player; the right stick scrubs its seek bar.
FocusScope {
    id: page

    focus: true

    property var game: null
    // Set by the shell when the journal jumps here.
    property string session: ""
    readonly property var store: api.screens.recordings
    readonly property var rows: store.rows
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
    readonly property string currentSession: current ? current.session : ""
    readonly property var frames: current && store.frameMap[current.session] ? store.frameMap[current.session] : null

    property bool videoFocused: false
    readonly property bool playing: player.playbackState === MediaPlayer.PlayingState
    readonly property bool stopped: player.playbackState === MediaPlayer.StoppedState
    // ms: the player's once loaded, else the probed file's, else the session's.
    readonly property real duration: player.duration > 0 ? player.duration
                                   : frames && frames.duration > 0 ? frames.duration * 1000
                                   : current ? current.duration_s * 1000 : 0

    // The seek in flight: the bar shows it at once, the player gets it on release.
    property bool scrubbing: false
    property real scrubPos: 0
    readonly property real shownPos: scrubbing ? scrubPos : player.position
    readonly property real stickX: api.pad.rightX
    // ms per second at full tilt: a dozen seconds across the recording, never under a minute a second.
    readonly property real scrubSpeed: Math.max(60000, duration / 12)
    readonly property real seekStep: 10000

    signal closeRequested()
    signal jumpRequested(string source, string session)

    readonly property var hints: {
        var out = [];
        if (videoFocused) {
            out.push({ glyph: "A", label: playing ? "Pause" : "Play" });
            out.push({ glyph: "dpad", label: "Seek 10 s" });
            out.push({ glyph: "RS", label: "Scrub" });
        } else {
            out.push({ glyph: "A", label: "Play" });
            out.push({ glyph: "dpad", label: "Navigate" });
        }
        if (current && current.hasJournal)
            out.push({ glyph: "Y", label: "Journal entry" });
        out.push({ glyph: "B", label: videoFocused ? "Back to list" : "Back" });
        return out;
    }

    readonly property real sideMargin: Theme.dp(90)
    readonly property real listWidth: Theme.dp(620)

    onGameChanged: {
        player.stop();
        index = 0;
        videoFocused = false;
        if (game)
            store.load(game.id);
        landOnSession();
    }

    onSessionChanged: landOnSession()
    onRowsChanged: landOnSession()

    function landOnSession() {
        if (session === "")
            return;
        for (var i = 0; i < rows.length; i++) {
            if (rows[i].session === session) {
                index = i;
                return;
            }
        }
    }

    onCurrentChanged: {
        player.stop();
        scrubbing = false;
        player.source = current ? current.url : "";
        if (current)
            store.select(current.session);
    }

    function step(d) {
        var next = Math.max(0, Math.min(rows.length - 1, index + d));
        next === index ? Sound.edge() : Sound.tick();
        index = next;
    }

    function focusVideo(play) {
        if (!current || !current.url) {
            Sound.edge();
            return;
        }
        Sound.panel();
        videoFocused = true;
        if (play && !playing)
            player.play();
        wake();
    }

    function togglePlay() {
        if (!current || !current.url) {
            Sound.edge();
            return;
        }
        Sound.enter();
        if (playing)
            player.pause();
        else
            player.play();
        wake();
    }

    // Presses add up on the pending target; the player only hears the last one.
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
        controls.awake = true;
        idleTimer.restart();
    }

    Timer {
        id: idleTimer
        interval: 2600
        onTriggered: controls.awake = false
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
        running: page.videoFocused && page.stickX !== 0 && page.duration > 0
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
        if (event.isAutoRepeat && !(page.videoFocused && arrow))
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            page.videoFocused ? togglePlay() : focusVideo(true);
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            if (page.videoFocused) {
                Sound.cancel();
                page.videoFocused = false;
            } else {
                player.stop();
                page.closeRequested();
            }
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            if (current && current.hasJournal)
                page.jumpRequested("pages/JournalPage.qml", current.session);
            else
                Sound.edge();
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            event.accepted = true;
            page.videoFocused ? Sound.edge() : step(event.key === Qt.Key_Up ? -1 : 1);
        } else if (arrow) {
            event.accepted = true;
            if (page.videoFocused) {
                if (!event.isAutoRepeat)
                    Sound.tick();
                commitTimer.stop();
                seekBy(event.key === Qt.Key_Left ? -seekStep : seekStep);
            } else if (event.key === Qt.Key_Right) {
                focusVideo(false);
            } else {
                Sound.edge();
            }
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
        color: Theme.ground
    }

    GameBackdrop {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
    }

    GameHeader {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        game: page.game
        label: "RECORDINGS"
        detail: Format.sessions(page.rows.length)
    }

    Text {
        anchors.centerIn: parent
        visible: page.rows.length === 0
        text: "No recordings for this game yet."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    ListView {
        id: list

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(32)
        anchors.bottom: hintBar.top
        anchors.left: parent.left
        anchors.leftMargin: page.sideMargin
        width: page.listWidth
        model: page.rows
        currentIndex: page.index
        interactive: false
        clip: true
        spacing: Theme.dp(12)
        opacity: page.videoFocused ? 0.55 : 1.0
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: 0
        preferredHighlightEnd: height
        highlightRangeMode: ListView.ApplyRange

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        delegate: Rectangle {
            readonly property bool focused: index === page.index
            readonly property bool lit: focused && !page.videoFocused
            readonly property var frames: page.store.frameMap[modelData.session] || null

            width: list.width
            height: Theme.dp(120)
            radius: Theme.dp(16)
            color: lit ? Theme.text : Theme.surface

            Behavior on color {
                ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
            }

            RoundedMask {
                id: thumb
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                anchors.margins: Theme.dp(10)
                width: height * 16 / 9
                radius: Theme.dp(10)

                Rectangle {
                    anchors.fill: parent
                    color: Theme.cardBase
                }

                Image {
                    anchors.fill: parent
                    source: frames ? frames.thumbnail : ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    opacity: status === Image.Ready ? 1.0 : 0.0

                    Behavior on opacity {
                        NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
                    }
                }
            }

            Column {
                anchors.left: thumb.right
                anchors.leftMargin: Theme.dp(20)
                anchors.right: journalMark.visible ? journalMark.left : parent.right
                anchors.rightMargin: Theme.dp(20)
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(6)

                Text {
                    width: parent.width
                    text: modelData.dateText
                    color: lit ? Theme.onLight : Theme.text
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(24)
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: modelData.durationText + " · " + modelData.sizeText
                    color: lit ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(20)
                    elide: Text.ElideRight
                }
            }

            MenuGlyph {
                id: journalMark
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(24)
                height: width
                visible: modelData.hasJournal
                kind: "book"
                tint: lit ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Theme.textMuted
            }
        }
    }

    RoundedMask {
        id: pane

        anchors.top: list.top
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(40)
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        height: width * 9 / 16
        radius: Theme.dp(16)
        visible: page.rows.length > 0

        Rectangle {
            anchors.fill: parent
            color: Theme.cardBase
        }

        // 4×4 of the sampled frames: the poster while stopped.
        Grid {
            id: mosaic
            anchors.fill: parent
            columns: 4
            visible: page.stopped
            opacity: page.stopped ? 1.0 : 0.0

            Behavior on opacity {
                NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
            }

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
                            NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic }
                        }
                    }
                }
            }
        }

        VideoOutput {
            id: video
            anchors.fill: parent
            visible: !page.stopped
        }

        MediaPlayer {
            id: player
            videoOutput: video
            audioOutput: AudioOutput {}
            onErrorOccurred: function(error, message) { page.wake(); }
        }

        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0.02, 0.02, 0.03, page.stopped ? 0.42 : 0.30)
            opacity: page.playing ? 0.0 : 1.0

            Behavior on opacity {
                NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
            }
        }

        Canvas {
            anchors.centerIn: parent
            width: Theme.dp(96)
            height: Theme.dp(96)
            opacity: page.playing ? 0.0 : 1.0
            scale: page.playing ? 0.8 : 1.0

            Behavior on opacity { NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic } }
            Behavior on scale { NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutBack } }

            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                ctx.fillStyle = Qt.rgba(1, 1, 1, 0.92);
                ctx.beginPath();
                ctx.arc(width / 2, height / 2, width / 2, 0, Math.PI * 2);
                ctx.fill();
                ctx.fillStyle = "#101116";
                ctx.beginPath();
                ctx.moveTo(width * 0.40, height * 0.30);
                ctx.lineTo(width * 0.72, height * 0.50);
                ctx.lineTo(width * 0.40, height * 0.70);
                ctx.closePath();
                ctx.fill();
            }
        }

        Text {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: Theme.dp(84)
            visible: player.error !== MediaPlayer.NoError
            text: player.errorString
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(20)
        }

        Item {
            id: controls

            property bool awake: false
            readonly property bool shown: page.videoFocused && (!page.playing || awake || page.scrubbing)
            readonly property real inset: Theme.dp(36)
            readonly property real fraction: page.duration > 0 ? page.shownPos / page.duration : 0

            anchors.fill: parent
            visible: !page.stopped || page.scrubbing
            opacity: shown ? 1.0 : 0.0

            Behavior on opacity {
                NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: Theme.dp(170)
                gradient: Gradient {
                    GradientStop { position: 0.0; color: Qt.rgba(0.02, 0.02, 0.03, 0.0) }
                    GradientStop { position: 1.0; color: Qt.rgba(0.02, 0.02, 0.03, 0.85) }
                }
            }

            Item {
                id: bar

                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.leftMargin: controls.inset
                anchors.rightMargin: controls.inset
                anchors.bottomMargin: Theme.dp(64)
                height: page.scrubbing ? Theme.dp(10) : Theme.dp(6)

                Behavior on height {
                    NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
                }

                Rectangle {
                    anchors.fill: parent
                    radius: height / 2
                    color: Qt.rgba(1, 1, 1, 0.22)
                }

                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: parent.width * controls.fraction
                    radius: height / 2
                    color: Theme.text
                }

                // Lifted and grown while a seek is in flight, back with an overshoot once it lands.
                Rectangle {
                    id: knob
                    x: parent.width * controls.fraction - width / 2
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: page.scrubbing ? -Theme.dp(4) : 0
                    width: Theme.dp(18)
                    height: width
                    radius: width / 2
                    color: Theme.text
                    scale: page.scrubbing ? 1.7 : 1.0

                    Behavior on scale {
                        NumberAnimation { duration: page.scrubbing ? Theme.durBase : Theme.durNudge; easing.type: page.scrubbing ? Easing.OutCubic : Easing.OutBack }
                    }
                    Behavior on anchors.verticalCenterOffset {
                        NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutBack }
                    }

                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width * 1.9
                        height: width
                        radius: width / 2
                        color: Qt.rgba(1, 1, 1, 0.16)
                        opacity: page.scrubbing ? 1.0 : 0.0
                        z: -1

                        Behavior on opacity { NumberAnimation { duration: Theme.durBase } }
                    }
                }

                // The sampled frame nearest the cursor.
                RoundedMask {
                    id: peek

                    readonly property int frame: Math.max(0, Math.min(15, Math.floor(controls.fraction * 16)))
                    readonly property string source: page.frames ? page.frames.frames[frame] : ""

                    width: Theme.dp(300)
                    height: width * 9 / 16
                    radius: Theme.dp(10)
                    x: Math.max(0, Math.min(parent.width - width, knob.x + knob.width / 2 - width / 2))
                    anchors.bottom: parent.top
                    anchors.bottomMargin: page.scrubbing ? Theme.dp(34) : Theme.dp(18)
                    opacity: page.scrubbing ? 1.0 : 0.0
                    scale: page.scrubbing ? 1.0 : 0.9
                    transformOrigin: Item.Bottom

                    Behavior on opacity { NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic } }
                    Behavior on scale { NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutBack } }
                    Behavior on anchors.bottomMargin { NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutBack } }

                    Rectangle {
                        anchors.fill: parent
                        color: Theme.cardBase
                    }

                    Image {
                        anchors.fill: parent
                        source: peek.source
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                    }

                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: Theme.dp(10)
                        width: peekTime.width + Theme.dp(20)
                        height: peekTime.height + Theme.dp(8)
                        radius: height / 2
                        color: Qt.rgba(0.02, 0.02, 0.03, 0.75)

                        Text {
                            id: peekTime
                            anchors.centerIn: parent
                            text: Format.clockTime(page.shownPos / 1000)
                            color: Theme.text
                            font.family: Theme.sans
                            font.weight: Font.DemiBold
                            font.pixelSize: Theme.dp(20)
                        }
                    }
                }
            }

            Text {
                anchors.left: bar.left
                anchors.top: bar.bottom
                anchors.topMargin: Theme.dp(14)
                text: Format.clockTime(page.shownPos / 1000) + "  /  " + Format.clockTime(page.duration / 1000)
                color: Theme.textHint
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(21)
            }

            Text {
                anchors.right: bar.right
                anchors.top: bar.bottom
                anchors.topMargin: Theme.dp(14)
                text: page.current ? page.current.dateText : ""
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: Theme.dp(5)
            color: Qt.rgba(1, 1, 1, 0.15)
            visible: !page.stopped
            opacity: controls.shown ? 0.0 : 1.0

            Behavior on opacity { NumberAnimation { duration: Theme.durBase } }

            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.width * controls.fraction
                color: Theme.text
            }
        }
    }

    Loader {
        anchors.fill: pane
        active: page.videoFocused
        sourceComponent: FocusRing {
            cornerRadius: pane.radius
            gapWidth: Theme.dp(4)
        }
    }

    Text {
        anchors.top: pane.bottom
        anchors.topMargin: Theme.dp(22)
        anchors.left: pane.left
        visible: page.current !== null
        text: page.current ? page.current.dateText + "  ·  " + page.current.durationText + "  ·  " + page.current.sizeText : ""
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(22)
    }

    Text {
        anchors.top: pane.bottom
        anchors.topMargin: Theme.dp(22)
        anchors.right: pane.right
        visible: page.current !== null
        width: pane.width * 0.45
        horizontalAlignment: Text.AlignRight
        text: page.current ? page.current.path.split("/").pop() : ""
        color: Theme.textFaint
        font.family: Theme.sans
        font.pixelSize: Theme.dp(20)
        elide: Text.ElideMiddle
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        showClock: true
        hints: page.hints
    }
}
