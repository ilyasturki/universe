import QtQuick
import Qt5Compat.GraphicalEffects
import "../core"

Item {
    id: tile

    property var game: null
    property bool focused: false
    property bool lift: true
    property real cornerRadius: Theme.dp(Theme.radiusTile)
    property bool outlineShown: true

    readonly property bool empty: game === null || game === undefined
    readonly property bool software: GraphicsInfo.api === GraphicsInfo.Software

    readonly property url squareSource: empty ? "" : game.assets.square
    readonly property url bannerSource: empty ? "" : game.assets.tile
    readonly property url boxSource: empty ? "" : game.assets.boxFront
    // The banner only stands in when it is square (a Pegasus tile); SteamGridDB's is 920×430.
    readonly property bool bannerSquare: banner.status === Image.Ready && banner.implicitWidth > 0
                                         && Math.abs(banner.implicitWidth / banner.implicitHeight - 1) < 0.08
    readonly property string shown: String(squareSource) !== "" && square.status !== Image.Error ? "square"
                                  : bannerSquare ? "banner"
                                  : String(boxSource) !== "" && box.status !== Image.Error ? "box" : "none"

    scale: focused && lift ? Theme.liftScale : 1.0
    z: focused ? 2 : 1

    Behavior on scale {
        NumberAnimation { duration: Theme.durLift; easing.type: Easing.OutCubic }
    }

    Item {
        id: body

        anchors.fill: parent
        layer.enabled: !tile.software
        layer.smooth: true
        layer.effect: tile.software ? null : maskEffect

        Rectangle {
            anchors.fill: parent
            radius: tile.software ? tile.cornerRadius : 0
            color: tile.empty ? Theme.slot : Theme.artShade
        }

        Image {
            id: square
            anchors.fill: parent
            source: tile.squareSource
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            mipmap: true
            sourceSize.width: 512
            sourceSize.height: 512
            visible: tile.shown === "square"
        }

        Image {
            id: banner
            anchors.fill: parent
            source: String(tile.squareSource) === "" || square.status === Image.Error ? tile.bannerSource : ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            mipmap: true
            sourceSize.width: 512
            visible: tile.shown === "banner"
        }

        Image {
            id: box
            anchors.fill: parent
            // Not derived from `shown`: that reads box.status back (binding loop).
            source: (String(tile.squareSource) === "" || square.status === Image.Error) && !tile.bannerSquare ? tile.boxSource : ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            mipmap: true
            sourceSize.height: 512
            visible: tile.shown === "box"
        }

        Text {
            anchors.centerIn: parent
            width: parent.width - Theme.dp(40)
            visible: !tile.empty && tile.shown === "none"
            text: tile.empty ? "" : tile.game.title
            color: Theme.artInk
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            maximumLineCount: 4
            elide: Text.ElideRight
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(34)
        }
    }

    Component {
        id: maskEffect
        OpacityMask { maskSource: mask }
    }

    Rectangle {
        id: mask
        anchors.fill: parent
        radius: tile.cornerRadius
        color: "white"
        antialiasing: true
        visible: false
    }

    FocusOutline {
        target: body
        cornerRadius: tile.cornerRadius
        shown: tile.focused && tile.outlineShown
    }
}
