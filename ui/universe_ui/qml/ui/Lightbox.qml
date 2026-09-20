import QtQuick
import "../core"

Rectangle {
    id: root

    property var images: []
    property int index: 0
    property bool open: false

    color: Qt.rgba(0.02, 0.02, 0.03, 0.94)
    opacity: open ? 1.0 : 0.0
    visible: opacity > 0.01

    Behavior on opacity {
        Ease {}
    }

    Item {
        id: stage

        anchors.fill: parent
        anchors.margins: Theme.dp(48)
        anchors.bottomMargin: Theme.dp(96)
        clip: true
        scale: root.open ? 1.0 : 0.96

        readonly property real pitch: width + Theme.dp(64)

        Behavior on scale {
            Ease {
                duration: Theme.durScene
            }
        }

        Item {
            id: strip

            width: parent.width
            height: parent.height
            x: -root.index * stage.pitch

            Behavior on x {
                // Off while closed: opening lands on the picture, it does not slide to it.
                enabled: root.open
                Ease {
                    duration: Theme.durView
                }
            }

            // Three slots keyed by residue: the slot that just left the screen picks up the picture two steps ahead.
            Repeater {
                model: 3

                Image {
                    readonly property int base: root.index - 1
                    readonly property int at: base + (((index - base) % 3) + 3) % 3

                    x: at * stage.pitch
                    width: stage.width
                    height: stage.height
                    source: at >= 0 && at < root.images.length ? root.images[at] : ""
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    sourceSize.width: 1920
                    opacity: at === root.index ? 1.0 : 0.4

                    Behavior on opacity {
                        Ease {
                            duration: Theme.durView
                        }
                    }
                }
            }
        }
    }

    Text {
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(34)
        anchors.horizontalCenter: parent.horizontalCenter
        text: (root.index + 1) + " / " + root.images.length
        color: Theme.textSecondary
        font.family: Theme.sans
        font.weight: Font.Medium
        font.pixelSize: Theme.dp(22)
    }
}
