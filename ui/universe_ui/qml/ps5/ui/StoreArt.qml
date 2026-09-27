import QtQuick
import "../core"
import "../../ui" as Base

// A store entry's picture: the library's art once the game is installed, else the store's own, else its title on dark glass.
Item {
    id: art

    // A sources row: { title, image, … }.
    property var entry: null
    property real radius: Theme.dp(3)
    property real titleSize: Math.max(Theme.dp(14), Math.min(Theme.dp(30), width / 9))

    readonly property bool shown: picture.status === Image.Ready

    Base.RoundedMask {
        anchors.fill: parent
        radius: art.radius

        Rectangle {
            anchors.fill: parent
            radius: Theme.software ? art.radius : 0
            gradient: Gradient {
                GradientStop {
                    position: 0.0
                    color: "#2a2d35"
                }
                GradientStop {
                    position: 1.0
                    color: "#1a1c22"
                }
            }
        }

        Label {
            anchors.centerIn: parent
            width: parent.width - Theme.dp(24)
            visible: !art.shown
            text: art.entry ? art.entry.title : ""
            color: Theme.textSecondary
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 3
            elide: Text.ElideRight
            font.weight: Font.DemiBold
            font.pixelSize: art.titleSize
        }

        Image {
            id: picture
            anchors.fill: parent
            source: art.entry && art.entry.image ? art.entry.image : ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            smooth: true
            sourceSize.width: 720
            visible: status === Image.Ready
        }
    }
}
