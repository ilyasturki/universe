import QtQuick
import "../core"
import "../../ui" as Base

// A game's square art, rounded: the square asset, else the box front cropped, else its title on a plain tile.
Item {
    id: art

    property var game: null
    property real radius: Theme.dp(Theme.radiusTile)
    // A download's icon, faded until it is playable.
    property bool dimmed: false
    property real titleSize: Math.max(Theme.dp(14), Math.min(Theme.dp(30), width / 7))

    readonly property bool empty: game === null || game === undefined
    readonly property url squareSource: empty ? "" : game.assets.square
    readonly property url boxSource: empty ? "" : game.assets.boxFront
    readonly property string shown: String(squareSource) !== "" && square.status !== Image.Error ? "square" : String(boxSource) !== "" && box.status !== Image.Error ? "box" : "none"

    Base.RoundedMask {
        anchors.fill: parent
        radius: art.radius

        Rectangle {
            anchors.fill: parent
            radius: Theme.software ? art.radius : 0
            color: Theme.artShade
        }

        Image {
            id: square
            anchors.fill: parent
            source: art.squareSource
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            mipmap: true
            sourceSize.width: 512
            sourceSize.height: 512
            visible: art.shown === "square"
        }

        Image {
            id: box
            anchors.fill: parent
            // Not derived from `shown`: that reads box.status back (binding loop).
            source: String(art.squareSource) === "" || square.status === Image.Error ? art.boxSource : ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            mipmap: true
            sourceSize.height: 512
            visible: art.shown === "box"
        }

        Label {
            anchors.centerIn: parent
            width: parent.width - Theme.dp(16)
            visible: !art.empty && art.shown === "none"
            text: art.empty ? "" : art.game.title
            color: Theme.textSecondary
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 4
            elide: Text.ElideRight
            font.weight: Font.DemiBold
            font.pixelSize: art.titleSize
        }

        Rectangle {
            anchors.fill: parent
            color: Theme.ground
            opacity: art.dimmed ? 0.55 : 0
            visible: opacity > 0
        }
    }
}
