import QtQuick
import QtQuick.Window
import "../core"

FocusScope {
    id: overlay

    property var game: null
    readonly property var session: api.universe.currentSession
    readonly property bool sessionRunning: session != null && session.session_id !== undefined
    readonly property bool running: sequence.running || waiting || settle.running || exit.running
    // Nobody could tell when the window came up (no shell extension): the poster holds this long.
    readonly property int settleMs: 1500

    property bool waiting: false
    property string launchedSession: ""

    readonly property string title: game ? game.title : ""

    signal finished
    signal failed(var game, string message)

    function begin(targetGame) {
        if (running)
            return;
        game = targetGame;
        launchedSession = "";
        frame.opacity = 0.0;
        frame.artScale = 1.06;
        sequence.start();
        forceActiveFocus();
    }

    function reset() {
        settle.stop();
        waiting = false;
        launchedSession = "";
        frame.opacity = 0.0;
        game = null;
    }

    function abort(message) {
        var g = game;
        sequence.stop();
        reset();
        finished();
        failed(g, message);
    }

    function handOver() {
        if (!waiting)
            return;
        waiting = false;
        settle.stop();
        exit.restart();
    }

    Connections {
        target: api.universe
        function onLaunched(sessionId, id) {
            if (overlay.waiting && overlay.game && overlay.game.id === id)
                overlay.launchedSession = sessionId;
        }
        function onSessionShown(sessionId, ok) {
            if (!overlay.waiting || sessionId !== overlay.launchedSession)
                return;
            if (ok)
                overlay.handOver();
            else
                settle.restart();
        }
        function onLaunchFailed(id, message) {
            if (overlay.game && overlay.game.id === id && !overlay.sessionRunning)
                overlay.abort(message);
        }
        function onSessionEnded(sessionId, id, duration) {
            if (overlay.waiting && sessionId === overlay.launchedSession)
                overlay.handOver();
        }
    }

    LaunchFrame {
        id: frame

        anchors.fill: parent
        game: overlay.game
        opacity: 0.0
        visible: opacity > 0.001
    }

    Timer {
        id: settle
        interval: overlay.settleMs
        onTriggered: overlay.handOver()
    }

    SequentialAnimation {
        id: sequence

        ParallelAnimation {
            NumberAnimation {
                target: frame
                property: "opacity"
                to: 1.0
                duration: Theme.durLaunch
                easing.type: Easing.InOutQuad
            }
            NumberAnimation {
                target: frame
                property: "artScale"
                to: 1.0
                duration: Theme.durLaunch
                easing.type: Easing.OutCubic
            }
        }

        // The grab feeds gamescope's keep-alive window; without one the keep-alive is black.
        ScriptAction {
            script: {
                if (overlay.game) {
                    overlay.waiting = true;
                    var game = overlay.game;
                    var dpr = overlay.Screen.devicePixelRatio;
                    var size = Qt.size(Math.round(frame.width * dpr), Math.round(frame.height * dpr));
                    if (!frame.grabToImage(function (result) {
                        game.launchWith(result);
                    }, size))
                        game.launch();
                }
            }
        }
    }

    SequentialAnimation {
        id: exit

        NumberAnimation {
            target: frame
            property: "opacity"
            to: 0.0
            duration: Theme.durScene
            easing.type: Easing.OutCubic
        }
        ScriptAction {
            script: {
                overlay.reset();
                overlay.finished();
            }
        }
    }

    Keys.onPressed: function (event) {
        event.accepted = true;
        if (waiting && launchedSession !== "" && !event.isAutoRepeat && api.keys.isCancel(event))
            handOver();
    }
    Keys.onReleased: function (event) {
        event.accepted = true;
    }
}
