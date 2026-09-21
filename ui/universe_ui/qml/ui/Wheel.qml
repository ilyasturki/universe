import QtQuick
import "../core"

// The wheel scrolls its parent view, the ring staying where it is. A notch adds a `step` to where the view is heading and
// the view eases after it every frame, so notches in a row run into one motion; a touchpad's pixels move it as they come.
// A vertical view takes the wheel's y; a `horizontal` one takes x, Shift+y and — unless `nested` in a page that scrolls,
// which then keeps it — plain y too. A view with a Behavior on its contentX/Y hands it over as `ease`, held off while rolling.
Item {
    id: wheel

    // Declared in a Flickable, a child lands on its contentItem.
    property Flickable view: parent instanceof Flickable ? parent : parent.parent
    property bool horizontal: false
    property bool nested: false
    property real step: Theme.dp(120)
    property var ease: null

    property real goal: 0

    anchors.fill: parent

    function low() {
        return horizontal ? view.originX - view.leftMargin : view.originY - view.topMargin;
    }

    function high() {
        var span = horizontal ? view.originX + view.contentWidth - view.width + view.rightMargin : view.originY + view.contentHeight - view.height + view.bottomMargin;
        return Math.max(low(), span);
    }

    function at() {
        return horizontal ? view.contentX : view.contentY;
    }

    function put(value) {
        if (horizontal)
            view.contentX = value;
        else
            view.contentY = value;
    }

    function clamp(value) {
        return Math.max(low(), Math.min(high(), value));
    }

    // The keys moved the view: whatever the wheel was heading for is off.
    function halt() {
        if (!mover.running)
            return;
        mover.stop();
        if (ease)
            ease.enabled = true;
    }

    function roll(angle, pixels) {
        if (!view)
            return;
        if (pixels !== 0) {
            halt();
            if (ease)
                ease.enabled = false;
            put(clamp(at() - pixels));
            if (ease)
                ease.enabled = true;
            return;
        }
        if (angle === 0)
            return;
        if (!mover.running) {
            goal = at();
            if (ease)
                ease.enabled = false;
        }
        goal = clamp(goal - angle / 120 * step);
        mover.start();
    }

    // A handler takes one orientation: two of them, the y one leaving a nested strip's page what it does not take.
    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        orientation: Qt.Vertical
        blocking: !wheel.nested
        onWheel: function (event) {
            var shifted = event.modifiers & Qt.ShiftModifier;
            if (wheel.horizontal ? (shifted || !wheel.nested) : !shifted)
                wheel.roll(event.angleDelta.y, event.pixelDelta.y);
        }
    }

    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        orientation: Qt.Horizontal
        enabled: wheel.horizontal
        onWheel: function (event) {
            wheel.roll(event.angleDelta.x, event.pixelDelta.x);
        }
    }

    // Closes a fixed share of what is left each frame: quick off the mark, settling without a stop-start between notches.
    FrameAnimation {
        id: mover

        readonly property real tau: 0.09

        onTriggered: {
            var left = wheel.goal - wheel.at();
            if (Math.abs(left) < 0.5) {
                wheel.put(wheel.goal);
                wheel.halt();
                return;
            }
            wheel.put(wheel.at() + left * (1 - Math.exp(-frameTime / tau)));
        }
    }
}
