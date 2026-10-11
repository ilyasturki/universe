import QtQuick
import QtMultimedia

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
            readonly property alias source: player.source

            MediaPlayer {
                id: player

                source: root.file
                loops: MediaPlayer.Infinite
                videoOutput: video
                Component.onCompleted: play()
            }
        }
    }
}
