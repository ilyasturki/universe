import QtQuick
import "../core"

Rectangle {
    id: dot

    property bool running: true

    width: Theme.dp(10)
    height: width
    radius: width / 2
    color: "#5fd48a"

    SequentialAnimation on opacity {
        running: dot.running
        loops: Animation.Infinite
        NumberAnimation { to: 0.3; duration: 900; easing.type: Easing.InOutQuad }
        NumberAnimation { to: 1.0; duration: 900; easing.type: Easing.InOutQuad }
    }
}
