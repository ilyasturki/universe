import QtQuick
import "../core"

Item {
    id: bar

    implicitHeight: Theme.dp(170)

    Rectangle {
        id: avatar

        x: Theme.dp(75)
        y: Theme.dp(69)
        width: Theme.dp(90)
        height: width
        radius: width / 2
        color: Theme.dark ? "#3a3a3a" : "#ffffff"
        border.width: Theme.dp(3)
        border.color: Theme.dark ? "#4a4a4a" : "#dcdcdc"

        Image {
            anchors.fill: parent
            anchors.margins: Theme.dp(14)
            source: Qt.resolvedUrl("../../../../icons/hicolor/scalable/apps/universe-ui.svg")
            fillMode: Image.PreserveAspectFit
            sourceSize.width: 128
            sourceSize.height: 128
            smooth: true
            mipmap: true
        }
    }

    Text {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(72)
        y: Theme.dp(90)
        text: Theme.clock
        color: Theme.text
        font.family: Theme.sans
        font.pixelSize: Theme.dp(36)
    }
}
