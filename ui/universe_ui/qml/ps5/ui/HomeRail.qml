import QtQuick
import "../core"
import "../pages/Home.js" as Home

// The home row: the focused tile large at a fixed place near the left edge, the rest sliding past it,
// the focused one's name beside it once the row has come to rest.
Item {
    id: rail

    // [{ kind, game, label }]
    property var entries: []
    property int current: 0
    property bool active: false
    property bool titleShown: true
    // Held off while the home builds itself back after a game: the tiles come in from the focused one out.
    property real reveal: 1.0

    signal pointed(int index)

    readonly property real small: Theme.dp(Theme.tileSize)
    readonly property real big: Theme.dp(Theme.tileFocus)
    readonly property real pitch: small + Theme.dp(Theme.tileGap)
    readonly property real anchorX: Theme.dp(Theme.tileFocusX)
    readonly property var entry: current >= 0 && current < entries.length ? entries[current] : null

    function slotX(i) {
        if (i === current)
            return anchorX;
        if (i < current)
            return anchorX - Theme.dp(15) - small - (current - i - 1) * pitch;
        return anchorX + big + Theme.dp(12) + (i - current - 1) * pitch;
    }

    height: big + Theme.dp(80)

    Repeater {
        model: rail.entries

        RailTile {
            id: tile

            readonly property int distance: Math.abs(index - rail.current)

            x: rail.slotX(index)
            width: index === rail.current ? rail.big : rail.small
            height: width
            game: modelData.game || null
            kind: modelData.kind
            focused: rail.active && index === rail.current
            opacity: rail.reveal >= 1 ? 1.0 : Math.max(0, Math.min(1, rail.reveal * 4 - distance * 0.35))
            visible: x + width > -rail.pitch && x < rail.width + rail.pitch

            Behavior on x {
                NumberAnimation {
                    duration: Theme.durMove
                    easing.type: Easing.OutCubic
                }
            }
            Behavior on width {
                NumberAnimation {
                    duration: Theme.durMove
                    easing.type: Easing.OutCubic
                }
            }

            Touch {
                current: tile.focused
                onPicked: rail.pointed(index)
            }
        }
    }

    Row {
        id: title

        readonly property var game: rail.entry && rail.entry.kind === "game" ? rail.entry.game : null
        readonly property string badge: Home.badge(game)

        x: rail.anchorX + rail.big + Theme.dp(12)
        y: Theme.dp(Theme.titleY - Theme.railY) - height / 2
        spacing: Theme.dp(12)
        opacity: rail.titleShown && rail.reveal >= 1 ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation {
                duration: rail.titleShown ? Theme.durQuick : 40
            }
        }

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            visible: title.badge !== ""
            width: badgeText.implicitWidth + Theme.dp(22)
            height: Theme.dp(32)
            radius: Theme.dp(4)
            color: "#ffffff"

            Label {
                id: badgeText
                anchors.centerIn: parent
                text: title.badge
                color: Theme.onLight
                font.weight: Font.DemiBold
                font.letterSpacing: Theme.dp(1)
                font.pixelSize: Theme.dp(19)
            }
        }

        Label {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, rail.width - title.x - Theme.dp(Theme.columnRight) - Theme.dp(120))
            text: rail.entry ? (title.game ? title.game.title : rail.entry.label || "") : ""
            elide: Text.ElideRight
            font.weight: Font.Light
            font.pixelSize: Theme.dp(34)
        }

        Label {
            anchors.verticalCenter: parent.verticalCenter
            visible: title.game !== null && title.game.installing
            text: title.game && title.game.installing ? (title.game.progress >= 0 ? "Installing · " + Math.round(title.game.progress * 100) + "%" : "Installing…") : ""
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }
}
