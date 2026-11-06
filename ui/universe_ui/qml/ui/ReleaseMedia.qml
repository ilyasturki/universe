import QtQuick
import QtMultimedia
import "../core"

// Takes no room until its file is downloaded: offline, the entry reads as text.
Item {
    id: root

    objectName: "releaseMedia"

    required property var media
    required property var store

    readonly property string file: store.files[media.url] || ""

    height: file !== "" ? Math.round(width * 9 / 16) : 0
    visible: file !== ""
    Accessible.name: media.alt
    Accessible.role: media.kind === "clip" ? Accessible.Animation : Accessible.Graphic

    Component.onCompleted: store.fetch(media.url)

    Loader {
        anchors.fill: parent
        active: root.file !== ""
        sourceComponent: root.media.kind === "clip" ? clip : still
    }

    Component {
        id: still

        Image {
            objectName: "releaseStill"
            source: root.file
            fillMode: Image.PreserveAspectFit
            asynchronous: true
        }
    }

    Component {
        id: clip

        VideoOutput {
            id: video

            objectName: "releaseClip"
            property QtObject player: null
            readonly property url source: player ? player.source : ""

            Component.onCompleted: player = Clips.open(root.file, video)
            Component.onDestruction: Clips.release(player)
        }
    }
}
