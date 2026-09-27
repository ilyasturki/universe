import QtQuick
import "../core"

// A tile of a Store collection: near-square corners, a small dark tag top left as the console's "Sponsored",
// a download's bar along the foot.
Item {
    id: tile

    property var entry: null
    property bool focused: false
    // How far the download under way (or the one stopped) got, 0–1.
    property real fraction: 0

    signal picked

    readonly property bool inProgress: entry !== null && (entry.busy || entry.partial)
    readonly property string tag: !entry ? "" : entry.busy ? entry.status : entry.partial ? "Paused" : entry.pending ? "Update" : entry.installed ? "Installed" : ""

    StoreArt {
        anchors.fill: parent
        entry: tile.entry
        radius: Theme.dp(4)
    }

    Rectangle {
        x: Theme.dp(12)
        y: Theme.dp(12)
        visible: tile.tag !== ""
        width: tagText.implicitWidth + Theme.dp(20)
        height: tagText.implicitHeight + Theme.dp(8)
        radius: Theme.dp(2)
        color: Qt.rgba(0.09, 0.09, 0.11, 0.88)

        Label {
            id: tagText
            anchors.centerIn: parent
            text: tile.tag
            color: tile.entry && (tile.entry.pending || tile.entry.busy) ? "#9cc8ff" : Theme.text
            font.pixelSize: Theme.dp(18)
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: Theme.dp(12)
        height: Theme.dp(5)
        radius: height / 2
        visible: tile.inProgress
        color: Qt.rgba(1, 1, 1, 0.28)

        Rectangle {
            width: parent.width * Math.max(0, Math.min(1, tile.fraction))
            height: parent.height
            radius: parent.radius
            color: Theme.text

            Behavior on width {
                NumberAnimation {
                    duration: Theme.durQuick
                }
            }
        }
    }

    FocusFrame {
        target: tile
        shown: tile.focused
        radius: Theme.dp(4)
        gap: Theme.dp(4)
        line: Theme.dp(3)
    }

    Touch {
        current: tile.focused
        onPicked: tile.picked()
    }
}
