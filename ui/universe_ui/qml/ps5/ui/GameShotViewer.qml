import QtQuick
import "../core"
import "../sound"

// A store screenshot at full size over the page: Left and Right step through them, B or A closes.
FocusScope {
    id: viewer

    property var images: []
    property int index: 0
    property bool open: false

    signal closed

    anchors.fill: parent
    visible: backdrop.opacity > 0.01
    focus: open

    function show(i) {
        index = Math.max(0, Math.min(images.length - 1, i));
        open = true;
        forceActiveFocus();
    }

    function close() {
        Sound.play("back");
        open = false;
        viewer.closed();
    }

    function step(d) {
        index = Sound.stepped(index, d, images.length);
    }

    Keys.onPressed: function (event) {
        if (!open)
            return;
        event.accepted = true;
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            step(event.key === Qt.Key_Left ? -1 : 1);
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isCancel(event) || api.keys.isAccept(event))
            close();
        else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down)
            Sound.play("edge");
    }

    Rectangle {
        id: backdrop
        anchors.fill: parent
        color: "#000000"
        opacity: viewer.open ? 0.96 : 0.0

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }
    }

    Image {
        id: picture
        anchors.fill: parent
        anchors.margins: Theme.dp(60)
        anchors.bottomMargin: Theme.dp(110)
        source: viewer.images.length > 0 ? viewer.images[viewer.index] : ""
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        smooth: true
        mipmap: true
        sourceSize.width: 1920
        opacity: viewer.open ? 1.0 : 0.0
        scale: viewer.open ? 1.0 : 0.97

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }
    }

    Label {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(44)
        visible: viewer.open && viewer.images.length > 1
        text: (viewer.index + 1) + " / " + viewer.images.length
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    Repeater {
        model: viewer.open && viewer.images.length > 1 ? [-1, 1] : []

        Glyph {
            x: modelData < 0 ? Theme.dp(20) : viewer.width - width - Theme.dp(20)
            anchors.verticalCenter: picture.verticalCenter
            width: Theme.dp(44)
            height: width
            kind: modelData < 0 ? "back" : "chevron"
            opacity: (modelData < 0 ? viewer.index > 0 : viewer.index < viewer.images.length - 1) ? 0.8 : 0.2
        }
    }

    // Left third back, right third on, the middle closes; nothing reaches the page beneath.
    Item {
        anchors.fill: parent
        enabled: viewer.open

        TapHandler {
            acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse
            acceptedButtons: Qt.LeftButton
            gesturePolicy: TapHandler.ReleaseWithinBounds
            onTapped: function (eventPoint, button) {
                var x = eventPoint.position.x;
                if (x < viewer.width / 3)
                    viewer.step(-1);
                else if (x > viewer.width * 2 / 3)
                    viewer.step(1);
                else
                    viewer.close();
            }
        }

        DragHandler {
            target: null
            acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse
            acceptedButtons: Qt.LeftButton
            grabPermissions: PointerHandler.CanTakeOverFromItems | PointerHandler.CanTakeOverFromHandlersOfDifferentType
        }
    }
}
