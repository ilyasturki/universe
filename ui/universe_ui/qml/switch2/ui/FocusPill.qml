import QtQuick
import "../core"

Rectangle {
    id: pill

    property bool focused: false

    radius: Theme.dp(Theme.radiusRow)
    color: focused ? Theme.focusFill : "transparent"
    visible: focused

    FocusOutline {
        target: pill
        cornerRadius: pill.radius
        gap: 0
        shown: pill.focused
    }
}
