import QtQuick
import "../core"
import "../core/Format.js" as Format

Row {
    id: root

    property var game: null
    property bool showYear: true

    spacing: Theme.dp(18)

    Component {
        id: dot
        Rectangle {
            width: Theme.dp(5)
            height: Theme.dp(5)
            radius: width / 2
            color: Theme.textFaint
            anchors.verticalCenter: parent.verticalCenter
        }
    }

    PlatformIcon {
        id: platform
        visible: root.game !== null && root.game.collections.count > 0
        game: root.game
        size: Theme.dp(28)
        color: Theme.textSecondary
        anchors.verticalCenter: parent.verticalCenter
    }

    Loader {
        sourceComponent: platform.visible && (yearText.visible || playText.visible || lastText.visible) ? dot : null
        anchors.verticalCenter: parent.verticalCenter
    }

    Text {
        id: yearText
        visible: root.showYear && text !== ""
        text: root.game && root.game.releaseYear > 0 ? root.game.releaseYear : ""
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(24)
        anchors.verticalCenter: parent.verticalCenter
    }

    Loader {
        sourceComponent: yearText.visible && (playText.visible || lastText.visible) ? dot : null
        anchors.verticalCenter: parent.verticalCenter
    }

    Text {
        id: playText
        visible: text !== ""
        text: root.game ? Format.playTime(root.game.playTime) : ""
        color: Theme.text
        font.family: Theme.sans
        font.pixelSize: Theme.dp(24)
        anchors.verticalCenter: parent.verticalCenter
    }

    Loader {
        sourceComponent: playText.visible && lastText.visible ? dot : null
        anchors.verticalCenter: parent.verticalCenter
    }

    Text {
        id: lastText
        visible: text !== ""
        text: root.game ? Format.lastPlayed(root.game.lastPlayed) : ""
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(24)
        anchors.verticalCenter: parent.verticalCenter
    }
}
