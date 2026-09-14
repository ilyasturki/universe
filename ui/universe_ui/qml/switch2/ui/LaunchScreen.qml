import QtQuick
import "../core"
import "../sound"

Item {
    id: screen

    property var game: null
    // launch() was called; the screen holds until the game's window is up and focused.
    property bool waiting: false
    property string launchedSession: ""
    readonly property bool running: sequence.running || waiting || settle.running
    // Nobody could tell when the window came up (no shell extension): the screen holds this long.
    readonly property int settleMs: 1500

    signal finished()
    signal failed(var game, string message)

    function begin(target) {
        if (running)
            return;
        game = target;
        launchedSession = "";
        art.game = target;
        caption.text = target.title;
        art.scale = 0.92;
        sequence.start();
    }

    function reset() {
        settle.stop();
        waiting = false;
        launchedSession = "";
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
        function onLaunched(sessionId, id) {
            if (screen.waiting && screen.game && screen.game.id === id)
                screen.launchedSession = sessionId;
        }
        function onSessionShown(sessionId, ok) {
            if (!screen.waiting || sessionId !== screen.launchedSession)
                return;
            if (ok)
                screen.done();
            else
                settle.restart();
        }
        function onLaunchFailed(id, message) {
            if (screen.game && screen.game.id === id)
                screen.abort(message);
        }
        // Over before its window came up: nothing to wait for.
        function onSessionEnded(sessionId, id, duration) {
            if (screen.waiting && sessionId === screen.launchedSession)
                screen.done();
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
                if (screen.game) {
                    screen.waiting = true;
                    screen.game.launch();
                }
            }
        }
    }

    Timer {
        id: settle
        interval: screen.settleMs
        onTriggered: screen.done()
    }
}
