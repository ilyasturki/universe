import QtQuick
import "../core"

Item {
    id: screen

    property var game: null
    property bool waiting: false
    property string launchedSession: ""
    readonly property bool running: sequence.running || waiting || settle.running

    signal finished
    signal failed(var game, string message)

    function begin(target) {
        if (running)
            return;
        game = target;
        launchedSession = "";
        art.scale = 0.92;
        sequence.start();
    }

    function reset() {
        settle.stop();
        waiting = false;
        launchedSession = "";
        frame.opacity = 0.0;
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
            game: screen.game
            outlineShown: false
        }

        Label {
            anchors.top: art.bottom
            anchors.topMargin: Theme.dp(40)
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width - Theme.dp(300)
            text: screen.game ? screen.game.title : ""
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
            font.pixelSize: Theme.dp(Theme.fontTitle)
        }
    }

    SequentialAnimation {
        id: sequence

        ParallelAnimation {
            NumberAnimation {
                target: frame
                property: "opacity"
                to: 1.0
                duration: Theme.durFade
                easing.type: Easing.InOutQuad
            }
            NumberAnimation {
                target: art
                property: "scale"
                to: 1.0
                duration: Theme.durFade
                easing.type: Easing.OutCubic
            }
        }
        PauseAnimation {
            duration: 500
        }
        ScriptAction {
            script: {
                if (screen.game) {
                    screen.waiting = true;
                    screen.game.launch();
                }
            }
        }
    }

    // Nobody could tell when the window came up (no shell extension): the screen holds this long.
    Timer {
        id: settle
        interval: 1500
        onTriggered: screen.done()
    }
}
