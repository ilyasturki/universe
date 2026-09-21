import QtQuick
import "../core"

Item {
    id: root

    property string source: ""
    property string caption: ""
    property string subcaption: ""
    property bool journaled: false
    property bool focused: false
    property bool dimmed: false

    readonly property real captionHeight: caption === "" ? 0 : Theme.dp(subcaption === "" ? 34 : 58)
    readonly property real radius: Theme.dp(14)

    scale: focused ? 1.0 : 0.96
    opacity: dimmed ? Theme.idleOpacity : 1.0

    Behavior on scale {
        Ease {
            duration: Theme.durQuick
        }
    }
    Behavior on opacity {
        Ease {
            duration: Theme.durQuick
        }
    }

    FocusRing {
        anchors.fill: frame
        cornerRadius: root.radius
        opacity: root.focused ? 1.0 : 0.0
        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }
    }

    RoundedMask {
        id: frame
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: root.captionHeight
        radius: root.radius

        Rectangle {
            anchors.fill: parent
            color: root.source === "" ? Theme.surface : Theme.cardBase
        }

        Image {
            anchors.fill: parent
            source: root.source
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            sourceSize.width: 960
        }

        MenuGlyph {
            anchors.centerIn: parent
            width: Theme.dp(44)
            height: Theme.dp(44)
            visible: root.source === ""
            kind: "camera"
            tint: Theme.textFaint
        }

        Rectangle {
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: Theme.dp(10)
            width: Theme.dp(34)
            height: width
            radius: width / 2
            color: Qt.rgba(0, 0, 0, 0.55)
            visible: root.journaled

            MenuGlyph {
                anchors.centerIn: parent
                width: Theme.dp(20)
                height: Theme.dp(20)
                kind: "book"
                tint: "#f2f3f5"
            }
        }
    }

    Column {
        anchors.top: frame.bottom
        anchors.topMargin: Theme.dp(8)
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: Theme.dp(2)
        visible: root.caption !== ""

        Text {
            width: parent.width
            text: root.caption
            color: root.focused ? Theme.text : Theme.textSecondary
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(19)
            elide: Text.ElideRight
        }

        Text {
            width: parent.width
            visible: root.subcaption !== ""
            text: root.subcaption
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(17)
            elide: Text.ElideRight
        }
    }
}
