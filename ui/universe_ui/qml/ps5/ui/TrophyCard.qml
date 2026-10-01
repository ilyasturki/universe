import QtQuick
import "../core"
import "../../ui" as Base

// One trophy of the list: its icon, its name over what it asks, and at the right when it was earned and how rare it is.
Item {
    id: card

    // An api.screens.achievements row.
    property var trophy: ({})
    property bool focused: false

    signal picked

    readonly property bool unlocked: trophy.unlocked === true
    readonly property int hidden: trophy.hidden || 0

    // The console's grades of rarity, by the share of players who earned it.
    function grade(r) {
        return r < 5 ? "Ultra rare" : r < 15 ? "Very rare" : r < 50 ? "Rare" : "Common";
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(Theme.radiusCard)
        color: card.unlocked ? Qt.rgba(0.1, 0.11, 0.14, 0.9) : Qt.rgba(0.07, 0.075, 0.095, 0.86)
        border.width: 1
        border.color: Theme.glassEdge
    }

    Base.AchievementBadge {
        id: badge
        x: Theme.dp(20)
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.dp(64)
        height: width
        icon: card.trophy.icon || ""
        unlocked: card.unlocked
        checked: card.unlocked
        tint: Theme.text
    }

    Column {
        anchors.left: badge.right
        anchors.leftMargin: Theme.dp(22)
        anchors.right: side.left
        anchors.rightMargin: Theme.dp(20)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(4)

        Label {
            width: parent.width
            text: card.hidden > 0 ? card.hidden + (card.hidden === 1 ? " hidden trophy" : " hidden trophies") : card.trophy.name || ""
            color: card.unlocked ? Theme.text : Theme.textSecondary
            elide: Text.ElideRight
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(26)
        }

        Label {
            width: parent.width
            visible: text !== ""
            text: card.trophy.description || ""
            color: Theme.textMuted
            elide: Text.ElideRight
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }

    Column {
        id: side
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(22)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(4)

        Label {
            anchors.right: parent.right
            visible: card.unlocked
            text: card.trophy.dateText || ""
            font.pixelSize: Theme.dp(Theme.fontTiny)
        }

        Label {
            anchors.right: parent.right
            visible: text !== ""
            text: card.trophy.rarity !== undefined && card.trophy.rarity >= 0 ? card.grade(card.trophy.rarity) + "  ·  " + (card.trophy.rarity < 10 ? card.trophy.rarity.toFixed(1) : Math.round(card.trophy.rarity)) + "%" : ""
            color: Theme.textMuted
            font.pixelSize: Theme.dp(Theme.fontTiny)
        }
    }

    FocusFrame {
        target: card
        shown: card.focused
        radius: Theme.dp(Theme.radiusCard)
        gap: Theme.dp(3)
        line: Theme.dp(3)
    }

    Touch {
        current: card.focused
        onPicked: card.picked()
    }
}
