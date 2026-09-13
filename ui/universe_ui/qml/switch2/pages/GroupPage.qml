import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Groups.js" as Groups

FocusScope {
    id: page

    property var shell: null
    property var args: ({})

    signal closeRequested()

    readonly property string groupKey: args.group || "favourites"
    readonly property string name: args.name || ""
    property string zone: "grid"
    property int sortMode: 0
    readonly property var sortNames: ["By Recently Played", "By Title", "By Play Time", "By Release"]

    readonly property var hints: zone === "rail" ? [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]
                               : [ { glyph: "Start", label: "Options" }, { glyph: "B", label: "Back" }, { glyph: "A", label: "Start" } ]

    property var list: []

    function key(g) {
        return sortMode === 1 ? g.sortTitle.toLowerCase()
             : sortMode === 2 ? -g.playTime
             : sortMode === 3 ? -g.releaseYear
             : -(g.lastPlayed instanceof Date && !isNaN(g.lastPlayed.getTime()) ? g.lastPlayed.getTime() : 0);
    }

    function rebuild() {
        var keyed = Groups.gamesOf(api.allGames, api.collections, groupKey).map(function(g) { return { k: page.key(g), g: g }; });
        keyed.sort(function(a, b) { return a.k < b.k ? -1 : a.k > b.k ? 1 : 0; });
        list = keyed.map(function(e) { return e.g; });
    }

    Connections {
        target: api.allGames
        function onCountChanged() { page.rebuild(); }
    }
    onArgsChanged: rebuild()
    onSortModeChanged: rebuild()

    focus: true

    onActiveFocusChanged: {
        if (activeFocus)
            zone === "rail" ? rail.forceActiveFocus() : grid.forceActiveFocus();
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "grid"
        title: page.name
        trailing: Groups.count(page.list.length)
    }

    Rail {
        id: rail
        x: Theme.dp(102)
        y: Theme.dp(170)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        items: [ { id: "sort", icon: "sort" } ]
        focus: page.zone === "rail"
        onActivated: function(id) {
            page.shell.pick({ title: "Sort", choices: page.sortNames, index: page.sortMode }, function(i) {
                if (i >= 0) {
                    page.sortMode = i;
                    grid.index = 0;
                }
            });
        }
        onEscapedRight: {
            page.zone = "grid";
            grid.forceActiveFocus();
        }
    }

    Text {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(160)
        y: Theme.dp(140)
        text: page.sortNames[page.sortMode]
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    SoftwareGrid {
        id: grid
        x: Theme.dp(253)
        y: Theme.dp(190)
        width: implicitWidth
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        games: page.list
        focus: page.zone === "grid"
        onEscapedLeft: {
            page.zone = "rail";
            rail.forceActiveFocus();
        }
        onEscapedUp: Sound.edge()
        onActivated: function(i) { page.shell.launch(games[i]); }
        onOptionsRequested: function(i) { page.shell.push("pages/SoftwareOptionsPage.qml", { gameId: games[i].id }); }
    }
}
