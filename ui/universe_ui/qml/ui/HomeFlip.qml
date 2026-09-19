import QtQuick
import "../core"

// gamescope swaps windows in one cut: the game's last frame bridges it.
Item {
    id: flip

    property rect tile: Qt.rect(0, 0, 0, 0)
    property bool covering: false
    property bool pending: false
    property var then: null
    readonly property bool running: shrink.running || grow.running || settle.running
    readonly property bool growing: grow.running

    readonly property rect fallback: Qt.rect((width - Theme.dp(240)) / 2, (height - Theme.dp(240)) / 2, Theme.dp(240), Theme.dp(240))

    visible: covering || running
    enabled: false

    function cover(rect) {
        shrink.stop();
        grow.stop();
        settle.stop();
        tile = rect || fallback;
        shrink.fades = !rect;
        place(Qt.rect(0, 0, width, height), 0, 1.0);
        covering = true;
        pending = true;
        // The binding to the new frame may not have run yet: a stale Ready is not painted.
        if (String(frame.source) === api.home.frame)
            frame.statusChanged(frame.status);
    }

    function fromTile(rect, done) {
        var midway = shrink.running;
        shrink.stop();
        settle.stop();
        pending = false;
        then = done;
        if (!midway)
            place(rect || fallback, Theme.dp(Theme.radiusTile), rect ? 1.0 : 0.0);
        covering = true;
        grow.start();
    }

    function place(rect, radius, opacity) {
        box.x = rect.x;
        box.y = rect.y;
        box.width = rect.width;
        box.height = rect.height;
        box.radius = radius;
        box.opacity = opacity;
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
        opacity: box.width >= flip.width ? 1.0 : 0.0

        Behavior on opacity { Ease { duration: Theme.durView } }
    }

    RoundedMask {
        id: box

        Image {
            id: frame
            anchors.fill: parent
            source: api.home.frame
            fillMode: Image.PreserveAspectCrop
            sourceSize.width: 1920
            asynchronous: true
            onStatusChanged: {
                if (!flip.pending)
                    return;
                if (status === Image.Ready)
                    painted.restart();
                else if (status === Image.Error) {
                    flip.pending = false;
                    flip.covering = false;
                    api.home.covered();
                }
            }
        }
    }

    // One frame past the decode: what gamescope shows at the swap is this, not the launcher beneath.
    Timer {
        id: painted
        interval: 35
        onTriggered: {
            flip.pending = false;
            api.home.covered();
            shrink.start();
        }
    }

    SequentialAnimation {
        id: shrink

        property bool fades: false

        PauseAnimation { duration: 90 }
        ParallelAnimation {
            NumberAnimation { target: box; property: "x"; to: flip.tile.x; duration: Theme.durScene; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "y"; to: flip.tile.y; duration: Theme.durScene; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "width"; to: flip.tile.width; duration: Theme.durScene; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "height"; to: flip.tile.height; duration: Theme.durScene; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "radius"; to: Theme.dp(Theme.radiusTile); duration: Theme.durScene; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "opacity"; to: shrink.fades ? 0.0 : 1.0; duration: Theme.durScene; easing.type: Easing.InCubic }
        }
        ScriptAction { script: flip.covering = false; }
    }

    SequentialAnimation {
        id: grow

        ParallelAnimation {
            NumberAnimation { target: box; property: "x"; to: 0; duration: Theme.durView; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "y"; to: 0; duration: Theme.durView; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "width"; to: flip.width; duration: Theme.durView; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "height"; to: flip.height; duration: Theme.durView; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "radius"; to: 0; duration: Theme.durView; easing.type: Easing.InOutCubic }
            NumberAnimation { target: box; property: "opacity"; to: 1.0; duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
        ScriptAction {
            script: {
                var done = flip.then;
                flip.then = null;
                if (done)
                    done();
                settle.restart();
            }
        }
    }

    // The swap takes a few frames; past that the launcher is either hidden or back, so the frame lets go.
    Timer {
        id: settle
        interval: 300
        onTriggered: flip.covering = false
    }
}
