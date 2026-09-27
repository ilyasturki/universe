import QtQuick
import "../core"

// The console's buttons: a dark glass pill that turns white under focus, with a ring a few units out.
Item {
    id: button

    property string text: ""
    property string glyph: ""
    property bool focused: false
    property bool danger: false
    property bool round: false
    property real fontSize: Theme.dp(30)
    property bool touchCurrent: focused
    property bool direct: false

    signal picked

    readonly property color ink: focused ? Theme.onLight : danger ? Theme.danger : Theme.text

    implicitHeight: Theme.dp(Theme.buttonHeight)
    implicitWidth: round ? implicitHeight : Math.max(Theme.dp(220), row.implicitWidth + Theme.dp(96))

    // A press: the white goes grey a beat, as the console's Play does before its splash.
    function flash() {
        pressAnim.restart();
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: -Theme.dp(6)
        radius: height / 2
        color: "transparent"
        border.width: Theme.dp(2)
        border.color: Qt.rgba(1, 1, 1, 0.42)
        opacity: button.focused ? 1.0 : 0.0

        Behavior on opacity {
            Ease {}
        }
    }

    Rectangle {
        id: fill
        anchors.fill: parent
        radius: height / 2
        color: button.focused ? Theme.buttonFocus : Theme.button

        Behavior on color {
            ColorAnimation {
                duration: Theme.durFocus
                easing.type: Easing.OutCubic
            }
        }

        Rectangle {
            id: press
            anchors.fill: parent
            radius: parent.radius
            color: "#9a9ca3"
            opacity: 0.0
        }
    }

    SequentialAnimation {
        id: pressAnim
        NumberAnimation {
            target: press
            property: "opacity"
            to: 0.8
            duration: 50
        }
        NumberAnimation {
            target: press
            property: "opacity"
            to: 0.0
            duration: 260
            easing.type: Easing.InQuad
        }
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.dp(14)

        Glyph {
            anchors.verticalCenter: parent.verticalCenter
            visible: button.glyph !== ""
            width: button.round ? Theme.dp(34) : Theme.dp(30)
            height: width
            kind: button.glyph
            tint: button.ink
        }

        Label {
            anchors.verticalCenter: parent.verticalCenter
            visible: button.text !== ""
            text: button.text
            color: button.ink
            font.weight: Font.DemiBold
            font.pixelSize: button.fontSize

            Behavior on color {
                ColorAnimation {
                    duration: Theme.durFocus
                }
            }
        }
    }

    Touch {
        current: button.touchCurrent
        direct: button.direct
        onPicked: button.picked()
    }
}
