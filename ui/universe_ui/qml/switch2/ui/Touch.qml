import QtQuick

// At the release: `action` on the `current` or a `direct` item, only a pick elsewhere; held with `menu`, a pick and +.
Item {
    id: touch

    property bool current: false
    property bool direct: false
    property bool menu: false
    property string action: "Accept"

    signal picked

    anchors.fill: parent
    z: 1

    TapHandler {
        acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse
        acceptedButtons: Qt.LeftButton
        gesturePolicy: TapHandler.ReleaseWithinBounds
        // 0: no long press, a held tap is still a tap at the release.
        longPressThreshold: touch.menu ? 0.45 : 0
        onTapped: {
            var acts = touch.current || touch.direct;
            if (!touch.current)
                touch.picked();
            if (acts && touch.action !== "")
                api.keys.press(touch.action);
        }
        onLongPressed: {
            if (!touch.current)
                touch.picked();
            api.keys.press("Menu");
        }
    }
}
