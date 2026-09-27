import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "achievementsPage"
    focus: true

    property var args: ({})
    readonly property var game: args.game || null
    readonly property var store: api.screens.achievements
    readonly property var rows: store.rows
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null

    signal closeRequested

    readonly property var hints: [
        {
            glyph: "X",
            label: "Ask the store again",
            dim: store.loading
        },
        {
            glyph: "B",
            label: "Back"
        }
    ]

    readonly property real sideMargin: Theme.dp(90)
    readonly property real listWidth: Theme.dp(760)

    Component.onDestruction: store.unload()

    onGameChanged: {
        index = 0;
        if (game)
            store.load(game.id);
    }

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

    Keys.onPressed: function (event) {
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (event.isAutoRepeat && !vertical && !screen)
            return;
        event.accepted = true;
        if (api.keys.isCancel(event)) {
            page.closeRequested();
        } else if (api.keys.isDetails(event)) {
            if (store.loading) {
                Sound.edge();
            } else {
                Sound.enter();
                store.refresh();
            }
        } else if (vertical) {
            step(event.key === Qt.Key_Up ? -1 : 1);
        } else if (screen) {
            step(screen * 5);
        } else if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            step((api.keys.isFirst(event) ? -1 : 1) * rows.length);
        } else {
            event.accepted = false;
        }
    }

    GameBackdrop {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
    }

    GameHeader {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        game: page.game
        label: "ACHIEVEMENTS"
    }

    Text {
        anchors.centerIn: parent
        visible: page.rows.length === 0
        text: page.store.loading ? "Asking the store…" : page.store.error !== "" ? page.store.error : "This game lists no achievements."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    ListView {
        id: list

        Wheel {
            step: Theme.dp(104) + list.spacing
        }

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(32)
        anchors.bottom: hintBar.top
        anchors.left: parent.left
        anchors.leftMargin: page.sideMargin
        width: page.listWidth
        model: page.rows
        currentIndex: page.index
        interactive: false
        clip: true
        spacing: Theme.dp(12)
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: 0
        preferredHighlightEnd: height
        highlightRangeMode: ListView.ApplyRange

        delegate: SessionRow {
            id: entry

            width: list.width
            height: Theme.dp(104)
            lit: index === page.index
            muted: !modelData.unlocked
            title: modelData.name
            subtitle: modelData.unlocked ? modelData.dateText : modelData.rarityText !== "" ? modelData.rarityText : "Locked"
            leadWidth: Theme.dp(72)
            gap: Theme.dp(20)

            Pointer {
                current: entry.lit
                radius: Theme.dp(14)
                onPicked: page.index = index
            }

            AchievementBadge {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(72)
                height: width
                icon: modelData.icon
                unlocked: modelData.unlocked
                tint: entry.lit ? Theme.onLight : Theme.text
            }
        }
    }

    Column {
        id: pane

        anchors.top: list.top
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(64)
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        spacing: Theme.dp(18)
        visible: page.rows.length > 0

        Text {
            text: page.store.unlocked + " of " + page.store.total + " unlocked"
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
                width: page.store.total > 0 ? parent.width * page.store.unlocked / page.store.total : 0
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
            text: page.store.loading ? "Asking the store…" : page.store.error !== "" ? page.store.error : page.store.fetchedText !== "" ? "From the store on " + page.store.fetchedText : ""
            color: page.store.error !== "" ? "#f0757a" : Theme.textMuted
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
            icon: page.current ? page.current.icon : ""
            unlocked: page.current ? page.current.unlocked : false
        }

        Text {
            width: parent.width
            text: page.current ? page.current.name : ""
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(32)
            wrapMode: Text.WordWrap
        }

        Text {
            width: parent.width
            visible: text !== ""
            text: page.current ? page.current.description : ""
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(24)
            wrapMode: Text.WordWrap
        }

        Text {
            width: parent.width
            text: !page.current ? "" : page.current.unlocked ? "Unlocked " + page.current.dateText : "Locked"
            color: page.current && page.current.unlocked ? Theme.text : Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(22)
        }

        Text {
            width: parent.width
            visible: text !== ""
            text: page.current ? page.current.rarityText : ""
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(22)
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        hints: page.hints
    }
}
