import QtQuick
import "../core"

// The wheel scrolls its parent view a `step` per notch, the ring staying where it is. A vertical view takes the wheel's y;
// a `horizontal` one takes x, Shift+y and — unless `nested` in a page that scrolls, which then keeps it — plain y too.
// Views that ease their own contentX/Y set `smooth: false`, or pass the move through `slide(value)`.
Item {
    id: wheel

    // Declared in a Flickable, a child lands on its contentItem.
    property Flickable view: parent instanceof Flickable ? parent : parent.parent
    property bool horizontal: false
    property bool nested: false
    property real step: view && view.cellHeight !== undefined ? (horizontal ? view.cellWidth : view.cellHeight) : Theme.dp(160)
    property bool smooth: true
    property var slide: null

    property real goal: 0
    property real stamp: 0

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

    function roll(delta) {
        if (!view || delta === 0)
            return;
        // A goal set within one ease is still in flight; an older one may have been moved by the keys since.
        var now = Date.now();
        if (now - stamp > Theme.durView)
            goal = at();
        stamp = now;
        goal = Math.max(low(), Math.min(high(), goal - delta / 120 * step));
        mover.stop();
        if (slide) {
            slide(goal);
        } else if (smooth) {
            mover.from = at();
            mover.to = goal;
            mover.start();
        } else if (horizontal) {
            view.contentX = goal;
        } else {
            view.contentY = goal;
        }
    }

    // A handler takes one orientation: two of them, the y one leaving a nested strip's page what it does not take.
    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        orientation: Qt.Vertical
        blocking: !wheel.nested
        onWheel: function (event) {
            var shifted = event.modifiers & Qt.ShiftModifier;
            if (wheel.horizontal ? (shifted || !wheel.nested) : !shifted)
                wheel.roll(event.angleDelta.y);
        }
    }

    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        orientation: Qt.Horizontal
        enabled: wheel.horizontal
        onWheel: function (event) {
            wheel.roll(event.angleDelta.x);
        }
    }

    NumberAnimation {
        id: mover
        target: wheel.view
        property: wheel.horizontal ? "contentX" : "contentY"
        duration: Theme.durView
        easing.type: Easing.OutQuint
    }
}
