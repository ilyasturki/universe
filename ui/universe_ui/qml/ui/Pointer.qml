import QtQuick
import "../core"

// The mouse over its parent. `hovered` fires once per entry under a moving mouse — a row sliding under a still cursor is not one —
// and a left click holds A as long as the button is, so a long click opens the game menu as a held A does.
Item {
    id: root

    property bool accept: true

    signal hovered

    anchors.fill: parent

    property bool landed: false

    function land() {
        if (!enabled || !hover.hovered) {
            landed = false;
            return;
        }
        if (landed || api.keys.mode !== "mouse")
            return;
        landed = true;
        Theme.pointed(parent);
        hovered();
    }

    HoverHandler {
        id: hover
        enabled: root.enabled
        onHoveredChanged: root.land()
    }

    Connections {
        target: api.keys
        function onMotionChanged() {
            root.land();
        }
    }

    TapHandler {
        enabled: root.enabled
        acceptedButtons: Qt.LeftButton
        gesturePolicy: TapHandler.ReleaseWithinBounds
        onPressedChanged: {
            if (pressed)
                root.land();
            if (!root.accept)
                return;
            pressed ? api.keys.hold("Accept") : api.keys.release("Accept");
        }
    }
}
