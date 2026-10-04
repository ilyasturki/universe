import QtQuick
import Universe
import "../core"
import "../sound"
import "../ui"
import "Groups.js" as Groups
import "Home.js" as Home

// The console's Game Library: the whole collection in a grid of five, a rail of search, sort and filter, and add.
FocusScope {
    id: page

    objectName: "libraryPage"
    property var shell: null
    property var args: ({})
    readonly property var hints: []
    readonly property bool strip: false
    signal closeRequested
    focus: true

    // 0 Your Collection, 1 Favourites, 2 Gamelists
    property int tab: 0
    // "grid", "tabs", "rail" or "drawer"
    property string zone: "grid"
    property string query: ""
    property int sortMode: 0
    property string platform: ""
    property string source: ""
    property string genre: ""
    // A gamelist open in the Gamelists tab, by its key; its games fill the grid.
    property string openList: ""
    property string openListName: ""
    property int listIndex: 0

    readonly property var tabNames: ["Your Collection", "Favourites", "Gamelists"]
    readonly property var sorts: [
        {
            label: "Most Recent",
            role: "recentAt",
            desc: true
        },
        {
            label: "Name (A - Z)",
            role: "sortTitle",
            desc: false
        },
        {
            label: "Name (Z - A)",
            role: "sortTitle",
            desc: true
        },
        {
            label: "Recently Added",
            role: "addedAt",
            desc: true
        },
        {
            label: "Play Time",
            role: "playTime",
            desc: true
        },
        {
            label: "Release Date",
            role: "releaseYear",
            desc: true
        }
    ]
    readonly property bool filtering: platform !== "" || source !== "" || genre !== ""

    readonly property var session: api.universe.currentSession
    readonly property string playingId: session && session.id !== undefined ? session.id : ""

    SearchGames {
        id: matches
        sourceModel: api.allGames
        query: page.query
    }

    SortedGames {
        id: sorted
        sourceModel: matches
        sortRoleName: page.sorts[page.sortMode].role
        descending: page.sorts[page.sortMode].desc
    }

    // The proxy's rows as a list, rebuilt when it changes: the filters below read games, not rows.
    property var sortedList: []

    function rebuild() {
        var out = [];
        for (var i = 0; i < sorted.count; i++) {
            var g = sorted.get(i);
            if (g)
                out.push(g);
        }
        sortedList = out;
    }

    Connections {
        target: sorted
        function onCountChanged() {
            Qt.callLater(page.rebuild);
        }
    }

    Component.onCompleted: rebuild()

    function platformOf(g) {
        return g.platform || g.source || "";
    }

    readonly property var filtered: sortedList.filter(function (g) {
        return (page.platform === "" || page.platformOf(g) === page.platform) && (page.source === "" || g.source === page.source) && (page.genre === "" || g.genreList.indexOf(page.genre) >= 0);
    })

    readonly property var gamelists: Groups.groups(api.allGames, api.collections).filter(function (l) {
        return l.games.length > 0;
    })

    readonly property var listGames: {
        if (openList === "")
            return [];
        var ids = {};
        Groups.gamesOf(api.allGames, api.collections, openList).forEach(function (g) {
            ids[g.id] = true;
        });
        return filtered.filter(function (g) {
            return ids[g.id] === true;
        });
    }

    readonly property bool showingLists: tab === 2 && openList === ""
    readonly property var items: tab === 0 ? filtered : tab === 1 ? filtered.filter(function (g) {
        return g.favorite;
    }) : openList !== "" ? listGames : gamelists

    readonly property string countText: tab === 0 ? "All: " + items.length : tab === 1 ? "Favourites: " + items.length : openList !== "" ? openListName + ": " + items.length : "Gamelists: " + items.length
    readonly property string sortText: showingLists ? "" : "Sort by: " + sorts[sortMode].label + (filtering ? "  ·  Filtered" : "") + (query !== "" ? "  ·  “" + query + "”" : "")

    // What each filter can be set to: the values the library holds.
    function valuesOf(kind) {
        var seen = {}, out = [];
        sortedList.forEach(function (g) {
            var values = kind === "platform" ? [platformOf(g)] : kind === "source" ? [g.source] : g.genreList;
            values.forEach(function (v) {
                if (v && !seen[v]) {
                    seen[v] = true;
                    out.push(v);
                }
            });
        });
        out.sort(function (a, b) {
            return a.toLowerCase() < b.toLowerCase() ? -1 : 1;
        });
        return out;
    }

    function sourceName(id) {
        var names = {
            gog: "GOG",
            steam: "Steam",
            epic: "Epic",
            itch: "itch.io",
            lutris: "Lutris",
            local: "This machine"
        };
        return names[id] || (id ? id.charAt(0).toUpperCase() + id.slice(1) : "");
    }

    readonly property var drawerRows: [
        {
            id: "sort",
            label: "Sort by",
            value: sorts[sortMode].label
        },
        {
            caption: true,
            label: "Filters"
        },
        {
            id: "platform",
            label: "Platform",
            value: platform === "" ? "All" : platform
        },
        {
            id: "source",
            label: "Source",
            value: source === "" ? "All" : sourceName(source)
        },
        {
            id: "genre",
            label: "Genre",
            value: genre === "" ? "All" : genre
        },
        {
            id: "reset",
            label: "Reset Filters",
            button: true,
            disabled: !filtering
        }
    ]

    function focusZone() {
        if (zone === "drawer")
            drawer.forceActiveFocus();
        else if (zone === "rail")
            rail.forceActiveFocus();
        else
            grid.forceActiveFocus();
    }

    onActiveFocusChanged: if (activeFocus)
        focusZone()

    function switchTab(t) {
        if (t < 0 || t >= tabNames.length || t === tab) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        tab = t;
        openList = "";
        grid.index = 0;
    }

    function openGamelist(i) {
        var l = gamelists[i];
        if (!l)
            return;
        listIndex = i;
        openListName = l.name;
        openList = l.key;
        grid.index = 0;
    }

    function closeGamelist() {
        Sound.play("back");
        openList = "";
        Qt.callLater(function () {
            grid.index = page.listIndex;
        });
    }

    function railAction(id) {
        if (id === "search") {
            shell.prompt({
                title: "Search your library",
                value: query,
                max: 40
            }, function (value) {
                if (value === null)
                    return;
                query = value;
                grid.index = 0;
                page.zone = "grid";
            });
        } else if (id === "sort") {
            drawer.reset();
            zone = "drawer";
            drawer.forceActiveFocus();
        } else if (id === "add") {
            shell.push("pages/AddGamePage.qml", {});
        }
    }

    function drawerAction(id) {
        var at = function (row) {
            var p = drawer.panel.mapToItem(page.shell, drawer.panel.width + Theme.dp(14), drawer.rowY(row) - Theme.dp(10));
            return {
                x: p.x,
                y: p.y
            };
        };
        if (id === "sort") {
            shell.pick({
                title: "Sort by",
                choices: sorts.map(function (s) {
                    return s.label;
                }),
                index: sortMode,
                at: at(0)
            }, function (i) {
                if (i >= 0 && i !== sortMode) {
                    sortMode = i;
                    grid.index = 0;
                }
            });
        } else if (id === "platform" || id === "source" || id === "genre") {
            var values = valuesOf(id);
            var current = id === "platform" ? platform : id === "source" ? source : genre;
            var row = id === "platform" ? 2 : id === "source" ? 3 : 4;
            shell.pick({
                title: id === "platform" ? "Platform" : id === "source" ? "Source" : "Genre",
                choices: ["All"].concat(values.map(function (v) {
                    return id === "source" ? sourceName(v) : v;
                })),
                index: current === "" ? 0 : values.indexOf(current) + 1,
                at: at(row)
            }, function (i) {
                if (i < 0)
                    return;
                var v = i === 0 ? "" : values[i - 1];
                if (id === "platform")
                    platform = v;
                else if (id === "source")
                    source = v;
                else
                    genre = v;
                grid.index = 0;
            });
        } else if (id === "reset") {
            platform = "";
            source = "";
            genre = "";
            grid.index = 0;
        }
    }

    // A: the console takes the game to its hub on Home; without that hook the game starts.
    function openGame(game) {
        if (!game)
            return;
        if (shell.openGame)
            shell.openGame(game.id);
        else if (game.id === playingId)
            shell.resume();
        else
            shell.launch(game);
    }

    function gameMenu(game) {
        if (!game || game.installing) {
            Sound.play("edge");
            return;
        }
        var items = Home.options(game, game.id === playingId);
        var id = game.id;
        Sound.play("open");
        shell.showMenu({
            items: items
        }, function (i) {
            if (i >= 0)
                shell.gameOption(items[i].act, id);
        });
    }

    function toggleFavourite(game) {
        if (!game || game.installing) {
            Sound.play("edge");
            return;
        }
        game.favorite = !game.favorite;
        Sound.play("select");
        shell.showToast(game.favorite ? "Added " + game.title + " to Favourites" : "Removed " + game.title + " from Favourites");
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (zone === "drawer")
            return;
        if (api.keys.isPrevPage(event) || api.keys.isNextPage(event)) {
            event.accepted = true;
            switchTab(tab + (api.keys.isPrevPage(event) ? -1 : 1));
        } else if (api.keys.isCancel(event) && openList !== "") {
            event.accepted = true;
            closeGamelist();
        } else if (api.keys.isCancel(event) && zone !== "grid") {
            event.accepted = true;
            Sound.play("back");
            zone = "grid";
            grid.forceActiveFocus();
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    LibraryGrid {
        id: grid

        x: Theme.dp(Theme.edge)
        y: Theme.dp(120)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: parent.height - y
        items: page.items
        lists: page.showingLists
        playingId: page.playingId
        tabNames: page.tabNames
        tab: page.tab
        tabsActive: page.zone === "tabs" && page.activeFocus
        countText: page.countText
        sortText: page.sortText
        emptyText: page.showingLists ? "Gamelists gather your games: your favourites, each platform, each tag you give a game." : page.query !== "" || page.filtering ? "No game matches." : page.tab === 1 ? "No favourites yet: Options on a game, then Favourite." : "Nothing in the library yet. Add Game, on the left, takes a file on this machine, a store's games or your Lutris library."
        focus: page.zone === "grid" || page.zone === "tabs"

        onEscapedLeft: {
            page.zone = "rail";
            rail.forceActiveFocus();
        }
        onEscapedUp: {
            page.zone = "tabs";
        }
        onPointed: page.zone = "grid"
        onActivated: function (i) {
            if (page.showingLists)
                page.openGamelist(i);
            else
                page.openGame(page.items[i]);
        }
        onMenuRequested: function (i) {
            page.gameMenu(page.items[i]);
        }
        onInfoRequested: function (i) {
            Sound.play("ok");
            page.shell.push("pages/SoftwareInfoPage.qml", {
                gameId: page.items[i].id
            });
        }
        onFavouriteRequested: function (i) {
            page.toggleFavourite(page.items[i]);
        }
        onTabPicked: function (i) {
            if (page.zone === "tabs" && i === page.tab)
                return;
            page.zone = "tabs";
            page.switchTab(i);
            grid.forceActiveFocus();
        }

        onTabStepped: function (d) {
            page.switchTab(page.tab + d);
        }
        onTabsLeft: page.zone = "grid"
    }

    // The console's page title stays put while the tabs and the rows scroll under it.
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        height: Theme.dp(128)
        gradient: Gradient {
            GradientStop {
                position: 0.0
                color: Qt.rgba(0.137, 0.153, 0.184, 1.0)
            }
            GradientStop {
                position: 0.75
                color: Qt.rgba(0.12, 0.135, 0.165, 0.92)
            }
            GradientStop {
                position: 1.0
                color: Qt.rgba(0.12, 0.135, 0.165, 0.0)
            }
        }
    }

    PageTitle {
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "library"
        title: page.openList !== "" ? page.openListName : "Game Library"
    }

    LibraryRail {
        id: rail

        x: Theme.dp(48)
        y: Theme.dp(309)
        items: [
            {
                id: "search",
                glyph: "search",
                label: page.query !== "" ? "Search · “" + page.query + "”" : "Search"
            },
            {
                id: "sort",
                glyph: "sort",
                label: "Sort and Filter"
            },
            {
                id: "add",
                glyph: "list-add",
                label: "Add Game"
            }
        ]
        focus: page.zone === "rail"

        onActivated: function (id) {
            page.railAction(id);
        }
        onEscapedRight: {
            page.zone = "grid";
            grid.forceActiveFocus();
        }
        onPointed: page.zone = "rail"
    }

    LibraryDrawer {
        id: drawer

        z: 5
        open: page.zone === "drawer"
        rows: page.drawerRows
        panelY: rail.y + rail.pitch - Theme.dp(12)

        onActivated: function (id) {
            page.drawerAction(id);
        }
        onClosed: {
            page.zone = "rail";
            rail.forceActiveFocus();
        }
    }
}
