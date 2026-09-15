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

    readonly property var store: api.screens.album
    readonly property var rows: store.rows
    readonly property var frameMap: store.frameMap

    property string filterId: ""
    property bool oldestFirst: false
    property string zone: "grid"

    readonly property var shown: {
        var out = rows.filter(function(r) { return filterId === "" || r.gameId === filterId; });
        return oldestFirst ? out.reverse() : out;
    }
    readonly property var current: grid.index < shown.length ? shown[grid.index] : null
    readonly property string filterName: {
        var g = api.allGames.byId(filterId);
        return filterId === "" ? "All" : g ? g.title : filterId;
    }

    readonly property var games: Feed.channels(rows)

    readonly property var hints: zone === "rail"
        ? [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]
        : [ { glyph: "Start", label: "Options", dim: shown.length === 0 }, { glyph: "B", label: "Back" }, { glyph: "A", label: shown.length > 0 ? "Play" : "OK", dim: shown.length === 0 } ]

    signal closeRequested()

    readonly property real thumbWidth: Theme.dp(290)
    readonly property real thumbHeight: Math.round(thumbWidth * 9 / 16)

    focus: true

    Component.onCompleted: store.loadAll()
    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (args.gameId)
            filterId = args.gameId;
    }

    onCurrentChanged: {
        if (current)
            store.select(current.session);
    }

    function play() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        shell.push("pages/PlayerPage.qml", { session: current.session, gameId: current.gameId });
    }

    function options() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        var row = current;
        var items = [{ label: "Play", act: "play" }];
        if (row.hasJournal)
            items.push({ label: "Open journal entry", act: "journal" });
        items.push({ label: "Remove recording…", act: "remove" });
        shell.menu(row.gameTitle + " · " + row.dateText, items, function(act) {
            if (act === "play")
                play();
            else if (act === "journal")
                shell.push("pages/ArticlePage.qml", { session: row.session, gameId: row.gameId });
            else
                Removal.recording(shell, api.screens, row, function() {});
        });
    }

    function railAction(id) {
        if (id === "filter") {
            var ids = [""].concat(games.map(function(g) { return g.id; }));
            var choices = ["All"].concat(games.map(function(g) { return g.title; }));
            shell.pick({ title: "Show", choices: choices, index: Math.max(0, ids.indexOf(filterId)) }, function(i) {
                if (i >= 0) {
                    filterId = ids[i];
                    grid.index = 0;
                }
            });
        } else if (id === "sort") {
            shell.pick({ title: "Sort", choices: ["Newest First", "Oldest First"], index: oldestFirst ? 1 : 0 }, function(i) {
                if (i >= 0) {
                    oldestFirst = i === 1;
                    grid.index = 0;
                }
            });
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "album"
        title: "Album"
        trailing: (page.oldestFirst ? "Oldest First" : "Newest First") + "  |  " + page.filterName + " (" + page.shown.length + ")"
    }

    Rail {
        id: rail
        x: Theme.dp(102)
        y: Theme.dp(185)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "rail"
        items: [ { id: "filter", icon: "filter", label: "Filter" }, { id: "sort", icon: "sort", label: "Sort" } ]
        onActivated: function(id) { page.railAction(id); }
        onEscapedRight: {
            if (page.shown.length > 0)
                page.zone = "grid";
            else
                Sound.play("edge");
        }
    }

    Label {
        anchors.centerIn: grid
        visible: page.shown.length === 0
        text: page.rows.length === 0 ? "No recordings yet. A session recorded by the capture module lands here." : "No recordings for this game yet."
        color: Theme.textMuted
    }

    CellGrid {
        id: grid

        x: Theme.dp(253)
        y: Theme.dp(192)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "grid"
        model: page.shown
        columns: 5
        cellWidth: page.thumbWidth
        cellHeight: page.thumbHeight
        gap: Theme.dp(12)

        onEscapedLeft: page.zone = "rail"
        onActivated: page.play()
        onOptionsRequested: page.options()

        delegate: Item {
            id: cell

            readonly property var frames: page.frameMap[entry.session] || null

            Rectangle {
                id: body
                anchors.fill: parent
                radius: Theme.dp(4)
                color: "#101010"

                Image {
                    anchors.fill: parent
                    source: cell.frames ? cell.frames.thumbnail : ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    opacity: status === Image.Ready ? 1.0 : 0.0

                    Behavior on opacity { Ease { duration: Theme.durFade } }
                }
            }

            FocusOutline {
                target: body
                cornerRadius: body.radius
                shown: focused
            }

            Tile {
                x: Theme.dp(8)
                y: Theme.dp(8)
                width: Theme.dp(40)
                height: Theme.dp(40)
                cornerRadius: Theme.dp(4)
                game: api.allGames.byId(entry.gameId)
                outlineShown: false
            }

            Rectangle {
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: Theme.dp(6)
                width: durationText.implicitWidth + Theme.dp(18)
                height: Theme.dp(34)
                radius: Theme.dp(3)
                color: Qt.rgba(0, 0, 0, 0.55)

                Label {
                    id: durationText
                    anchors.centerIn: parent
                    text: entry.durationText
                    color: "#ffffff"
                    font.pixelSize: Theme.dp(Theme.fontTiny)
                }
            }
        }
    }
}
