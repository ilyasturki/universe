import QtQuick
import "../../core" as Base
import "../core"

// The console's notification: top right, dark glass, an icon and one line.
Item {
    id: toast

    readonly property bool shown: Base.Notices.current !== null
    readonly property string icon: {
        var key = Base.Notices.current ? String(Base.Notices.current.key || "") : "";
        return Base.Notices.error ? "info" : key === "stop" ? "gamepad" : key === "power" ? "power" : key === "bolt" ? "bolt" : key.indexOf("journal") === 0 ? "journal" : key === "shot" ? "capture" : "bell";
    }

    anchors.right: parent ? parent.right : undefined
    anchors.rightMargin: shown ? Theme.dp(60) : Theme.dp(20)
    y: Theme.dp(36)
    width: Math.min(row.implicitWidth + Theme.dp(56), parent ? parent.width - Theme.dp(200) : 0)
    height: Theme.dp(88)
    opacity: shown ? 1.0 : 0.0
    visible: opacity > 0.01

    Behavior on opacity {
        NumberAnimation {
            duration: Theme.durChrome
            easing.type: Easing.OutCubic
        }
    }
    Behavior on anchors.rightMargin {
        NumberAnimation {
            duration: Theme.durPage
            easing.type: Easing.OutCubic
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(Theme.radiusCard)
        color: Theme.glassHigh
        border.width: 1
        border.color: Base.Notices.error ? Theme.alpha(Theme.danger, 0.7) : Theme.glassEdge
    }

    Row {
        id: row
        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(26)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(20)

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(48)
            height: width
            radius: Theme.dp(8)
            color: Base.Notices.error ? Theme.alpha(Theme.danger, 0.22) : Qt.rgba(1, 1, 1, 0.1)

            Glyph {
                anchors.centerIn: parent
                width: Theme.dp(28)
                height: width
                kind: toast.icon
                tint: Base.Notices.error ? Theme.danger : Theme.text
            }
        }

        Label {
            id: label
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, (toast.parent ? toast.parent.width : 1920) - Theme.dp(360))
            text: Base.Notices.text
            elide: Text.ElideRight
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }
}
