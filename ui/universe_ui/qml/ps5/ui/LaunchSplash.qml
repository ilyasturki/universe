import QtQuick
import "../core"

// Play: the chrome goes, the game's art zooms to fill the screen and stays as the splash for the whole load, then the game cuts in.
Item {
    id: splash

    property var game: null
    property bool waiting: false
    property string launchedSession: ""
    readonly property bool running: sequence.running || waiting || settle.running

    signal finished
    signal failed(var game, string message)

    readonly property url art: {
        if (!game)
            return "";
        if (String(game.assets.background) !== "")
            return game.assets.background;
        var shots = game.assets.screenshotList;
        if (shots && shots.length > 0)
            return shots[0];
        if (String(game.assets.banner) !== "")
            return game.assets.banner;
        return game.assets.boxFront;
    }

    function begin(target) {
        if (running)
            return;
        game = target;
        launchedSession = "";
        picture.scale = 1.0;
        sequence.start();
    }

    function reset() {
        sequence.stop();
        settle.stop();
        waiting = false;
        launchedSession = "";
        frame.opacity = 0.0;
        game = null;
    }

    // The game is up: a hard cut, as the console's.
    function done() {
        reset();
        finished();
    }

    function abort(message) {
        var g = game;
        done();
        failed(g, message);
    }

    Connections {
        target: api.universe
        function onLaunched(sessionId, id) {
            if (splash.waiting && splash.game && splash.game.id === id)
                splash.launchedSession = sessionId;
        }
        function onSessionShown(sessionId, ok) {
            if (!splash.waiting || sessionId !== splash.launchedSession)
                return;
            if (ok)
                splash.done();
            else
                settle.restart();
        }
        function onLaunchFailed(id, message) {
            if (splash.game && splash.game.id === id)
                splash.abort(message);
        }
        function onSessionEnded(sessionId, id, duration) {
            if (splash.waiting && sessionId === splash.launchedSession)
                splash.done();
        }
    }

    Rectangle {
        id: frame

        anchors.fill: parent
        color: "#000000"
        opacity: 0.0
        visible: opacity > 0.001

        Image {
            id: picture
            anchors.fill: parent
            source: splash.art
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            smooth: true
            sourceSize.width: 1920
        }

        // No art: the title on black, as a disc with no splash of its own.
        Label {
            anchors.centerIn: parent
            visible: picture.status !== Image.Ready
            width: parent.width - Theme.dp(400)
            text: splash.game ? splash.game.title : ""
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
            font.weight: Font.Light
            font.pixelSize: Theme.dp(56)
        }
    }

    SequentialAnimation {
        id: sequence

        PauseAnimation {
            duration: Theme.durChrome
        }
        ParallelAnimation {
            NumberAnimation {
                target: frame
                property: "opacity"
                to: 1.0
                duration: Theme.durSplash
                easing.type: Easing.InOutQuad
            }
            NumberAnimation {
                target: picture
                property: "scale"
                from: 1.0
                to: 1.06
                duration: Theme.durSplash * 6
                easing.type: Easing.OutCubic
            }
            SequentialAnimation {
                PauseAnimation {
                    duration: Theme.durSplash
                }
                ScriptAction {
                    script: {
                        if (splash.game) {
                            splash.waiting = true;
                            splash.game.launch();
                        }
                    }
                }
            }
        }
    }

    // Nobody could tell when the window came up (no shell extension): the splash holds this long.
    Timer {
        id: settle
        interval: 1500
        onTriggered: splash.done()
    }
}
