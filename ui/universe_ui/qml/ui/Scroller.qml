import QtQuick
import "../core"

Item {
    id: scroller

    // Declared in a Flickable, a child lands on its contentItem.
    property Flickable view: parent instanceof Flickable ? parent : parent.parent
    property bool horizontal: false
    property bool nested: false
    property real step: Theme.dp(120)
    // The view sets highlightFollowsCurrentItem: false, or Qt drags it after the cursor at its own 400 px/s as well.
    property bool follow: view instanceof ListView || view instanceof GridView
    readonly property real span: horizontal ? view.width : view.height
    property real rangeBegin: 0
    property real rangeEnd: span

    property real goal: 0
    property bool landing: false

    anchors.fill: parent

    function low() {
        return horizontal ? view.originX - view.leftMargin : view.originY - view.topMargin;
    }

    function high() {
        var extent = horizontal ? view.originX + view.contentWidth - view.width + view.rightMargin : view.originY + view.contentHeight - view.height + view.bottomMargin;
        return Math.max(low(), extent);
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

    function heading() {
        return slide.running || mover.running ? goal : at();
    }

    function halt() {
        slide.stop();
        mover.stop();
    }

    function place(value) {
        halt();
        goal = clamp(value);
        put(goal);
    }

    // False when the content is already there or heading there: the page plays its edge.
    function to(value) {
        var target = clamp(value);
        if (Math.abs(target - heading()) < 0.5)
            return false;
        halt();
        goal = target;
        slide.from = at();
        slide.to = target;
        slide.start();
        return true;
    }

    function by(delta) {
        return to(heading() + delta);
    }

    function reveal(start, end, now) {
        var base = heading();
        var target = base;
        if (end > base + rangeEnd)
            target = end - rangeEnd;
        if (start < target + rangeBegin)
            target = start - rangeBegin;
        if (now)
            place(target);
        else
            to(target);
    }

    function show(index, now) {
        if (!view || index < 0 || index >= view.count || span <= 0)
            return;
        var was = at();
        var item = view.itemAtIndex(index);
        if (!item) {
            view.positionViewAtIndex(index, ListView.Contain);
            item = view.itemAtIndex(index);
        }
        var start = item ? (horizontal ? item.x : item.y) : 0;
        var size = item ? (horizontal ? item.width : item.height) : 0;
        if (at() !== was)
            put(was);
        if (item)
            reveal(start, start + size, now);
    }

    // A cursor moved in the same turn as the model, or as the view's size, lands at once.
    function land() {
        if (landing)
            return;
        landing = true;
        Qt.callLater(function () {
            landing = false;
            if (scroller.view)
                scroller.show(scroller.view.currentIndex, true);
        });
    }

    onSpanChanged: if (follow)
        land()
    Component.onCompleted: if (follow)
        land()

    Connections {
        target: scroller.follow ? scroller.view : null

        function onCurrentIndexChanged() {
            if (!scroller.landing)
                scroller.show(scroller.view.currentIndex, false);
        }
        function onCountChanged() {
            scroller.land();
        }
        function onModelChanged() {
            scroller.land();
        }
    }

    function roll(angle, pixels) {
        if (!view)
            return;
        if (pixels !== 0) {
            halt();
            put(clamp(at() - pixels));
            return;
        }
        if (angle === 0)
            return;
        var base = heading();
        slide.stop();
        goal = clamp(base - angle / 120 * step);
        mover.start();
    }

    // A handler takes one orientation: two of them, the y one leaving a nested strip's page what it does not take.
    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        orientation: Qt.Vertical
        blocking: !scroller.nested
        onWheel: function (event) {
            var shifted = event.modifiers & Qt.ShiftModifier;
            if (scroller.horizontal ? (shifted || !scroller.nested) : !shifted)
                scroller.roll(event.angleDelta.y, event.pixelDelta.y);
        }
    }

    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        orientation: Qt.Horizontal
        enabled: scroller.horizontal
        onWheel: function (event) {
            scroller.roll(event.angleDelta.x, event.pixelDelta.x);
        }
    }

    DragHandler {
        id: drag

        property real last: 0

        target: null
        acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse
        acceptedButtons: Qt.LeftButton
        xAxis.enabled: scroller.horizontal
        yAxis.enabled: !scroller.horizontal
        // Scene coordinates: the view's content, this handler's parent among it, moves under the finger.
        readonly property real along: scroller.horizontal ? centroid.scenePosition.x : centroid.scenePosition.y

        onActiveChanged: {
            if (active) {
                scroller.halt();
                last = along;
                return;
            }
            var speed = scroller.horizontal ? centroid.velocity.x : centroid.velocity.y;
            scroller.goal = scroller.clamp(scroller.at() - speed * 0.3);
            mover.tau = 0.22;
            mover.start();
        }
        onAlongChanged: {
            if (!active)
                return;
            scroller.put(scroller.clamp(scroller.at() - (along - last)));
            last = along;
        }
    }

    NumberAnimation {
        id: slide

        target: scroller.view
        property: scroller.horizontal ? "contentX" : "contentY"
        duration: Theme.durView
        easing.type: Easing.OutQuint
    }

    // Closes a fixed share of what is left each frame, so notches in a row run into one motion.
    FrameAnimation {
        id: mover

        property real tau: 0.09

        onRunningChanged: if (!running)
            tau = 0.09
        onTriggered: {
            var left = scroller.goal - scroller.at();
            if (Math.abs(left) < 0.5) {
                scroller.put(scroller.goal);
                stop();
                return;
            }
            scroller.put(scroller.at() + left * (1 - Math.exp(-frameTime / tau)));
        }
    }
}
