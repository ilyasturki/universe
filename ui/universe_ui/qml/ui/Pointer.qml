import QtQuick
import "../core"
import "../sound"

// The mouse over its parent. Hovering only brightens it; a left click picks it — `picked` moves the ring there, with the pad's
// tick — and a click on the item that already holds the ring (`current`), or on a `direct` one such as a button, holds A as
// long as the button is, so a long click opens the game menu as a held A does.
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

    TapHandler {
        id: tap

        property bool holding: false

        enabled: root.enabled
        acceptedButtons: Qt.LeftButton
        gesturePolicy: TapHandler.ReleaseWithinBounds
        onPressedChanged: {
            if (pressed) {
                // Read before the pick: the binding flips as soon as the ring moves.
                var held = root.current;
                Theme.pointed(root.parent);
                if (!held) {
                    if (!root.direct)
                        Sound.tick();
                    root.picked();
                }
                if (root.accept && (held || root.direct)) {
                    holding = true;
                    api.keys.hold("Accept");
                }
            } else if (holding) {
                holding = false;
                api.keys.release("Accept");
            }
        }
    }
}
