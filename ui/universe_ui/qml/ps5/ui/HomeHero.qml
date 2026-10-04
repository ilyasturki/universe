import QtQuick
import "../core"
import "../pages/Home.js" as Home

// The hero under the row: the game's logo, one line, Play and "…", and the tile at the right.
Item {
    id: hero

    // { kind, game, label, tagline, action }
    property var entry: null
    property string playLabel: "Play"
    property bool active: false
    // 0 the main button, 1 "…", 2 the side tile.
    property int index: 0
    property bool shown: true
    property bool sideShown: true
    readonly property bool isGame: entry !== null && entry.kind === "game" && entry.game !== null && entry.game !== undefined
    readonly property var game: isGame ? entry.game : null
    readonly property int buttons: isGame ? 3 : 1
    readonly property alias playButton: play
    readonly property alias moreButton: more

    signal pointed(int index)

    readonly property url logo: game && String(game.assets.logo) !== "" ? game.assets.logo : ""
    readonly property url sideArt: Home.sideArt(game)

    Item {
        id: main

        anchors.fill: parent
        opacity: hero.shown ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: hero.shown ? Theme.durHero : Theme.durHeroOut
                easing.type: Easing.OutCubic
            }
        }

        Item {
            id: logoBox
            x: Theme.dp(Theme.edge)
            y: Theme.dp(560)
            width: Math.min(Theme.dp(700), hero.width * 0.45)
            height: Theme.dp(160)

            Image {
                id: logoImage
                anchors.fill: parent
                source: hero.logo
                fillMode: Image.PreserveAspectFit
                horizontalAlignment: Image.AlignLeft
                verticalAlignment: Image.AlignBottom
                asynchronous: true
                smooth: true
                mipmap: true
                sourceSize.width: 900
                visible: status === Image.Ready
            }

            Label {
                anchors.bottom: parent.bottom
                width: Math.min(Theme.dp(1000), hero.width - x - Theme.dp(Theme.sideTile + Theme.columnRight + 60))
                visible: !logoImage.visible
                text: hero.isGame ? hero.game.title : hero.entry ? hero.entry.label || "" : ""
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
                lineHeight: 0.95
                font.weight: Font.Light
                font.pixelSize: Theme.dp(66)
            }
        }

        Label {
            x: Theme.dp(Theme.edge)
            y: Theme.dp(Theme.heroTagY) - height / 2
            width: Math.min(Theme.dp(1000), hero.width - x - Theme.dp(Theme.sideTile + Theme.columnRight + 60))
            text: hero.isGame ? Home.tagline(hero.game) : hero.entry ? hero.entry.tagline || "" : ""
            elide: Text.ElideRight
            color: Qt.rgba(1, 1, 1, 0.92)
            font.weight: Font.Light
            font.pixelSize: Theme.dp(30)
        }

        Row {
            x: Theme.dp(Theme.edge)
            y: Theme.dp(Theme.heroButtonsY)
            spacing: Theme.dp(18)

            PillButton {
                id: play
                width: Theme.dp(Theme.playWidth)
                text: hero.isGame ? hero.playLabel : hero.entry ? hero.entry.action || "Open" : "Open"
                focused: hero.active && hero.index === 0
                onPicked: hero.pointed(0)
            }

            PillButton {
                id: more
                visible: hero.isGame
                round: true
                glyph: "more"
                focused: hero.active && hero.index === 1
                onPicked: hero.pointed(1)
            }
        }
    }

    Item {
        id: side

        x: hero.width - Theme.dp(Theme.columnRight + Theme.sideTile)
        y: Theme.dp(622)
        width: Theme.dp(Theme.sideTile)
        height: Theme.dp(Theme.sideTile)
        visible: hero.isGame && opacity > 0.01
        opacity: hero.shown && hero.sideShown ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation {
                duration: hero.sideShown ? Theme.durSide : Theme.durHeroOut
                easing.type: Easing.OutCubic
            }
        }

        HubCard {
            anchors.fill: parent
            image: hero.sideArt
            tint: Qt.rgba(0.06, 0.07, 0.1, 0.9)
            focused: hero.active && hero.index === 2
            onPicked: hero.pointed(2)
        }

        Column {
            anchors.left: parent.left
            anchors.bottom: parent.bottom
            anchors.margins: Theme.dp(22)
            spacing: Theme.dp(12)

            Rectangle {
                width: tag.implicitWidth + Theme.dp(18)
                height: tag.implicitHeight + Theme.dp(10)
                color: Qt.rgba(0, 0, 0, 0.35)
                border.width: Theme.dp(1.5)
                border.color: Qt.rgba(1, 1, 1, 0.85)

                Label {
                    id: tag
                    anchors.centerIn: parent
                    text: hero.game ? (hero.game.runnerName || hero.game.source || "Game") : ""
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(20)
                }
            }

            Label {
                text: hero.game ? (hero.game.playTime > 0 ? "Played " + Home.hours(hero.game.playTime) : "Not played yet") : ""
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(22)
            }
        }
    }
}
