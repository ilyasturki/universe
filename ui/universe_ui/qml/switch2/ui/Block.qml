import QtQuick

// A tap or a swipe here reaches nothing beneath.
Item {
    id: block

    signal tapped

    anchors.fill: parent

    TapHandler {
        acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse
        acceptedButtons: Qt.LeftButton
        gesturePolicy: TapHandler.ReleaseWithinBounds
        onTapped: block.tapped()
    }

    // A Swipe beneath gets the press too: this one, met first, holds the drag and hands it to none of them.
    DragHandler {
        target: null
        acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse
        acceptedButtons: Qt.LeftButton
        grabPermissions: PointerHandler.CanTakeOverFromItems | PointerHandler.CanTakeOverFromHandlersOfDifferentType
    }
}
