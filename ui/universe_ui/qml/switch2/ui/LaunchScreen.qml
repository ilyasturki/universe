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

    // from: the tile the game was started from, echoed as the screen goes dark
    function begin(target, from) {
        if (running)
            return;
        game = target;
        launchedSession = "";
        echo.visible = !!from;
        if (from) {
            var p = from.mapToItem(screen, 0, 0);
            echo.x = p.x;
            echo.y = p.y;
            echo.width = from.width;
            echo.height = from.height;
        }
        echo.scale = 1.0;
        echo.opacity = 0.35;
        echo.drift = 0;
        sequence.start();
    }

    function reset() {
        settle.stop();
        waiting = false;
        launchedSession = "";
        frame.opacity = 0.0;
        echo.visible = false;
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

    Tile {
        id: echo

        property real drift: 0

        transform: Translate {
            y: echo.drift
        }
        visible: false
        game: screen.game
        outlineShown: false
    }

    Rectangle {
        id: frame

        anchors.fill: parent
        color: "#000000"
        opacity: 0.0
        visible: opacity > 0.001
    }

    SequentialAnimation {
        id: sequence

        ParallelAnimation {
            NumberAnimation {
                target: echo
                property: "scale"
                to: 1.3
                duration: 300
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                target: echo
                property: "drift"
                to: -Theme.dp(40)
                duration: 300
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                target: echo
                property: "opacity"
                to: 0
                duration: 300
                easing.type: Easing.OutQuad
            }
            SequentialAnimation {
                PauseAnimation {
                    duration: 200
                }
                NumberAnimation {
                    target: frame
                    property: "opacity"
                    to: 1.0
                    duration: 190
                    easing.type: Easing.InQuad
                }
            }
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
