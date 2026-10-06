import QtQuick
import "../core"
import "../sound"

// A game's achievements in one column, each row with its description and its date once unlocked, its rarity while locked: the Achievements page and the dock's tray.
Item {
    id: list

    property var store: null
    property int index: 0
    property real sideMargin: Theme.dp(90)
    readonly property real rowHeight: Theme.dp(80)
    readonly property int perScreen: Math.max(1, Math.floor((view.height + view.spacing) / (rowHeight + view.spacing)))

    readonly property var rows: store ? store.rows : []

    onRowsChanged: {
        if (index >= rows.length)
            index = Math.max(0, rows.length - 1);
    }

    function step(d) {
        var next = Math.max(0, Math.min(rows.length - 1, index + d));
        if (rows.length === 0 || next === index) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
    }

    Text {
        anchors.centerIn: parent
        visible: list.rows.length === 0
        text: list.store.loading ? "Asking the store…" : list.store.error !== "" ? list.store.error : "This game lists no achievements."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    Item {
        id: tally

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: list.sideMargin
        anchors.rightMargin: list.sideMargin
        height: count.height + Theme.dp(24)
        visible: list.rows.length > 0

        Text {
            id: count
            text: list.store.unlocked + " / " + list.store.total
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(26)
        }

        Rectangle {
            anchors.left: count.right
            anchors.leftMargin: Theme.dp(24)
            anchors.right: parent.right
            anchors.verticalCenter: count.verticalCenter
            height: Theme.dp(6)
            radius: height / 2
            color: Theme.surface
            visible: list.store.error === ""

            Rectangle {
                width: list.store.total > 0 ? parent.width * list.store.unlocked / list.store.total : 0
                height: parent.height
                radius: parent.radius
                color: Theme.text
            }
        }

        // A refresh that failed keeps the list it had.
        Text {
            anchors.left: count.right
            anchors.leftMargin: Theme.dp(24)
            anchors.right: parent.right
            anchors.verticalCenter: count.verticalCenter
            visible: list.store.error !== ""
            text: list.store.error
            color: Theme.danger
            font.family: Theme.sans
            font.pixelSize: Theme.dp(20)
            elide: Text.ElideRight
        }
    }

    ListView {
        id: view

        Scroller {
            step: list.rowHeight + view.spacing
        }

        anchors.top: tally.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: list.sideMargin
        anchors.rightMargin: list.sideMargin
        model: list.rows
        currentIndex: list.index
        interactive: false
        clip: true
        spacing: Theme.dp(8)
        highlightFollowsCurrentItem: false

        delegate: SessionRow {
            id: entry

            width: view.width
            height: list.rowHeight
            lit: index === list.index
            muted: !modelData.unlocked
            title: modelData.name
            subtitle: modelData.description
            trailing: modelData.unlocked ? modelData.dateText : modelData.rarityText
            leadWidth: Theme.dp(56)
            gap: Theme.dp(20)

            Pointer {
                current: entry.lit
                radius: Theme.dp(14)
                onPicked: list.index = index
            }

            AchievementBadge {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(56)
                height: width
                icon: modelData.icon
                unlocked: modelData.unlocked
                checked: modelData.unlocked
                tint: entry.lit ? Theme.onLight : Theme.text
                checkInk: entry.lit ? Theme.text : Theme.onLight
            }
        }
    }
}
