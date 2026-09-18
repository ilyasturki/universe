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
    readonly property var shots: api.screens.shots
    readonly property var frameMap: store.frameMap

    // Screenshots and recordings on one grid, newest first, like the console's Album.
    readonly property var rows: {
        var out = store.rows.map(function(r) { return Object.assign({ kind: "recording", when: r.created_at }, r); })
            .concat(shots.rows.map(function(r) { return Object.assign({ kind: "shot", when: r.taken_at }, r); }));
        out.sort(function(a, b) { return a.when < b.when ? 1 : a.when > b.when ? -1 : 0; });
        return out;
    }

    property string filterId: ""
    property string kindFilter: ""
    property bool oldestFirst: false
    property string zone: "grid"
    property bool viewing: false

    readonly property var shown: {
        var out = rows.filter(function(r) { return (filterId === "" || r.gameId === filterId) && (kindFilter === "" || r.kind === kindFilter); });
        return oldestFirst ? out.reverse() : out;
    }
    readonly property var current: grid.index < shown.length ? shown[grid.index] : null
    readonly property string filterName: {
        var g = api.allGames.byId(filterId);
        return filterId === "" ? "All" : g ? g.title : filterId;
    }

    readonly property var games: Feed.channels(rows)

    readonly property var hints: viewing
        ? [ { glyph: "dpad", label: "Previous / next" }, { glyph: "B", label: "Close" } ]
        : zone === "rail"
        ? [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]
        : [ { glyph: "Start", label: "Options", dim: shown.length === 0 }, { glyph: "B", label: "Back" },
            { glyph: "A", label: shown.length === 0 ? "OK" : current && current.kind === "shot" ? "View" : "Play", dim: shown.length === 0 } ]

    signal closeRequested()

    readonly property real thumbWidth: Theme.dp(290)
    readonly property real thumbHeight: Math.round(thumbWidth * 9 / 16)

    focus: true

    Component.onCompleted: {
        store.loadAll();
        shots.loadAll();
    }
    Component.onDestruction: {
        store.unload();
        shots.unload();
    }

    onArgsChanged: {
        if (args.gameId)
            filterId = args.gameId;
    }

    onCurrentChanged: {
        if (current && current.kind === "recording")
            store.select(current.session);
    }

    function play() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        if (current.kind === "shot")
            viewing = true;
        else
            shell.push("pages/PlayerPage.qml", { session: current.session, gameId: current.gameId });
    }

    function stepShot(d) {
        var i = grid.index + d;
        while (i >= 0 && i < shown.length && shown[i].kind !== "shot")
            i += d;
        if (i < 0 || i >= shown.length) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        grid.index = i;
    }

    Keys.onPressed: function(event) {
        if (!viewing)
            return;
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        if (event.isAutoRepeat && !arrow)
            return;
        event.accepted = true;
        if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
            Sound.play("back");
            viewing = false;
        } else if (arrow) {
            stepShot(event.key === Qt.Key_Left ? -1 : 1);
        }
    }

    function options() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        var row = current;
        var items = [{ label: row.kind === "shot" ? "View" : "Play", act: "play" }];
        if (row.hasJournal)
            items.push({ label: "Open journal entry", act: "journal" });
        items.push({ label: row.kind === "shot" ? "Remove screenshot…" : "Remove recording…", act: "remove" });
        shell.menu(row.gameTitle + " · " + row.dateText, items, function(act) {
            if (act === "play")
                play();
            else if (act === "journal")
                shell.push("pages/ArticlePage.qml", { session: row.session, gameId: row.gameId });
            else if (row.kind === "shot")
                Removal.screenshot(shell, api.screens, row, function() {});
            else
                Removal.recording(shell, api.screens, row, function() {});
        });
    }

    function railAction(id) {
        if (id === "kind") {
            var kinds = ["", "shot", "recording"];
            shell.pick({ title: "Show", choices: ["Everything", "Screenshots", "Videos"], index: Math.max(0, kinds.indexOf(kindFilter)) }, function(i) {
                if (i >= 0) {
                    kindFilter = kinds[i];
                    grid.index = 0;
                }
            });
        } else if (id === "filter") {
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
        trailing: (page.oldestFirst ? "Oldest First" : "Newest First") + "  |  " + (page.kindFilter === "" ? "Everything" : page.kindFilter === "shot" ? "Screenshots" : "Videos") + "  |  " + page.filterName + " (" + page.shown.length + ")"
    }

    Rail {
        id: rail
        x: Theme.dp(102)
        y: Theme.dp(185)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "rail" && !page.viewing
        items: [ { id: "kind", icon: "album", label: "Show" }, { id: "filter", icon: "filter", label: "Filter" }, { id: "sort", icon: "sort", label: "Sort" } ]
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
        text: page.rows.length === 0 ? "Nothing yet. Screenshots and recorded sessions land here." : "Nothing here for this filter."
        color: Theme.textMuted
    }

    CellGrid {
        id: grid

        x: Theme.dp(253)
        y: Theme.dp(192)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "grid" && !page.viewing
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

            readonly property var frames: entry.kind === "recording" ? page.frameMap[entry.session] || null : null

            Rectangle {
                id: body
                anchors.fill: parent
                radius: Theme.dp(4)
                color: "#101010"

                Image {
                    anchors.fill: parent
                    source: entry.kind === "shot" ? entry.url : cell.frames ? cell.frames.thumbnail : ""
                    sourceSize.width: 640
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
                visible: entry.kind === "recording"
                width: durationText.implicitWidth + Theme.dp(18)
                height: Theme.dp(34)
                radius: Theme.dp(3)
                color: Qt.rgba(0, 0, 0, 0.55)

                Label {
                    id: durationText
                    anchors.centerIn: parent
                    text: entry.durationText || ""
                    color: "#ffffff"
                    font.pixelSize: Theme.dp(Theme.fontTiny)
                }
            }
        }
    }

    Rectangle {
        id: viewer

        anchors.fill: parent
        z: 10
        color: "#000000"
        opacity: page.viewing ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity { Ease { duration: Theme.durFade } }

        Image {
            anchors.fill: parent
            anchors.margins: Theme.dp(24)
            source: page.viewing && page.current && page.current.kind === "shot" ? page.current.url : ""
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            sourceSize.width: 1920
        }

        Label {
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.dp(Theme.hintBarHeight) + Theme.dp(12)
            anchors.horizontalCenter: parent.horizontalCenter
            text: page.current ? page.current.gameTitle + " · " + page.current.dateText : ""
            color: "#ffffff"
        }
    }
}
