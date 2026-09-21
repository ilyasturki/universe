import QtQuick

// The wheel over a surface with nothing to scroll, as the keys it stands for: a notch is one Up or Down, `horizontal` one
// Left or Right, from the wheel's y and, sideways, its x.
Item {
    id: wheel

    property bool horizontal: false
    property real carried: 0

    anchors.fill: parent

    function roll(delta) {
        carried += delta;
        while (Math.abs(carried) >= 120) {
            var up = carried > 0;
            carried -= up ? 120 : -120;
            api.keys.press(horizontal ? (up ? "Left" : "Right") : (up ? "Up" : "Down"));
        }
    }

    WheelHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        orientation: Qt.Vertical
        onWheel: function (event) {
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
}
