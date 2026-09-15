import QtQuick
import Universe
import "../core"
import "../ui"
import "Groups.js" as Groups

FocusScope {
    id: page

    property var shell: null

    signal closeRequested()

    property int tab: 0
    property string zone: "grid"
    property int sortMode: 0
    property string query: ""

    readonly property var sortNames: ["By Recently Played", "By Title", "By Play Time", "By Release"]

    readonly property var hints: tab === 1 || zone === "rail" ? [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]
                               : [ { glyph: "Start", label: "Options" }, { glyph: "B", label: "Back" }, { glyph: "A", label: "Start" } ]

    readonly property real gridX: Theme.dp(253)
    readonly property real gridY: Theme.dp(190)

    focus: true

    SearchGames {
        id: matches
        sourceModel: api.allGames
        query: page.query
    }

    LibraryGames {
        id: sorted
        sourceModel: matches
        sortMode: page.sortMode
    }

    readonly property var groupList: Groups.groups(api.allGames, api.collections)

    function focusZone() {
        if (tab === 1)
            groupGrid.forceActiveFocus();
        else if (zone === "rail")
            rail.forceActiveFocus();
        else
            softwareGrid.forceActiveFocus();
    }

    function railAction(id) {
        if (id === "search") {
            shell.prompt({ title: "Search", value: query, max: 32 }, function(value) {
                if (value !== null) {
                    query = value;
                    softwareGrid.index = 0;
                }
            });
        } else if (id === "sort") {
            shell.pick({ title: "Sort", choices: sortNames, index: sortMode }, function(i) {
                if (i >= 0) {
                    sortMode = i;
                    softwareGrid.index = 0;
                }
            });
        }
    }

    onActiveFocusChanged: if (activeFocus) focusZone()

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isPrevPage(event)) {
            event.accepted = true;
            tabs.step(-1);
        } else if (api.keys.isNextPage(event)) {
            event.accepted = true;
            tabs.step(1);
        }
    }

    Tabs {
        id: tabs
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        names: ["Software", "Groups"]
        onChanged: function(i) {
            page.tab = i;
            page.focusZone();
        }
    }

    Item {
        anchors.fill: parent
        visible: page.tab === 0

        Rail {
            id: rail
            x: Theme.dp(102)
            y: Theme.dp(170)
            height: parent.height - y - Theme.dp(Theme.hintBarHeight)
            items: [ { id: "search", icon: "search" }, { id: "sort", icon: "sort" } ]
            focus: page.zone === "rail"
            onActivated: function(id) { page.railAction(id); }
            onEscapedRight: {
                page.zone = "grid";
                softwareGrid.forceActiveFocus();
            }
        }

        Label {
            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(160)
            y: Theme.dp(140)
            text: page.sortNames[page.sortMode] + (page.query !== "" ? "  ·  “" + page.query + "”" : "")
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        SoftwareGrid {
            id: softwareGrid
            x: page.gridX
            y: page.gridY
            width: implicitWidth
            height: parent.height - y - Theme.dp(Theme.hintBarHeight)
            games: sorted
            focus: page.zone === "grid"
            onEscapedLeft: {
                page.zone = "rail";
                rail.forceActiveFocus();
            }
            onActivated: page.shell.launch(current)
            onOptionsRequested: page.shell.push("pages/SoftwareOptionsPage.qml", { gameId: current.id })
        }
    }

    SoftwareGrid {
        id: groupGrid
        x: page.gridX
        y: page.gridY
        width: implicitWidth
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        visible: page.tab === 1
        games: page.groupList
        groups: true
        escapesLeft: false
        focus: page.tab === 1
        onActivated: page.shell.push("pages/GroupPage.qml", { group: current.key, name: current.name })
    }
}
