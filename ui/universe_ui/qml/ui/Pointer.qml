import QtQuick
import "../core"
import "../sound"

// The mouse or a finger over its parent. Hovering only brightens it; a press picks it — `picked` moves the ring there, with
// the pad's tick — and on the item that already holds the ring (`current`), or on a `direct` one such as a button, it is A:
// pressed at the release, held from `holdDelay` on so a long press opens the game menu as a held A does. A press that
// turns into a drag (a view scrolled under the finger) is no A at all.
Item {
    id: root

    property bool accept: true
    property bool current: false
    property bool direct: false
    property real radius: 0
    property real wash: 0.07

    readonly property bool hovering: enabled && hover.hovered && api.keys.mode === "mouse"

    signal picked

    anchors.fill: parent
    z: 1

    Rectangle {
        anchors.fill: parent
        radius: root.radius
        color: "white"
        opacity: root.hovering ? root.wash : 0

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }
    }

    HoverHandler {
        id: hover
        enabled: root.enabled
    }

    // Long enough for a drag to declare itself, short enough that a press still feels like one.
    readonly property int holdDelay: 180

    TapHandler {
        id: tap

        // Read at the press: the binding flips as soon as the ring moves.
        property bool wasCurrent: false
        property bool armed: false
        property bool holding: false
        property bool done: false

        // A finger that lands to scroll moves no ring: the pick waits for the tap, or for the press to be held.
        function pick() {
            if (done)
                return;
            done = true;
            Theme.pointed(root.parent);
            if (!wasCurrent) {
                if (!root.direct)
                    Sound.tick();
                root.picked();
            }
        }

        function reset() {
            holdTimer.stop();
            armed = false;
            if (holding) {
                holding = false;
                api.keys.release("Accept");
            }
        }

        enabled: root.enabled
        acceptedButtons: Qt.LeftButton
        gesturePolicy: TapHandler.ReleaseWithinBounds
        onPressedChanged: {
            if (pressed) {
                wasCurrent = root.current;
                armed = root.accept && (wasCurrent || root.direct);
                done = false;
                holdTimer.restart();
            } else if (holding) {
                // A long press ends as a held A does; the tap that may follow the release is not a second A.
                reset();
            } else
                holdTimer.stop();
        }
        // Qt reports the tap before the release: a press held into A is not pressed again here.
        onTapped: {
            pick();
            if (armed && !holding)
                api.keys.press("Accept");
            armed = false;
        }
        onCanceled: reset()
    }

    Timer {
        id: holdTimer

        interval: root.holdDelay
        onTriggered: {
            if (!tap.pressed)
                return;
            tap.pick();
            if (tap.armed) {
                tap.holding = true;
                api.keys.hold("Accept");
            }
        }
    }
}
