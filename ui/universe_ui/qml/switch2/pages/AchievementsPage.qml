import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var store: api.screens.achievements
    readonly property var rows: store.rows

    readonly property var hints: [
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "X",
            label: "Ask the store again" + (store.fetchedText !== "" ? " · updated " + store.fetchedText : "")
        }
    ]

    signal closeRequested

    focus: true

    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (args && args.gameId)
            store.load(args.gameId);
    }

    Keys.onPressed: function (event) {
        if (!api.keys.isDetails(event))
            return;
        event.accepted = true;
        if (store.loading) {
            Sound.play("edge");
        } else {
            Sound.play("ok");
            store.refresh();
        }
    }

    readonly property var entries: rows.map(function (r) {
        return {
            key: r.key,
            label: r.name,
            secondary: r.description,
            type: "static",
            dim: !r.unlocked,
            icon: r.icon,
            iconSlot: true,
            display: r.unlocked ? r.dateText : r.rarityText
        };
    })

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        title: "Achievements"
        subtitle: page.game ? page.game.title : ""
        trailing: page.store.total > 0 ? page.store.unlocked + " / " + page.store.total : ""
    }

    Label {
        anchors.centerIn: list
        visible: page.rows.length === 0
        text: page.store.loading ? "Checking with the store…" : page.store.error !== "" ? page.store.error : "This software lists no achievements."
        color: Theme.textMuted
    }

    SettingsRows {
        id: list

        objectName: "achievements"
        x: Theme.dp(120)
        y: Theme.dp(160)
        width: parent.width - x * 2
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        focus: true
        model: page.entries

        onActivated: Sound.play("edge")
        onEscapedLeft: Sound.play("edge")
    }
}
