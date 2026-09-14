import QtQuick
import "../core"
import "../sound"

// The launch poster. The page rises into it, and the poster holds — art, logo and title — until
// the game's window is up and focused; it then leaves under it, and the launcher is home again
// with the game pinned first, ready for when the desktop hands back.
FocusScope {
    id: overlay

    property var game: null
    readonly property var session: api.universe.currentSession
    readonly property bool sessionRunning: session !== null && session !== undefined && session.session_id !== undefined
    // A launch is in progress: from the first frame of the poster to its last.
    readonly property bool running: sequence.running || waiting || settle.running || exit.running
    readonly property int holdMs: 450
    // Nobody could tell when the window came up (no shell extension): the poster holds this long.
    readonly property int settleMs: 1500

    // launch() was called; the poster holds until the window is on screen.
    property bool waiting: false
    property string launchedSession: ""

    readonly property string title: game ? game.title : ""

    signal finished()
    signal failed(var game, string message)

    function show(targetGame) {
        game = targetGame;
        frame.heroSource = targetGame ? targetGame.assets.background : "";
        frame.boxSource = targetGame ? targetGame.assets.boxFront : "";
        frame.logoSource = targetGame ? targetGame.assets.logo : "";
        frame.title = targetGame ? targetGame.title : "";
    }

    function begin(targetGame) {
        if (running)
            return;
        show(targetGame);
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
        frame.heroSource = "";
        frame.boxSource = "";
        frame.logoSource = "";
        frame.title = "";
        game = null;
    }

    function abort(message) {
        var g = game;
        sequence.stop();
        reset();
        finished();
        failed(g, message);
    }

    // The game has the screen: the poster fades out behind it, the launcher goes home under it.
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
            // Over before its window came up: nothing to hand over to.
            if (overlay.waiting && sessionId === overlay.launchedSession)
                overlay.handOver();
        }
    }

    LaunchFrame {
        id: frame

        anchors.fill: parent
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

        // The page fades on the same curve underneath: one crossfade into the poster.
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

        // The poster is on screen before the handover starts; gamescope's window then maps
        // over it, black until the game draws.
        PauseAnimation { duration: overlay.holdMs }

        ScriptAction {
            script: {
                if (overlay.game) {
                    overlay.waiting = true;
                    overlay.game.launch();
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

    Keys.onPressed: function(event) { event.accepted = true; }
    Keys.onReleased: function(event) { event.accepted = true; }
}
