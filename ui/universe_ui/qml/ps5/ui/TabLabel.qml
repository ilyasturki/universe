import QtQuick
import "../core"

// One tab of a bar, the console's way on every screen: the focused one boxed (a thin light line over a faint fill),
// the one shown in bright text once the focus moves on, the rest dim.
Item {
    id: tab
    objectName: "tabLabel"

    property string text: ""
    property bool current: false
    property bool focused: false
    property real fontSize: Theme.dp(Theme.fontTitle)
    property int weight: Font.Light
    // From the box's edge to the text: a bar that places its text at x places the tab at x - pad.
    property real pad: Theme.dp(28)
    property real boxHeight: Theme.dp(80)

    signal picked

    implicitWidth: label.implicitWidth + pad * 2
    implicitHeight: boxHeight

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(4)
        color: tab.focused ? Qt.rgba(1, 1, 1, 0.12) : "transparent"
        border.width: tab.focused ? Theme.dp(2) : 0
        border.color: Qt.rgba(1, 1, 1, 0.85)

        Behavior on color {
            ColorAnimation {
                duration: Theme.durFocus
            }
        }
    }

    Label {
        id: label
        anchors.centerIn: parent
        text: tab.text
        color: tab.current || tab.focused ? Theme.text : Qt.rgba(1, 1, 1, 0.5)
        font.weight: tab.weight
        font.pixelSize: tab.fontSize

        Behavior on color {
            ColorAnimation {
                duration: Theme.durFocus
            }
        }
    }

    Touch {
        current: tab.focused
        onPicked: tab.picked()
    }
}
