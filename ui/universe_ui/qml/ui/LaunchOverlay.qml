import QtQuick
import "../core"

Item {
    id: overlay

    property var game: null
    readonly property bool running: sequence.running || settle.running || fallback.running
    readonly property int gameTakeoverSlackMs: 1500
    // Launch answers after the pre-launch hooks, which may take their whole timeout.
    readonly property int launchTimeoutMs: 25000
    readonly property int holdMs: 450
    readonly property int dipMs: 300

    signal finished()
    signal failed(var game, string message)

    function begin(targetGame) {
        if (sequence.running)
            return;
        game = targetGame;

        frame.heroSource = targetGame.assets.background;
        frame.boxSource = targetGame.assets.boxFront;
        frame.logoSource = targetGame.assets.logo;
        frame.title = targetGame.title;
        frame.opacity = 0.0;
        frame.artOpacity = 1.0;
        frame.artScale = 1.06;

        sequence.start();
    }

    function reset() {
        settle.stop();
        fallback.stop();
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

    // The daemon answers Launch asynchronously: the page comes back once the session has
    // started and the game is mapping over us, or at once when the launch failed.
    Connections {
        target: api.universe
        function onSessionStarted(sessionId, id) {
            if (overlay.game && overlay.game.id === id)
                settle.restart();
        }
        function onLaunchFailed(id, message) {
            if (overlay.game && overlay.game.id === id)
                overlay.abort(message);
        }
    }

    LaunchFrame {
        id: frame

        anchors.fill: parent
        opacity: 0.0
        visible: opacity > 0.001
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

        PauseAnimation { duration: overlay.holdMs }

        // Down to plain ground before the handover: whatever the compositor
        // animates between this window and the splash is then black on black.
        NumberAnimation {
            target: frame
            property: "artOpacity"
            to: 0.0
            duration: overlay.dipMs
            easing.type: Easing.InOutQuad
        }

        // The ground frame must be on screen before the handover starts.
        PauseAnimation { duration: 120 }

        ScriptAction {
            script: {
                // Armed first: a throw in launch() would otherwise strand the overlay.
                fallback.restart();
                if (overlay.game)
                    overlay.game.launch();
            }
        }
    }

    Timer {
        id: settle
        interval: overlay.gameTakeoverSlackMs
        onTriggered: {
            overlay.reset();
            overlay.finished();
        }
    }

    Timer {
        id: fallback
        interval: overlay.launchTimeoutMs
        onTriggered: {
            overlay.reset();
            overlay.finished();
        }
    }
}
