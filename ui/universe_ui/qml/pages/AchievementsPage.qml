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

    Component.onDestruction: store.unload()

    onGameChanged: {
        list.index = 0;
        if (game)
            store.load(game.id);
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
            list.step(event.key === Qt.Key_Up ? -1 : 1);
        } else if (screen) {
            list.step(screen * list.perScreen);
        } else if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            list.step((api.keys.isFirst(event) ? -1 : 1) * list.rows.length);
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

    AchievementList {
        id: list

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(32)
        anchors.bottom: hintBar.top
        anchors.left: parent.left
        anchors.right: parent.right
        store: page.store
        sideMargin: page.sideMargin
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
