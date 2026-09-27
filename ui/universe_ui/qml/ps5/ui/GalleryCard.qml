import QtQuick
import "../core"
import "../../ui" as Base
import "../pages/Home.js" as Home

// A capture of the Media Gallery, as the console's: the picture cropped to the card, a recording's length top left,
// its play mark and the game's tile bottom left; a journal entry's heading over its picture.
Item {
    id: card

    // A row of api.screens.media: kind, gameId, url, image, thumb, durationText, title, excerpt, dateText.
    property var row: null
    property bool focused: false

    signal picked

    readonly property string kind: row ? row.kind : ""
    readonly property var game: row ? api.allGames.byId(row.gameId) : null
    // `version` is read so the card repaints when its thumbnail lands.
    readonly property string thumb: row && row.thumb ? (api.screens.thumbs.version, api.screens.thumbs.url(row.thumb)) : ""
    readonly property string picture: !row ? "" : kind === "shot" ? (thumb !== "" ? thumb : row.url) : kind === "recording" ? row.image || "" : thumb
    readonly property string fallback: game ? String(Home.art(game).source) : ""
    readonly property real radius: Theme.dp(4)

    Base.RoundedMask {
        anchors.fill: parent
        radius: card.radius

        Rectangle {
            anchors.fill: parent
            radius: Theme.software ? card.radius : 0
            color: Theme.artShade
        }

        Image {
            anchors.fill: parent
            source: card.picture !== "" ? card.picture : card.fallback
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            sourceSize.width: 640
            opacity: status === Image.Ready ? (card.picture !== "" ? 1.0 : 0.45) : 0.0

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.durHero
                }
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: parent.height * (card.kind === "journal" ? 0.8 : 0.4)
            gradient: Gradient {
                GradientStop {
                    position: 0.0
                    color: Qt.rgba(0, 0, 0, 0)
                }
                GradientStop {
                    position: 1.0
                    color: Qt.rgba(0, 0, 0, card.kind === "journal" ? 0.86 : 0.55)
                }
            }
        }
    }

    Rectangle {
        x: Theme.dp(16)
        y: Theme.dp(16)
        visible: card.kind === "recording" && card.row.durationText !== ""
        width: duration.implicitWidth + Theme.dp(20)
        height: Theme.dp(38)
        radius: Theme.dp(4)
        color: Qt.rgba(0, 0, 0, 0.72)

        Label {
            id: duration
            anchors.centerIn: parent
            text: card.row && card.row.durationText ? card.row.durationText : ""
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(21)
        }
    }

    Rectangle {
        x: Theme.dp(16)
        y: Theme.dp(16)
        visible: card.kind === "journal"
        width: Theme.dp(42)
        height: width
        radius: Theme.dp(8)
        color: "#ffffff"

        Glyph {
            anchors.centerIn: parent
            width: Theme.dp(28)
            height: width
            kind: "journal"
            tint: Theme.onLight
            stroke: 2
        }
    }

    Column {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: marks.top
        anchors.margins: Theme.dp(18)
        anchors.bottomMargin: Theme.dp(10)
        spacing: Theme.dp(4)
        visible: card.kind === "journal"

        Label {
            width: parent.width
            text: card.row && card.row.title ? card.row.title : ""
            elide: Text.ElideRight
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(26)
        }

        Label {
            width: parent.width
            visible: text !== ""
            text: card.row && card.row.excerpt ? card.row.excerpt : ""
            color: Theme.textSecondary
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
            lineHeight: 1.15
            font.pixelSize: Theme.dp(Theme.fontTiny)
        }
    }

    Row {
        id: marks

        x: Theme.dp(16)
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(14)
        spacing: Theme.dp(12)

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            visible: card.kind === "recording"
            width: Theme.dp(42)
            height: width
            radius: width / 2
            color: Qt.rgba(0, 0, 0, 0.45)
            border.width: Theme.dp(2)
            border.color: Qt.rgba(1, 1, 1, 0.85)

            Glyph {
                anchors.centerIn: parent
                anchors.horizontalCenterOffset: Theme.dp(1.5)
                width: Theme.dp(18)
                height: width
                kind: "play"
            }
        }

        TileArt {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(40)
            height: width
            radius: Theme.dp(6)
            game: card.game
            titleSize: Theme.dp(8)
        }

        Label {
            anchors.verticalCenter: parent.verticalCenter
            visible: card.kind === "journal"
            text: card.row ? card.row.dateText : ""
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontTiny)
        }
    }

    FocusFrame {
        target: card
        shown: card.focused
        radius: card.radius
        gap: Theme.dp(3)
        line: Theme.dp(3)
    }

    Touch {
        current: card.focused
        menu: true
        onPicked: card.picked()
    }
}
