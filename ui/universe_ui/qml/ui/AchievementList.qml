import QtQuick
import "../core"
import "../sound"

// A game's achievements as a list, the lit one told in full beside it: the Achievements page and the dock's tray.
Item {
    id: list

    property var store: null
    property int index: 0
    property real sideMargin: Theme.dp(90)
    property real listWidth: Theme.dp(760)
    // Off (the dock's tray): the list alone, each row with its description and date.
    property bool pane: true
    readonly property real rowHeight: Theme.dp(pane ? 104 : 92)

    readonly property var rows: store ? store.rows : []
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null

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
        height: list.pane ? 0 : count.height + Theme.dp(28)
        visible: !list.pane && list.rows.length > 0

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

            Rectangle {
                width: list.store.total > 0 ? parent.width * list.store.unlocked / list.store.total : 0
                height: parent.height
                radius: parent.radius
                color: Theme.text
            }
        }
    }

    ListView {
        id: view

        Wheel {
            step: list.rowHeight + view.spacing
        }

        anchors.top: tally.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.leftMargin: list.sideMargin
        width: list.pane ? list.listWidth : list.width - list.sideMargin * 2
        model: list.rows
        currentIndex: list.index
        interactive: false
        clip: true
        spacing: Theme.dp(12)
        highlightFollowsCurrentItem: true
        highlightMoveDuration: Theme.durNudge
        preferredHighlightBegin: 0
        preferredHighlightEnd: height
        highlightRangeMode: ListView.ApplyRange

        delegate: SessionRow {
            id: entry

            width: view.width
            height: list.rowHeight
            lit: index === list.index
            muted: !modelData.unlocked
            title: modelData.name
            subtitle: !list.pane ? modelData.description : modelData.unlocked ? modelData.dateText : modelData.rarityText !== "" ? modelData.rarityText : "Locked"
            trailing: !list.pane && modelData.unlocked ? modelData.dateText : ""
            leadWidth: Theme.dp(72)
            gap: Theme.dp(20)

            Pointer {
                current: entry.lit
                radius: Theme.dp(14)
                onPicked: list.index = index
            }

            AchievementBadge {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(72)
                height: width
                icon: modelData.icon
                unlocked: modelData.unlocked
                checked: modelData.unlocked
                tint: entry.lit ? Theme.onLight : Theme.text
            }
        }
    }

    Column {
        id: pane

        anchors.top: view.top
        anchors.left: view.right
        anchors.leftMargin: Theme.dp(64)
        anchors.right: parent.right
        anchors.rightMargin: list.sideMargin
        spacing: Theme.dp(18)
        visible: list.pane && list.rows.length > 0

        Text {
            text: list.store.unlocked + " of " + list.store.total + " unlocked"
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(34)
        }

        Rectangle {
            width: parent.width
            height: Theme.dp(10)
            radius: height / 2
            color: Theme.surface

            Rectangle {
                width: list.store.total > 0 ? parent.width * list.store.unlocked / list.store.total : 0
                height: parent.height
                radius: parent.radius
                color: Theme.text

                Behavior on width {
                    Ease {
                        duration: Theme.durView
                    }
                }
            }
        }

        Text {
            width: parent.width
            text: list.store.loading ? "Asking the store…" : list.store.error !== "" ? list.store.error : list.store.fetchedText !== "" ? "From the store on " + list.store.fetchedText : ""
            color: list.store.error !== "" ? "#f0757a" : Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(20)
            elide: Text.ElideRight
        }

        Item {
            width: parent.width
            height: Theme.dp(24)
        }

        AchievementBadge {
            width: Theme.dp(160)
            height: width
            icon: list.current ? list.current.icon : ""
            unlocked: list.current ? list.current.unlocked : false
        }

        Text {
            width: parent.width
            text: list.current ? list.current.name : ""
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(32)
            wrapMode: Text.WordWrap
        }

        Text {
            width: parent.width
            visible: text !== ""
            text: list.current ? list.current.description : ""
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(24)
            wrapMode: Text.WordWrap
        }

        Text {
            width: parent.width
            text: !list.current ? "" : list.current.unlocked ? "Unlocked " + list.current.dateText : "Locked"
            color: list.current && list.current.unlocked ? Theme.text : Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(22)
        }

        Text {
            width: parent.width
            visible: text !== ""
            text: list.current ? list.current.rarityText : ""
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(22)
        }
    }
}
