import QtQuick
import "../core"
import "../core/Format.js" as Format
import "../sound"

// The launch poster, and the running view it turns into. The page rises into the poster, the art
// dips to ground for the handover, and once the session is up the poster stays — art dimmed, the
// time played, a held A to quit — until the session is over; it then fades to the game's page.
FocusScope {
    id: overlay

    property var game: null
    readonly property var session: api.universe.currentSession
    readonly property bool sessionRunning: session !== null && session !== undefined && session.session_id !== undefined
    // The session is up and the launch animation is done — or was never ours (a game already
    // running when the host started).
    readonly property bool playing: sessionRunning && !sequence.running
    readonly property bool running: sequence.running || sessionRunning || exit.running
    readonly property int holdMs: 450
    readonly property int dipMs: 300
    readonly property int quitHoldMs: 1000
    readonly property real restBottom: Theme.dp(72)
    // Room under the caption for the quit pill and the hints.
    readonly property real playBottom: Theme.dp(Theme.hintBarHeight + 24 + 78 + 34)
    readonly property real artDim: 0.62

    property bool holding: false
    property real holdFill: 0.0
    property bool quitting: false
    property int elapsed: 0

    readonly property string title: game ? game.title : (session && session.title ? session.title : "")
    // The controller's own way out, when one is bound for the pad in hand.
    readonly property var stopMacro: {
        var c = api.screens.controller;
        var macros = c.state && c.state.macros ? c.state.macros : [];
        for (var i = 0; i < macros.length; i++) {
            var m = macros[i];
            if (m.action === "stop" && m.trigger === "hold" && (m.family === "*" || m.family === c.family))
                return m;
        }
        return null;
    }
    readonly property var hints: {
        var out = [ { glyph: "A", label: quitting ? "Stopping…" : "Hold to quit" } ];
        if (stopMacro)
            out.push({ glyph: stopMacro.button, label: "Hold in the game to quit" });
        return out;
    }

    signal finished()
    signal failed(var game, string message)
    // The session is over: fired as the poster starts to fade, so the page can rise under it.
    signal ended(var game)

    function show(targetGame) {
        game = targetGame;
        frame.heroSource = targetGame ? targetGame.assets.background : "";
        frame.boxSource = targetGame ? targetGame.assets.boxFront : "";
        frame.logoSource = targetGame ? targetGame.assets.logo : "";
        frame.title = targetGame ? targetGame.title : (session && session.title ? session.title : "");
    }

    function begin(targetGame) {
        if (running)
            return;
        show(targetGame);
        frame.opacity = 0.0;
        frame.artOpacity = 1.0;
        frame.artScale = 1.06;
        frame.dim = 0.0;
        frame.captionBottom = restBottom;
        sequence.start();
    }

    function reset() {
        enter.stop();
        holding = false;
        quitting = false;
        frame.opacity = 0.0;
        frame.artOpacity = 1.0;
        frame.dim = 0.0;
        frame.captionBottom = restBottom;
        panel.opacity = 0.0;
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

    function enterPlaying() {
        if (!game) {
            show(api.allGames.byId(session.id));
            frame.opacity = 0.0;
            frame.artOpacity = 0.0;
            frame.artScale = 1.0;
        }
        quitting = false;
        holding = false;
        syncElapsed();
        frame.dim = artDim;
        enter.restart();
        forceActiveFocus();
    }

    function syncElapsed() {
        var started = session && session.started_at ? Date.parse(session.started_at) : NaN;
        elapsed = isNaN(started) ? 0 : Math.max(0, Math.round((Date.now() - started) / 1000));
    }

    function quit() {
        if (!sessionRunning || quitting)
            return;
        quitting = true;
        holding = false;
        Sound.cancel();
        api.universe.stop(session.session_id);
    }

    onPlayingChanged: {
        if (playing)
            enterPlaying();
        else if (!sessionRunning && !sequence.running && !exit.running && frame.opacity > 0.001)
            exit.restart();
    }

    // A session found at startup: the focus is only ours once the page loaders and the window exist.
    Component.onCompleted: if (playing) Qt.callLater(enterPlaying)

    onHoldingChanged: {
        if (holding) {
            fillUp.restart();
            holdFire.restart();
        } else {
            fillUp.stop();
            holdFire.stop();
            holdFill = 0.0;
        }
    }

    // A press the game's window swallowed the release of must not quit it a second later.
    readonly property bool windowActive: Window.active
    onWindowActiveChanged: if (!windowActive) holding = false

    Behavior on holdFill {
        enabled: !overlay.holding
        NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
    }

    Connections {
        target: api.universe
        function onLaunchFailed(id, message) {
            if (overlay.game && overlay.game.id === id && !overlay.sessionRunning)
                overlay.abort(message);
        }
        function onError(kind, message) {
            overlay.quitting = false;
        }
    }

    LaunchFrame {
        id: frame

        anchors.fill: parent
        opacity: 0.0
        visible: opacity > 0.001
    }

    Item {
        id: panel

        anchors.fill: parent
        opacity: 0.0
        visible: opacity > 0.001

        Row {
            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(Theme.edgeMargin)
            anchors.bottom: parent.bottom
            anchors.bottomMargin: frame.captionBottom + frame.logoHeight + Theme.dp(24)
            spacing: Theme.dp(14)

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(12)
                height: width
                radius: width / 2
                color: "#5fd48a"

                SequentialAnimation on opacity {
                    running: overlay.playing
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.3; duration: 900; easing.type: Easing.InOutQuad }
                    NumberAnimation { to: 1.0; duration: 900; easing.type: Easing.InOutQuad }
                }
            }

            CapsLabel {
                anchors.verticalCenter: parent.verticalCenter
                text: "PLAYING  ·  " + Format.clockTime(overlay.elapsed)
                color: Theme.textSecondary
                size: Theme.dp(21)
                tracking: 0.11
            }
        }

        PillButton {
            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(Theme.edgeMargin)
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.dp(Theme.hintBarHeight + 24)
            icon: "stop"
            label: overlay.quitting ? "Stopping…" : "Quit " + overlay.title
            focused: overlay.playing && !overlay.quitting
            dimmed: overlay.quitting
            fill: overlay.holdFill
        }

        HintBar {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            showClock: true
            hints: overlay.hints
        }
    }

    Timer {
        interval: 1000
        running: overlay.playing
        repeat: true
        onTriggered: overlay.syncElapsed()
    }

    NumberAnimation {
        id: fillUp
        target: overlay
        property: "holdFill"
        from: 0.0
        to: 1.0
        duration: overlay.quitHoldMs
    }

    Timer {
        id: holdFire
        interval: overlay.quitHoldMs
        onTriggered: overlay.quit()
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
                if (overlay.game)
                    overlay.game.launch();
            }
        }
    }

    // The running view: the art back up under its dim, the caption making room for the pill.
    ParallelAnimation {
        id: enter

        NumberAnimation {
            target: frame
            property: "opacity"
            to: 1.0
            duration: Theme.durScene
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: frame
            property: "artOpacity"
            to: 1.0
            duration: Theme.durLaunch
            easing.type: Easing.InOutQuad
        }
        NumberAnimation {
            target: frame
            property: "captionBottom"
            to: overlay.playBottom
            duration: Theme.durScene
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: panel
            property: "opacity"
            to: 1.0
            duration: Theme.durScene
            easing.type: Easing.OutCubic
        }
    }

    SequentialAnimation {
        id: exit

        ScriptAction {
            script: {
                enter.stop();
                overlay.holding = false;
                overlay.ended(overlay.game);
            }
        }
        ParallelAnimation {
            NumberAnimation {
                target: frame
                property: "opacity"
                to: 0.0
                duration: Theme.durScene
                easing.type: Easing.OutCubic
            }
            NumberAnimation {
                target: panel
                property: "opacity"
                to: 0.0
                duration: Theme.durQuick
                easing.type: Easing.OutCubic
            }
        }
        ScriptAction { script: overlay.reset() }
    }

    Keys.onPressed: function(event) {
        event.accepted = true;
        if (event.isAutoRepeat || !overlay.playing || overlay.quitting)
            return;
        if (api.keys.isAccept(event))
            overlay.holding = true;
    }

    Keys.onReleased: function(event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event))
            overlay.holding = false;
    }
}
