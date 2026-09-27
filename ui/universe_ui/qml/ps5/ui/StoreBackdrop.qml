import QtQuick
import "../core"

// The Store's own ground: near black, a faint grid of panels behind the collections, as the console's store draws.
Item {
    id: root

    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop {
                position: 0.0
                color: "#16181c"
            }
            GradientStop {
                position: 1.0
                color: "#0b0c0f"
            }
        }
    }

    Repeater {
        model: Math.ceil(root.width / Theme.dp(395)) + 1

        Rectangle {
            x: Theme.dp(172) + index * Theme.dp(395)
            width: 1
            height: root.height
            color: Qt.rgba(1, 1, 1, 0.035)
        }
    }

    Repeater {
        model: 3

        Rectangle {
            y: Theme.dp(342) + index * Theme.dp(395)
            width: root.width
            height: 1
            color: Qt.rgba(1, 1, 1, 0.03)
        }
    }

    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop {
                position: 0.0
                color: Qt.rgba(0.05, 0.055, 0.07, 0.55)
            }
            GradientStop {
                position: 0.6
                color: Qt.rgba(0.05, 0.055, 0.07, 0)
            }
        }
    }
}
