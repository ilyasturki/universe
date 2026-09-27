import QtQuick

// Over its view, as a sibling: the view's contentX/Y Behavior comes as `ease`, held off while the finger has the view.
Item {
    id: swipe

    property Flickable flickable: null
    property bool horizontal: false
    property var ease: null

    property real goal: 0
    property real placed: 0

    anchors.fill: flickable

    function low() {
        return horizontal ? flickable.originX - flickable.leftMargin : flickable.originY - flickable.topMargin;
    }

    function high() {
        var span = horizontal ? flickable.originX + flickable.contentWidth - flickable.width + flickable.rightMargin : flickable.originY + flickable.contentHeight - flickable.height + flickable.bottomMargin;
        return Math.max(low(), span);
    }

    function at() {
        return horizontal ? flickable.contentX : flickable.contentY;
    }

    function put(value) {
        placed = value;
        if (horizontal)
            flickable.contentX = value;
        else
            flickable.contentY = value;
    }

    function clamp(value) {
        return Math.max(low(), Math.min(high(), value));
    }

    function halt() {
        mover.stop();
        if (ease)
            ease.enabled = true;
    }

    DragHandler {
        id: drag

        property real last: 0

        target: null
        acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse
        acceptedButtons: Qt.LeftButton
        xAxis.enabled: swipe.horizontal
        yAxis.enabled: !swipe.horizontal
        // Takes over from a tap, never hands a drag it holds to another view's.
        grabPermissions: PointerHandler.CanTakeOverFromItems | PointerHandler.CanTakeOverFromHandlersOfDifferentType
        readonly property real along: swipe.horizontal ? centroid.scenePosition.x : centroid.scenePosition.y

        onActiveChanged: {
            if (active) {
                mover.stop();
                if (swipe.ease)
                    swipe.ease.enabled = false;
                last = along;
                return;
            }
            var speed = swipe.horizontal ? centroid.velocity.x : centroid.velocity.y;
            swipe.goal = swipe.clamp(swipe.at() - speed * 0.3);
            swipe.placed = swipe.at();
            mover.start();
        }
        onAlongChanged: {
            if (!active)
                return;
            swipe.put(swipe.clamp(swipe.at() - (along - last)));
            last = along;
        }
    }

    // Closes a fixed share of what is left each frame; a view moved by anything else (the keys) ends it there.
    FrameAnimation {
        id: mover

        onTriggered: {
            var left = swipe.goal - swipe.at();
            if (swipe.at() !== swipe.placed || Math.abs(left) < 0.5) {
                if (swipe.at() === swipe.placed)
                    swipe.put(swipe.goal);
                swipe.halt();
                return;
            }
            swipe.put(swipe.at() + left * (1 - Math.exp(-frameTime / 0.22)));
        }
    }
}
