import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../core/Format.js" as Format

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var session: api.universe.currentSession
    readonly property bool playing: game !== null && session !== null && session !== undefined && session.id === game.id

    readonly property var hints: [
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: "OK"
        }
    ]

    signal closeRequested

    focus: true

    readonly property var entries: {
        if (!game)
            return [];
        var start = playing ? [
            {
                key: "resume",
                label: "Resume",
                type: "action",
                display: "Playing"
            },
            {
                key: "close",
                label: "Close Software",
                type: "action"
            }
        ] : [
            {
                key: "start",
                label: game.playTime > 0 ? "Continue" : "Start",
                type: "action"
            }
        ];
        return start.concat([
            {
                key: "info",
                label: "Software Information",
                type: "action",
                page: "pages/SoftwareInfoPage.qml"
            },
            {
                key: "favourite",
                label: game.favorite ? "Remove from Favourites" : "Add to Favourites",
                type: "action"
            },
            {
                key: "settings",
                label: "Game Settings",
                type: "action",
                page: "pages/GameSettingsPage.qml"
            },
            {
                key: "recordings",
                label: "Recordings",
                type: "action",
                page: "pages/AlbumPage.qml"
            },
            {
                key: "journal",
                label: "Journal",
                type: "action",
                page: "pages/NewsPage.qml"
            },
            {
                key: "artwork",
                label: "Artwork",
                type: "action",
                page: "pages/ArtworkPage.qml"
            },
            {
                key: "remove",
                label: "Remove from Library…",
                type: "action"
            }
        ]);
    }

    readonly property var stats: {
        if (!game)
            return [];
        var last = Format.lastPlayed(game.lastPlayed), time = Format.playTime(game.playTime);
        return [last === "Never played" ? last : "Last played " + last.toLowerCase(), time && "Played for " + time, Format.sessions(game.playCount)].filter(Boolean);
    }

    function activate(index, row) {
        if (!game)
            return;
        if (row.key === "start") {
            shell.launch(game);
        } else if (row.key === "resume") {
            shell.resume();
        } else if (row.key === "close") {
            Sound.play("ok");
            shell.closeSoftware(game);
        } else if (row.page) {
            Sound.play("ok");
            shell.push(row.page, {
                gameId: game.id
            });
        } else if (row.key === "favourite") {
            game.favorite = !game.favorite;
            Sound.play("select");
        } else if (row.key === "remove") {
            Sound.play("ok");
            var id = game.id, title = game.title;
            shell.dialogAsk({
                message: "Remove " + title + " from the library?",
                detail: "The install folder, the hours and the journal stay on disk.",
                buttons: ["Cancel", "Remove"],
                danger: 1
            }, function (i) {
                if (i === 1) {
                    api.universe.remove(id, false);
                    shell.pop();
                }
            });
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        title: page.game ? page.game.title : ""
        subtitle: page.game && page.game.publisherList.length > 0 ? page.game.publisherList.join(", ") : ""
    }

    Column {
        x: Theme.dp(530) - width / 2
        y: Theme.dp(300)
        width: Theme.dp(560)
        spacing: Theme.dp(10)

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: page.playing
            text: "Playing"
            color: Theme.accent
        }

        Item {
            width: parent.width
            height: page.playing ? Theme.dp(10) : Theme.dp(60)
        }

        Tile {
            anchors.horizontalCenter: parent.horizontalCenter
            width: Theme.dp(300)
            height: Theme.dp(300)
            game: page.game
            outlineShown: false
        }

        Item {
            width: parent.width
            height: Theme.dp(26)
        }

        Label {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: page.game ? page.game.title : ""
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            elide: Text.ElideRight
        }

        Item {
            width: parent.width
            height: Theme.dp(6)
        }

        Repeater {
            model: page.stats

            Label {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: modelData
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }

    SettingsRows {
        id: rows

        x: Theme.dp(1005)
        y: Theme.dp(260)
        width: Theme.dp(675)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        focus: true
        model: page.entries

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
        onEscapedDown: Sound.play("edge")
    }
}
