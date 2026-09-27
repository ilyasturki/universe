import QtQuick
import "../core"

Item {
    id: line

    property var job: null
    property bool closable: false
    readonly property bool shown: job !== null && job !== undefined
    readonly property bool settled: shown && job.ok !== null && job.ok !== undefined
    readonly property real fraction: settled ? 1 : job && job.total > 0 ? job.done / job.total : 0

    signal closeRequested

    height: shown ? Theme.dp(90) : 0
    visible: shown

    Label {
        width: parent.width - (closer.visible ? closer.width + Theme.dp(12) : 0)
        text: line.job ? line.job.message + (line.job.ok === true ? " ✓" : line.job.ok === false && !line.job.cancelled ? " ✗" : "") : ""
        elide: Text.ElideRight
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    Item {
        id: closer
        objectName: "hideJob"

        anchors.right: parent.right
        width: Theme.dp(40)
        height: width
        visible: line.closable

        Glyph {
            anchors.centerIn: parent
            width: Theme.dp(24)
            height: width
            kind: "cross"
        }

        Touch {
            direct: true
            action: ""
            onPicked: line.closeRequested()
        }
    }

    Rectangle {
        y: Theme.dp(50)
        width: parent.width
        height: Theme.dp(8)
        radius: height / 2
        color: Theme.hairlineSoft

        Rectangle {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: parent.width * line.fraction
            radius: height / 2
            color: Theme.accentStrong

            Behavior on width {
                Ease {
                    duration: Theme.durQuick
                }
            }
        }
    }
}
