import QtQuick
import "../core"

// A picture full screen, its neighbours a slide away; a tap anywhere closes it.
Rectangle {
    id: viewer

    property var images: []
    property int index: 0
    property bool open: false
    property string caption: ""

    color: Qt.rgba(0.01, 0.01, 0.015, 0.97)
    opacity: open ? 1.0 : 0.0
    visible: opacity > 0.01

    Behavior on opacity {
        NumberAnimation {
            duration: Theme.durChrome
            easing.type: Easing.OutCubic
        }
    }

    Block {
        onTapped: if (viewer.open)
            api.keys.press("Cancel")
    }

    Item {
        id: stage

        anchors.fill: parent
        anchors.margins: Theme.dp(40)
        anchors.bottomMargin: Theme.dp(110)
        clip: true
        scale: viewer.open ? 1.0 : 0.97

        readonly property real pitch: width + Theme.dp(64)

        Behavior on scale {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }

        Item {
            width: parent.width
            height: parent.height
            x: -viewer.index * stage.pitch

            Behavior on x {
                // Off while closed: opening lands on the picture, it does not slide to it.
                enabled: viewer.open
                NumberAnimation {
                    duration: Theme.durPage
                    easing.type: Easing.OutCubic
                }
            }

            // Three slots keyed by residue: the slot that just left the screen takes the picture two steps ahead.
            Repeater {
                model: 3

                Image {
                    readonly property int base: viewer.index - 1
                    readonly property int at: base + (((index - base) % 3) + 3) % 3

                    x: at * stage.pitch
                    width: stage.width
                    height: stage.height
                    source: viewer.open && at >= 0 && at < viewer.images.length ? viewer.images[at] : ""
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    sourceSize.width: 1920
                    opacity: at === viewer.index ? 1.0 : 0.3
                }
            }
        }
    }

    Label {
        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(Theme.edge)
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(40)
        width: parent.width * 0.6
        text: viewer.caption
        elide: Text.ElideRight
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    Label {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(Theme.columnRight)
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(40)
        text: viewer.images.length > 0 ? (viewer.index + 1) + " / " + viewer.images.length : ""
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }
}
