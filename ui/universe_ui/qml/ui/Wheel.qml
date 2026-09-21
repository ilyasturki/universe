import QtQuick

// The wheel over its parent as the keys it stands for: a notch is one Up or Down, `horizontal` one Left or Right.
WheelHandler {
    id: wheel

    property bool horizontal: false
    property real carried: 0

    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
    onWheel: function (event) {
        var delta = event.angleDelta.y !== 0 ? event.angleDelta.y : (horizontal ? -event.angleDelta.x : 0);
        carried += delta;
        while (Math.abs(carried) >= 120) {
            var up = carried > 0;
            carried -= up ? 120 : -120;
            api.keys.press(horizontal ? (up ? "Left" : "Right") : (up ? "Up" : "Down"));
        }
    }
}
