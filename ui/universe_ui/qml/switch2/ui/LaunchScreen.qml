import QtQuick
import "../core"
import "../sound"

Item {
    id: screen

    property var game: null
    readonly property bool running: sequence.running || settle.running || fallback.running
    readonly property int takeoverSlackMs: 1500
    readonly property int launchTimeoutMs: 25000

    signal finished()
    signal failed(var game, string message)

    function begin(target) {
        if (sequence.running)
            return;
        game = target;
        art.game = target;
        caption.text = target.title;
        art.scale = 0.92;
        sequence.start();
    }

    function reset() {
        settle.stop();
        fallback.stop();
        frame.opacity = 0.0;
        art.game = null;
        game = null;
    }

    function done() {
        reset();
        finished();
    }

    function abort(message) {
        var g = game;
        sequence.stop();
        done();
        failed(g, message);
    }

    Connections {
        target: api.universe
        function onSessionStarted(sessionId, id) {
            if (screen.game && screen.game.id === id)
                settle.restart();
        }
        function onLaunchFailed(id, message) {
            if (screen.game && screen.game.id === id)
                screen.abort(message);
        }
    }

    Rectangle {
        id: frame

        anchors.fill: parent
        color: Theme.ground
        opacity: 0.0
        visible: opacity > 0.001

        Tile {
            id: art
            anchors.centerIn: parent
            anchors.verticalCenterOffset: -Theme.dp(50)
            width: Theme.dp(420)
            height: Theme.dp(420)
            game: null
            outlineShown: false
            lift: false
        }

        Text {
            id: caption
            anchors.top: art.bottom
            anchors.topMargin: Theme.dp(40)
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width - Theme.dp(300)
            color: Theme.text
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
            font.family: Theme.sans
            font.pixelSize: Theme.dp(Theme.fontTitle)
        }
    }

    SequentialAnimation {
        id: sequence

        ParallelAnimation {
            NumberAnimation { target: frame; property: "opacity"; to: 1.0; duration: Theme.durFade; easing.type: Easing.InOutQuad }
            NumberAnimation { target: art; property: "scale"; to: 1.0; duration: Theme.durFade; easing.type: Easing.OutCubic }
        }
        PauseAnimation { duration: 500 }
        ScriptAction {
            script: {
                fallback.restart();
                if (screen.game)
                    screen.game.launch();
            }
        }
    }

    Timer {
        id: settle
        interval: screen.takeoverSlackMs
        onTriggered: screen.done()
    }

    Timer {
        id: fallback
        interval: screen.launchTimeoutMs
        onTriggered: screen.done()
    }
}
