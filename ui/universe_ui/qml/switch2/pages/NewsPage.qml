import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Feed.js" as Feed
import "../ui/Removal.js" as Removal

FocusScope {
    id: page

    property var shell: null
    property var args: ({})

    readonly property var store: api.screens.news
    readonly property var rows: store.rows

    property string filterId: ""
    property string zone: "grid"

    readonly property var articles: rows.filter(function(r) { return filterId === "" || r.gameId === filterId; })
    readonly property var channels: Feed.channels(rows)
    readonly property var current: grid.index < articles.length ? articles[grid.index] : null

    readonly property var hints: zone === "rail"
        ? [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]
        : [ { glyph: "Start", label: "Options", dim: current === null }, { glyph: "B", label: "Back" }, { glyph: "A", label: "Read", dim: current === null } ]

    signal closeRequested()

    readonly property real cardWidth: Theme.dp(489)
    readonly property real imageHeight: Math.round(cardWidth * 9 / 16)
    readonly property real captionHeight: Theme.dp(105)

    focus: true

    Component.onCompleted: store.loadAll()
    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (args.gameId)
            filterId = args.gameId;
    }

    function options() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        var row = current;
        var pending = row.state === "pending";
        var items = pending ? [] : [{ label: "Read", act: "read" }];
        if (row.hasRecording)
            items.push({ label: "Watch the recording", act: "recording" });
        items.push({ label: pending ? "Cancel the writing…" : "Remove entry…", act: "remove" });
        shell.menu(row.gameTitle + " · " + row.dateText, items, function(act) {
            if (act === "read")
                shell.push("pages/ArticlePage.qml", { session: row.session, gameId: row.gameId });
            else if (act === "recording")
                shell.push("pages/PlayerPage.qml", { session: row.session, gameId: row.gameId });
            else
                Removal.entry(shell, api.screens, row, function() {});
        });
    }

    function activate() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        shell.push("pages/ArticlePage.qml", { session: current.session, gameId: current.gameId });
    }

    function railAction(id) {
        if (id === "filter") {
            var ids = [""].concat(channels.map(function(c) { return c.id; }));
            var choices = ["All"].concat(channels.map(function(c) { return c.title; }));
            shell.pick({ title: "Show", choices: choices, index: Math.max(0, ids.indexOf(filterId)) }, function(i) {
                if (i >= 0) {
                    filterId = ids[i];
                    grid.index = 0;
                }
            });
        }
    }

    function filterName() {
        var g = api.allGames.byId(filterId);
        return g ? g.title : filterId;
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "news"
        title: "News"
        trailing: page.filterId !== "" ? page.filterName() : ""
    }

    Rail {
        id: rail
        x: Theme.dp(102)
        y: Theme.dp(260)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "rail"
        items: [ { id: "filter", icon: "filter", label: "Filter" } ]
        onActivated: function(id) { page.railAction(id); }
        onEscapedRight: {
            if (page.articles.length > 0)
                page.zone = "grid";
            else
                Sound.play("edge");
        }
    }

    Label {
        anchors.centerIn: grid
        visible: page.articles.length === 0
        text: page.rows.length === 0 ? "No entries yet. The journal writes one after each session." : "No entries for this game yet."
        color: Theme.textMuted
    }

    CellGrid {
        id: grid

        x: Theme.dp(253)
        y: Theme.dp(226)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "grid"
        model: page.articles
        columns: 3
        cellWidth: page.cardWidth
        cellHeight: page.imageHeight + page.captionHeight
        gap: Theme.dp(18)

        onEscapedLeft: page.zone = "rail"
        onActivated: page.activate()
        onOptionsRequested: page.options()

        delegate: Item {
            id: card

            readonly property var game: api.allGames.byId(entry.gameId)
            readonly property string picture: entry.images && entry.images.length > 0 ? entry.images[0] : game ? String(game.assets.banner) : ""

            Rectangle {
                id: body
                anchors.fill: parent
                radius: Theme.dp(6)
                color: Theme.card
            }

            FocusOutline {
                target: body
                cornerRadius: body.radius
                shown: focused
            }

            Item {
                id: picture
                width: parent.width
                height: page.imageHeight
                clip: true

                Rectangle {
                    anchors.fill: parent
                    color: Theme.artShade
                }

                Image {
                    anchors.fill: parent
                    source: card.picture
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    sourceSize.width: 640
                    visible: card.picture !== "" && status === Image.Ready
                }

                Tile {
                    anchors.fill: parent
                    visible: card.picture === ""
                    game: card.picture === "" ? card.game : null
                    cornerRadius: 0
                    outlineShown: false
                }
            }

            Tile {
                id: badge
                x: Theme.dp(18)
                y: page.imageHeight + Theme.dp(24)
                width: Theme.dp(52)
                height: Theme.dp(52)
                cornerRadius: Theme.dp(4)
                game: card.game
                outlineShown: false
            }

            Label {
                x: badge.x + badge.width + Theme.dp(20)
                y: page.imageHeight + Theme.dp(14)
                width: parent.width - x - Theme.dp(18)
                height: page.captionHeight - Theme.dp(14)
                verticalAlignment: Text.AlignVCenter
                text: entry.title
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
                lineHeight: 1.15
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }
}
