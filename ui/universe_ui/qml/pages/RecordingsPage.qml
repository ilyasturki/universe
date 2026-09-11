import QtQuick
import QtMultimedia
import "../core"
import "../sound"
import "../ui"

// A game's recordings: the list on the left, the picked clip playing on the right.
FocusScope {
    id: page

    focus: true

    property var game: null
    readonly property var store: api.screens.recordings
    readonly property var rows: store.rows
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
    readonly property bool playing: player.playbackState === MediaPlayer.PlayingState

    signal closeRequested()

    readonly property var hints: [
        { glyph: "A", label: playing ? "Pause" : "Play" },
        { glyph: "dpad", label: "Navigate" },
        { glyph: "B", label: "Back" }
    ]

    readonly property real sideMargin: Theme.dp(90)
    readonly property real listWidth: Theme.dp(620)

    onGameChanged: {
        player.stop();
        index = 0;
        if (game)
            store.load(game.id);
    }

    onCurrentChanged: {
        player.stop();
        player.source = current ? current.url : "";
    }

    function step(d) {
        var next = Math.max(0, Math.min(rows.length - 1, index + d));
        next === index ? Sound.edge() : Sound.tick();
        index = next;
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
    }

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)
    Keys.onLeftPressed: Sound.edge()
    Keys.onRightPressed: Sound.edge()

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            togglePlay();
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            player.stop();
            page.closeRequested();
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    Item {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(44)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: title.height + Theme.dp(10) + subtitle.height

        Text {
            id: title
            text: "Recordings"
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(46)
        }

        Text {
            id: subtitle
            anchors.top: title.bottom
            anchors.topMargin: Theme.dp(10)
            text: (page.game ? page.game.title : "") + (page.rows.length > 0 ? " · " + page.rows.length + (page.rows.length === 1 ? " clip" : " clips") : "")
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(24)
        }
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
        anchors.topMargin: Theme.dp(30)
        anchors.bottom: hintBar.top
        anchors.left: parent.left
        anchors.leftMargin: page.sideMargin
        width: page.listWidth
        model: page.rows
        currentIndex: page.index
        interactive: false
        clip: true
        spacing: Theme.dp(12)
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: 0
        preferredHighlightEnd: height
        highlightRangeMode: ListView.ApplyRange

        delegate: Rectangle {
            readonly property bool focused: index === page.index

            width: list.width
            height: Theme.dp(120)
            radius: Theme.dp(16)
            color: focused ? Theme.text : Theme.surface

            Behavior on color {
                ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
            }

            Rectangle {
                id: thumb
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                anchors.margins: Theme.dp(10)
                width: height * 16 / 9
                radius: Theme.dp(10)
                color: Theme.cardBase
                clip: true

                Image {
                    anchors.fill: parent
                    source: modelData.thumbnail
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                }
            }

            Column {
                anchors.left: thumb.right
                anchors.leftMargin: Theme.dp(20)
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(20)
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(6)

                Text {
                    width: parent.width
                    text: modelData.dateText
                    color: focused ? Theme.onLight : Theme.text
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(24)
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: modelData.durationText + " · " + modelData.sizeText
                    color: focused ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(20)
                    elide: Text.ElideRight
                }
            }
        }
    }

    Rectangle {
        id: preview

        anchors.top: list.top
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(40)
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        height: width * 9 / 16
        radius: Theme.dp(16)
        color: Theme.cardBase
        clip: true
        visible: page.rows.length > 0

        Image {
            anchors.fill: parent
            source: page.current ? page.current.thumbnail : ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            visible: !page.playing && player.playbackState !== MediaPlayer.PausedState
        }

        VideoOutput {
            id: video
            anchors.fill: parent
        }

        MediaPlayer {
            id: player
            videoOutput: video
            audioOutput: AudioOutput {}
        }

        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0.02, 0.02, 0.03, 0.35)
            visible: !page.playing
        }

        Canvas {
            anchors.centerIn: parent
            width: Theme.dp(96)
            height: Theme.dp(96)
            visible: !page.playing
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

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: Theme.dp(6)
            color: Qt.rgba(1, 1, 1, 0.15)
            visible: player.duration > 0

            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: player.duration > 0 ? parent.width * player.position / player.duration : 0
                color: Theme.text
            }
        }
    }

    Text {
        anchors.top: preview.bottom
        anchors.topMargin: Theme.dp(20)
        anchors.left: preview.left
        anchors.right: preview.right
        visible: page.current !== null
        text: page.current ? page.current.path : ""
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(19)
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
