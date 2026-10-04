import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../core/Format.js" as Format
import "Home.js" as Home

// The Store: the linked store's games as the console's collections under a hero for the focused one, a game's card
// with its Install on A, and Downloads — what installs, what waits for an update, what is on the disk.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    signal closeRequested
    focus: true

    readonly property var sources: api.screens.sources
    // 0 the store's games, 1 Downloads.
    property int tab: 0
    // "top" (the tabs and the icons), "main" (the collections or the downloads) or "card" (a game's card).
    property string zone: "main"
    property int topIndex: 0
    property int row: 0
    property var cols: ({})
    property int lineIndex: 0
    // The store id the cursor is on, kept across the listing's reloads.
    property string focusId: ""
    property string cardId: ""
    property int cardIndex: 0

    readonly property bool strip: false
    readonly property bool cardOpen: zone === "card"

    readonly property string sourceName: sources.current ? sources.current.name : (sources.source || "Store")
    readonly property bool linked: sources.sources.length > 0
    readonly property bool loggedIn: sources.current ? sources.current.logged_in === true : false
    readonly property string gamesDir: sources.current && sources.current.games_dir ? sources.current.games_dir : ""
    readonly property bool running: sources.job !== null && sources.job !== undefined && (sources.job.ok === null || sources.job.ok === undefined)
    readonly property string libraryLine: sources.error !== "" ? sourceName + " unreachable · listing from " + (sources.libraryAge || "before") : sources.busy ? "Loading…" : !linked ? "" : (loggedIn ? "Signed in" : "Not signed in") + (sources.libraryAge ? " · refreshed " + sources.libraryAge : "")

    readonly property var tabs: [linked ? sourceName : "Store", "Downloads"]
    readonly property var icons: (sources.stores.length > 1 ? [
            {
                id: "store",
                glyph: "store",
                label: "Change Store"
            }
        ] : []).concat([
        {
            id: "search",
            glyph: "search",
            label: "Search"
        },
        {
            id: "refresh",
            glyph: "refresh",
            label: "Refresh"
        },
        {
            id: "sources",
            glyph: "settings",
            label: "Sources"
        },
        {
            id: "options",
            glyph: "more",
            label: "Options"
        }
    ])

    // The store's games: the search's results alone while there is one, else what could install, what waits, what is in.
    readonly property var collections: {
        var rows = sources.rows;
        var all = rows.map(function (r, i) {
            return i;
        });
        if (sources.query)
            return all.length > 0 ? [
                {
                    id: "results",
                    title: "Results for “" + sources.query + "”",
                    items: all
                }
            ] : [];
        var out = [];
        var ready = all.filter(function (i) {
            return !rows[i].installed;
        });
        var updates = all.filter(function (i) {
            return rows[i].installed && rows[i].pending;
        });
        var installed = all.filter(function (i) {
            return rows[i].installed;
        });
        if (ready.length > 0)
            out.push({
                id: "ready",
                title: "Ready to install",
                items: ready
            });
        if (updates.length > 0)
            out.push({
                id: "updates",
                title: "Updates",
                items: updates
            });
        if (installed.length > 0)
            out.push({
                id: "installed",
                title: "Installed",
                items: installed
            });
        return out;
    }
    readonly property var collection: collections.length > 0 ? collections[Math.max(0, Math.min(row, collections.length - 1))] : null
    readonly property int col: collection ? Math.max(0, Math.min(cols[collection.id] || 0, collection.items.length - 1)) : 0
    readonly property int currentRow: {
        var c = collection;
        if (!c || c.items.length === 0)
            return -1;
        return c.items[Math.max(0, Math.min(cols[c.id] || 0, c.items.length - 1))];
    }
    readonly property var current: currentRow >= 0 && currentRow < sources.rows.length ? sources.rows[currentRow] : null

    readonly property real onDisk: sources.rows.reduce(function (sum, r) {
        return sum + (r.installed ? r.disk_size : 0);
    }, 0)

    // Downloads: the jobs (running or stopped) first, then the updates, then what is installed.
    readonly property var lines: {
        var rows = sources.rows, out = [], jobs = [], installed = [], busy = 0, paused = 0;
        for (var i = 0; i < rows.length; i++) {
            if (rows[i].busy || rows[i].partial) {
                jobs.push(i);
                rows[i].busy ? busy++ : paused++;
            } else if (rows[i].installed)
                installed.push(i);
        }
        if (jobs.length > 0) {
            out.push({
                heading: true,
                label: "Installing",
                meta: [busy > 0 ? busy + " running" : "", paused > 0 ? paused + " paused" : ""].filter(Boolean).join(" · ")
            });
            jobs.forEach(function (k) {
                out.push({
                    row: k,
                    game: rows[k]
                });
            });
        }
        var ups = sources.updates;
        if (ups.length > 0) {
            out.push({
                heading: true,
                label: "Updates",
                meta: Format.plural(ups.length, "update", "updates")
            });
            out.push({
                all: true
            });
            ups.forEach(function (u, k) {
                out.push({
                    update: k,
                    item: u
                });
            });
        }
        out.push({
            heading: true,
            label: "Installed",
            meta: Format.plural(installed.length, "game", "games") + (page.onDisk > 0 ? " · " + Format.bytes(page.onDisk) : "")
        });
        installed.forEach(function (k) {
            out.push({
                row: k,
                game: rows[k]
            });
        });
        return out;
    }
    readonly property var line: lineIndex >= 0 && lineIndex < lines.length && !lines[lineIndex].heading ? lines[lineIndex] : null
    readonly property var lineGame: line && line.row !== undefined ? line.game : null

    readonly property int cardRow: {
        var id = cardId;
        return id === "" ? -1 : sources.rows.findIndex(function (r) {
            return r.id === id;
        });
    }
    readonly property var cardEntry: cardRow >= 0 ? sources.rows[cardRow] : null
    readonly property var cardActions: cardEntry ? actionsFor(cardEntry) : []

    readonly property var hints: {
        var out = [
            {
                glyph: "Y",
                label: "Refresh"
            }
        ];
        if (running)
            out.push({
                glyph: "X",
                label: "Cancel " + (sources.job.label.indexOf("Updating") === 0 ? "update" : "install")
            });
        out.push({
            glyph: "Start",
            label: "Options"
        }, {
            glyph: "B",
            label: "Back"
        });
        if (zone === "card" && cardActions.length > 0)
            out.push({
                glyph: "A",
                label: cardActions[Math.min(cardIndex, cardActions.length - 1)].label
            });
        else if (zone === "main" && tab === 0 && current)
            out.push({
                glyph: "A",
                label: "Open"
            });
        else if (zone === "main" && tab === 1 && line)
            out.push({
                glyph: "A",
                label: line.all ? "Update everything" : line.update !== undefined ? "Update" : "Options"
            });
        else
            out.push({
                glyph: "A",
                label: "OK"
            });
        return out;
    }

    readonly property real contentTop: Theme.dp(226)

    Component.onCompleted: sources.load()

    // Opened onto Downloads from HOME's arriving tile.
    onArgsChanged: if (args && args.tab)
        setTab(args.tab, true)

    onCurrentChanged: {
        if (current) {
            focusId = current.id;
            if (!current.installed && !current.download_size)
                sources.peek(currentRow);
        }
    }

    onCollectionsChanged: Qt.callLater(restore)
    onLinesChanged: {
        if (!line)
            lineIndex = firstLine();
    }
    onLineIndexChanged: downloads.reveal()

    // The listing came back (an install landed, a search): the cursor stays on the same game where it still is.
    function restore() {
        if (focusId === "")
            return;
        for (var k = 0; k < collections.length; k++) {
            var items = collections[k].items;
            for (var i = 0; i < items.length; i++) {
                var r = sources.rows[items[i]];
                if (r && r.id === focusId) {
                    row = k;
                    setCol(collections[k].id, i);
                    return;
                }
            }
        }
        row = Math.min(row, Math.max(0, collections.length - 1));
    }

    function firstLine() {
        for (var i = 0; i < lines.length; i++)
            if (!lines[i].heading)
                return i;
        return 0;
    }

    function setCol(id, i) {
        var next = Object.assign({}, cols);
        next[id] = i;
        cols = next;
    }

    function colOf(id) {
        return cols[id] || 0;
    }

    function setTab(t, quiet) {
        if (t < 0 || t >= tabs.length) {
            Sound.play("edge");
            return;
        }
        if (!quiet)
            Sound.play(t === tab ? "edge" : "tick");
        tab = t;
        topIndex = t;
        if (zone === "card")
            zone = "main";
    }

    // How far a download got: the running job's bytes, else what a stopped one kept.
    function fractionOf(g) {
        if (!g)
            return 0;
        if (g.busy && running && sources.job.game === g.id && sources.job.total > 0)
            return sources.job.done / sources.job.total;
        return g.disk_size > 0 ? g.partial_bytes / g.disk_size : 0;
    }

    function isLive(g) {
        return g !== null && g.busy && running && sources.job.game === g.id;
    }

    // The console's price line: what this game is to you and how much it takes.
    function statusOf(g) {
        if (!g)
            return "";
        if (isLive(g))
            return sources.job.message.replace(sources.job.label + " · ", "");
        if (g.busy || g.partial)
            return g.status;
        if (g.installed)
            return (g.pending ? "Update available" : "Installed") + (g.sizeText ? " · " + g.sizeText : "");
        if (g.status === "Not owned")
            return "Not owned on " + sourceName;
        return "Owned on " + sourceName + (g.download_size > 0 ? " · " + Format.bytes(g.download_size) : "");
    }

    function sizesOf(g) {
        if (!g)
            return "";
        var parts = [];
        if (!g.installed && g.download_size > 0)
            parts.push(Format.bytes(g.download_size) + " to download");
        if (g.disk_size > 0)
            parts.push(Format.bytes(g.disk_size) + " on disk");
        if (parts.length === 0 && !g.installed)
            parts.push("Size not known yet");
        return parts.join(" · ");
    }

    function libraryGameOf(g) {
        return g && g.game_id !== "" ? api.allGames.byId(g.game_id) : null;
    }

    function mainOf(g) {
        if (g.busy)
            return {
                act: "cancel",
                label: "Cancel",
                glyph: "stop"
            };
        if (g.partial)
            return {
                act: "resume-download",
                label: "Resume",
                glyph: "download"
            };
        if (g.pending)
            return {
                act: "update",
                label: "Update",
                glyph: "download"
            };
        if (!g.installed && g.status !== "Not owned")
            return {
                act: "install",
                label: "Install",
                glyph: "download"
            };
        if (g.installed && libraryGameOf(g))
            return {
                act: "play",
                label: "Play",
                glyph: "play"
            };
        return null;
    }

    function moreOf(g) {
        var game = libraryGameOf(g);
        if (!game)
            return [];
        var session = api.universe.currentSession;
        var main = mainOf(g);
        // Play once: the card's own button when that is Play, nowhere before the files are there.
        var items = Home.options(game, !!session && session.id === game.id).filter(function (i) {
            return i.act !== "play" || (g.installed && (!main || main.act !== "play"));
        });
        if (g.installed) {
            items[items.length - 1].gap = false;
            items.splice(items.length - 1, 0, {
                label: "Uninstall…",
                glyph: "trash",
                act: "uninstall",
                gap: true
            });
        }
        return items;
    }

    function actionsFor(g) {
        var main = mainOf(g);
        var out = main ? [main] : [];
        if (moreOf(g).length > 0)
            out.push({
                act: "more",
                label: "Options",
                glyph: "more",
                round: true
            });
        return out;
    }

    function askInstall(g, index) {
        var parts = [];
        if (g.download_size > 0)
            parts.push(Format.bytes(g.download_size) + " to download");
        if (g.disk_size > 0)
            parts.push(Format.bytes(g.disk_size) + " on disk");
        var free = sources.freeSpace > 0 ? Format.bytes(sources.freeSpace) + " free" + (gamesDir ? " in " + gamesDir : "") : gamesDir ? "Into " + gamesDir : "";
        shell.dialogAsk({
            message: "Install " + g.title + "?",
            detail: [parts.length > 0 ? parts.join(" · ") : "Size not known yet", free].filter(Boolean).join("\n"),
            buttons: ["Not Now", "Install"]
        }, function (k) {
            if (k === 1)
                Sound.play(sources.install(index) !== "" ? "ok" : "edge");
        });
    }

    function cancel() {
        if (!running) {
            Sound.play("edge");
            return;
        }
        sources.cancel() ? Sound.play("back") : Sound.play("edge");
    }

    function perform(id, g, index) {
        if (!g)
            return;
        var title = g.title, gameId = g.game_id;
        if (id === "install") {
            Sound.play("ok");
            askInstall(g, index);
        } else if (id === "resume-download" || id === "update") {
            Sound.play(sources.install(index) !== "" ? "ok" : "edge");
        } else if (id === "cancel") {
            cancel();
        } else if (id === "more") {
            Sound.play("open");
            var items = moreOf(g);
            shell.showMenu({
                title: title,
                items: items
            }, function (i) {
                if (i >= 0)
                    page.perform(items[i].act, g, index);
            });
        } else if (id === "uninstall") {
            var via = api.universe.uninstallVia(gameId);
            shell.dialogAsk({
                message: "Uninstall " + title + "?",
                detail: (via ? via + " removes the files" : "The install folder goes to the trash") + "; the hours and the journal stay.",
                buttons: ["Cancel", "Uninstall"],
                danger: 1,
                index: 0
            }, function (k) {
                if (k === 1)
                    sources.uninstall(gameId);
            });
        } else if (id === "remove") {
            shell.dialogAsk({
                message: "Remove " + title + " from the library?",
                detail: "The entry is archived; the files are left where they are.",
                buttons: ["Cancel", "Remove"],
                danger: 1,
                index: 0
            }, function (k) {
                if (k === 1)
                    sources.remove(gameId);
            });
        } else {
            shell.gameOption(id, gameId);
        }
    }

    function openCard(g) {
        if (!g) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        cardId = g.id;
        cardIndex = 0;
        zone = "card";
    }

    function closeCard() {
        Sound.play("back");
        zone = "main";
    }

    function openSearch() {
        Sound.play("ok");
        shell.prompt({
            title: "Search " + sourceName,
            value: sources.query
        }, function (q) {
            if (q === null)
                return;
            sources.search(q);
            row = 0;
            setTab(0, true);
            zone = "main";
        });
    }

    function refresh() {
        Sound.play("ok");
        sources.refresh();
    }

    function openSources() {
        Sound.play("ok");
        shell.push("pages/SettingsPage.qml", {
            section: "sources"
        });
    }

    function openStores() {
        Sound.play("ok");
        shell.menu("Store", sources.stores.map(function (s) {
            return {
                label: s.name,
                glyph: s.id === sources.source ? "check" : "store",
                act: s.id
            };
        }), function (id) {
            if (id === sources.source)
                return;
            sources.pick(id);
            row = 0;
            setTab(0, true);
            zone = "main";
        });
    }

    function topAction(i) {
        if (i < tabs.length) {
            setTab(i);
            zone = "main";
            return;
        }
        var id = icons[i - tabs.length].id;
        if (id === "store")
            openStores();
        else if (id === "search")
            openSearch();
        else if (id === "refresh")
            refresh();
        else if (id === "sources")
            openSources();
        else
            options();
    }

    // A line of Downloads: an update of its own, all of them, or a game's options.
    function activateLine() {
        var l = line;
        if (!l) {
            Sound.play("edge");
            return;
        }
        if (l.all) {
            Sound.play(sources.updateAll() !== "" ? "ok" : "edge");
            return;
        }
        if (l.update !== undefined) {
            Sound.play(sources.update(l.update) !== "" ? "ok" : "edge");
            return;
        }
        openCard(l.game);
    }

    // Start: every action of the page, the focused game's first.
    function options() {
        var g = zone === "card" ? cardEntry : tab === 0 ? current : lineGame;
        var index = zone === "card" ? cardRow : tab === 0 ? currentRow : line ? line.row : -1;
        var main = g ? mainOf(g) : null;
        var items = (main ? [main] : []).concat(g ? moreOf(g) : []);
        var general = [
            {
                label: "Search…",
                glyph: "search",
                act: "search"
            }
        ];
        if (sources.query !== "")
            general.push({
                label: "Clear the Search",
                glyph: "cross",
                act: "clear"
            });
        if (running && !(g && g.busy))
            general.push({
                label: sources.job.label.indexOf("Updating") === 0 ? "Cancel the Update" : "Cancel the Install",
                glyph: "stop",
                act: "cancel-job"
            });
        if (sources.updates.length > 0)
            general.push({
                label: "Update Everything",
                glyph: "download",
                act: "update-all"
            });
        if (sources.stores.length > 1)
            general.push({
                label: "Change Store",
                glyph: "store",
                act: "store"
            });
        general.push({
            label: tab === 0 ? "Downloads" : page.tabs[0],
            glyph: tab === 0 ? "download" : "store",
            act: "tab"
        }, {
            label: "Refresh",
            glyph: "refresh",
            act: "refresh"
        }, {
            label: "Sources Settings",
            glyph: "settings",
            act: "sources"
        });
        general[0].gap = items.length > 0;
        items = items.concat(general);
        shell.showMenu({
            title: g ? g.title : sourceName,
            items: items
        }, function (i) {
            if (i < 0)
                return;
            var act = items[i].act;
            if (act === "search")
                page.openSearch();
            else if (act === "clear")
                sources.search("");
            else if (act === "cancel-job")
                page.cancel();
            else if (act === "update-all")
                Sound.play(sources.updateAll() !== "" ? "ok" : "edge");
            else if (act === "store")
                page.openStores();
            else if (act === "tab")
                page.setTab(page.tab === 0 ? 1 : 0);
            else if (act === "refresh")
                page.refresh();
            else if (act === "sources")
                page.openSources();
            else
                page.perform(act, g, index);
        });
    }

    function stepRow(d) {
        var next = row + d;
        if (next < 0) {
            Sound.play("tick");
            topIndex = tab;
            zone = "top";
            return;
        }
        if (next >= collections.length) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        row = next;
    }

    function stepCol(d) {
        if (!collection)
            return;
        var next = Sound.stepped(col, d, collection.items.length);
        setCol(collection.id, next);
    }

    function stepLine(d) {
        var stops = [];
        for (var i = 0; i < lines.length; i++)
            if (!lines[i].heading)
                stops.push(i);
        var pos = stops.indexOf(lineIndex) + d;
        if (pos < 0) {
            Sound.play("tick");
            topIndex = tab;
            zone = "top";
            return;
        }
        if (pos >= stops.length) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        lineIndex = stops[pos];
    }

    function pointTile(k, i) {
        if (zone === "main" && tab === 0 && row === k && col === i) {
            openCard(current);
            return;
        }
        Sound.play("tick");
        zone = "main";
        row = k;
        setCol(collections[k].id, i);
        page.forceActiveFocus();
    }

    function pointLine(i) {
        if (zone === "main" && tab === 1 && lineIndex === i) {
            activateLine();
            return;
        }
        Sound.play("tick");
        zone = "main";
        lineIndex = i;
        page.forceActiveFocus();
    }

    function pointTop(i) {
        if (zone === "top" && topIndex === i) {
            topAction(i);
            return;
        }
        Sound.play("tick");
        zone = "top";
        topIndex = i;
        page.forceActiveFocus();
    }

    Connections {
        target: page.sources
        function onMessage(text) {
            page.shell.showToast(text);
        }
    }

    // The console's buttons over the Store's near-black ground: a faint light pill under the glass one.
    component StoreButton: Item {
        id: sb

        property string text: ""
        property string glyph: ""
        property bool focused: false
        property bool danger: false
        property bool round: false
        // Off, a tap only picks: the hero's buttons act on the pick themselves, no A follows.
        property bool direct: true

        signal picked

        width: round ? height : button.implicitWidth
        height: Theme.dp(64)

        Rectangle {
            anchors.fill: parent
            radius: height / 2
            visible: !sb.focused
            color: Qt.rgba(1, 1, 1, 0.1)
        }

        PillButton {
            id: button
            anchors.fill: parent
            text: sb.text
            glyph: sb.glyph
            focused: sb.focused
            danger: sb.danger
            round: sb.round
            fontSize: Theme.dp(26)
            direct: sb.direct
            onPicked: sb.picked()
        }
    }

    Keys.onPressed: function (event) {
        var horizontal = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        if (event.isAutoRepeat && !horizontal && !vertical)
            return;
        var d = event.key === Qt.Key_Left || event.key === Qt.Key_Up ? -1 : 1;
        if (api.keys.isPrevPage(event) || api.keys.isNextPage(event)) {
            event.accepted = true;
            setTab(api.keys.isPrevPage(event) ? 0 : 1);
            return;
        }
        if (api.keys.isFilters(event)) {
            event.accepted = true;
            refresh();
            return;
        }
        if (api.keys.isDetails(event)) {
            event.accepted = true;
            cancel();
            return;
        }
        if (api.keys.isMenu(event)) {
            event.accepted = true;
            options();
            return;
        }
        if (zone === "card") {
            event.accepted = true;
            if (horizontal)
                cardIndex = Sound.stepped(cardIndex, d, cardActions.length);
            else if (vertical)
                Sound.play("edge");
            else if (api.keys.isAccept(event)) {
                var a = cardActions[cardIndex];
                a ? perform(a.act, cardEntry, cardRow) : Sound.play("edge");
            } else if (api.keys.isCancel(event))
                closeCard();
            else
                event.accepted = false;
            return;
        }
        if (api.keys.isCancel(event) && sources.query !== "") {
            event.accepted = true;
            Sound.play("back");
            sources.search("");
            return;
        }
        if (zone === "top") {
            if (horizontal) {
                event.accepted = true;
                topIndex = Sound.stepped(topIndex, d, tabs.length + icons.length);
            } else if (event.key === Qt.Key_Down) {
                event.accepted = true;
                Sound.play("tick");
                zone = "main";
            } else if (event.key === Qt.Key_Up) {
                event.accepted = true;
                Sound.play("edge");
            } else if (api.keys.isAccept(event)) {
                event.accepted = true;
                topAction(topIndex);
            }
            return;
        }
        if (tab === 0) {
            if (horizontal) {
                event.accepted = true;
                stepCol(d);
            } else if (vertical) {
                event.accepted = true;
                if (collections.length === 0 && event.key === Qt.Key_Up) {
                    Sound.play("tick");
                    topIndex = tab;
                    zone = "top";
                } else if (collections.length === 0)
                    Sound.play("edge");
                else
                    stepRow(d);
            } else if (api.keys.isAccept(event)) {
                event.accepted = true;
                if (current)
                    openCard(current);
                else if (!linked || !loggedIn)
                    openSources();
                else
                    Sound.play("edge");
            }
            return;
        }
        if (vertical) {
            event.accepted = true;
            stepLine(d);
        } else if (horizontal) {
            event.accepted = true;
            Sound.play("edge");
        } else if (api.keys.isAccept(event)) {
            event.accepted = true;
            activateLine();
        }
    }

    StoreBackdrop {
        anchors.fill: parent
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "store"
        title: "Store"
        trailing: page.libraryLine
    }

    // The tabs; the icons at the right.
    Item {
        id: topRow

        y: Theme.dp(163) - height / 2
        width: parent.width
        height: Theme.dp(80)

        Row {
            x: Theme.dp(Theme.edge) - Theme.dp(28)
            height: parent.height
            spacing: Theme.dp(10)

            Repeater {
                model: page.tabs

                TabLabel {
                    text: modelData
                    current: index === page.tab
                    focused: page.zone === "top" && page.topIndex === index && page.activeFocus
                    boxHeight: parent.height
                    onPicked: page.pointTop(index)
                }
            }
        }

        Repeater {
            model: page.icons

            Item {
                id: icon

                readonly property int at: page.tabs.length + index
                readonly property bool focused: page.zone === "top" && page.topIndex === at && page.activeFocus

                x: topRow.width - Theme.dp(200 + (page.icons.length - 1 - index) * 104) - width / 2
                y: (topRow.height - height) / 2
                width: Theme.dp(66)
                height: width

                Rectangle {
                    anchors.fill: parent
                    radius: width / 2
                    color: "#ffffff"
                    opacity: icon.focused ? 1.0 : 0.0
                    scale: icon.focused ? 1.0 : 0.8

                    Behavior on opacity {
                        Ease {}
                    }
                    Behavior on scale {
                        Ease {}
                    }
                }

                Glyph {
                    anchors.centerIn: parent
                    width: Theme.dp(36)
                    height: width
                    kind: modelData.glyph
                    tint: icon.focused ? Theme.onLight : Theme.text
                }

                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.top: parent.bottom
                    anchors.topMargin: Theme.dp(8)
                    text: modelData.label
                    opacity: icon.focused ? 1.0 : 0.0
                    font.pixelSize: Theme.dp(Theme.fontTiny)

                    Behavior on opacity {
                        Ease {}
                    }
                }

                Touch {
                    current: icon.focused
                    onPicked: page.pointTop(icon.at)
                }
            }
        }
    }

    JobLine {
        id: jobLine
        x: Theme.dp(Theme.edge)
        y: page.contentTop
        width: parent.width - x - Theme.dp(Theme.columnRight)
        job: page.sources.job
    }

    // ---- The store's games ----

    Item {
        id: storeLayer

        y: page.contentTop + jobLine.height
        width: parent.width
        height: parent.height - y
        opacity: page.tab === 0 && !page.cardOpen ? 1.0 : 0.0
        visible: opacity > 0.01
        transform: Translate {
            x: page.tab === 0 ? 0 : -Theme.dp(60)

            Behavior on x {
                NumberAnimation {
                    duration: Theme.durTab
                    easing.type: Easing.OutCubic
                }
            }
        }

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durTab
                easing.type: Easing.OutCubic
            }
        }

        Column {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: -Theme.dp(80)
            width: parent.width - Theme.dp(Theme.edge * 2)
            spacing: Theme.dp(28)
            visible: page.collections.length === 0

            Label {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: page.sources.busy ? "Loading…" : !page.linked ? "No store linked yet" : !page.loggedIn ? "Sign in to " + page.sourceName + " to see your games" : page.sources.query !== "" ? "Nothing matched “" + page.sources.query + "”" : "Nothing here yet"
                wrapMode: Text.WordWrap
                font.weight: Font.Light
                font.pixelSize: Theme.dp(46)
            }

            Label {
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                visible: !page.sources.busy && (!page.linked || !page.loggedIn)
                text: "Link your stores in Settings › Sources; their games show up here to install."
                color: Theme.textSecondary
                wrapMode: Text.WordWrap
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            PillButton {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: !page.sources.busy && (!page.linked || !page.loggedIn)
                text: "Go to Settings"
                focused: page.zone === "main" && page.activeFocus
                touchCurrent: false
                onPicked: page.openSources()
            }
        }

        // The focused game, as the console's store hero: its name and what it is to you left, its picture right.
        Item {
            id: hero

            width: parent.width
            height: Theme.dp(300)
            visible: page.current !== null

            Item {
                id: heroArt

                x: parent.width - Theme.dp(Theme.columnRight) - width
                y: Theme.dp(10)
                width: Theme.dp(504)
                height: Theme.dp(284)

                StoreArt {
                    anchors.fill: parent
                    entry: page.current
                    radius: Theme.dp(4)
                    titleSize: Theme.dp(34)
                }
            }

            Column {
                x: Theme.dp(Theme.edge)
                y: Theme.dp(24)
                width: heroArt.x - x - Theme.dp(60)
                spacing: Theme.dp(12)

                Label {
                    width: parent.width
                    text: page.current ? page.current.title : ""
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    lineHeight: 0.95
                    font.weight: Font.Light
                    font.pixelSize: Theme.dp(56)
                }

                Label {
                    width: parent.width
                    text: page.statusOf(page.current)
                    color: page.current && (page.current.pending || page.current.busy) ? "#9cc8ff" : Theme.text
                    elide: Text.ElideRight
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(26)
                }

                Label {
                    width: parent.width
                    text: page.sizesOf(page.current)
                    color: Theme.textSecondary
                    elide: Text.ElideRight
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }

                Item {
                    width: 1
                    height: Theme.dp(8)
                }

                Row {
                    spacing: Theme.dp(18)

                    StoreButton {
                        readonly property var action: page.current ? page.mainOf(page.current) : null
                        visible: action !== null
                        text: action ? action.label : ""
                        glyph: action ? action.glyph : ""
                        direct: false
                        onPicked: page.perform(action.act, page.current, page.currentRow)
                    }

                    StoreButton {
                        round: true
                        glyph: "more"
                        direct: false
                        onPicked: page.openCard(page.current)
                    }
                }
            }
        }

        Flickable {
            id: rows

            readonly property real captionH: Theme.dp(40)
            readonly property real tileW: Theme.dp(296)
            readonly property real tileH: Theme.dp(167)
            readonly property real blockH: captionH + Theme.dp(14) + tileH + Theme.dp(44)

            y: Theme.dp(346)
            width: parent.width
            height: parent.height - y
            contentWidth: width
            contentHeight: page.collections.length * blockH + Theme.dp(40)
            interactive: false
            clip: true

            // A collection comes in whole: past the view's foot it scrolls to the top, as the console's store does.
            function reveal() {
                var top = page.row * blockH, bottom = top + blockH - Theme.dp(44);
                var last = Math.max(0, contentHeight - height);
                if (bottom > contentY + height)
                    contentY = Math.min(top, last);
                else if (top < contentY)
                    contentY = top;
            }

            Connections {
                target: page
                function onRowChanged() {
                    rows.reveal();
                }
            }

            Behavior on contentY {
                id: rowsEase
                NumberAnimation {
                    duration: Theme.durScroll
                    easing.type: Easing.OutCubic
                }
            }

            Repeater {
                model: page.collections

                Item {
                    id: block

                    readonly property int k: index
                    readonly property var coll: modelData
                    readonly property int at: page.colOf(coll.id)
                    readonly property bool active: page.zone === "main" && page.tab === 0 && page.row === k && page.activeFocus

                    y: k * rows.blockH
                    width: rows.width
                    height: rows.blockH
                    opacity: page.row === k || Math.abs(page.row - k) === 1 ? 1.0 : 0.4

                    Behavior on opacity {
                        NumberAnimation {
                            duration: Theme.durScroll
                        }
                    }

                    Label {
                        x: Theme.dp(Theme.edge)
                        height: rows.captionH
                        verticalAlignment: Text.AlignVCenter
                        text: block.coll.title
                        font.pixelSize: Theme.dp(26)
                    }

                    Label {
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(Theme.columnRight)
                        height: rows.captionH
                        verticalAlignment: Text.AlignVCenter
                        text: Format.plural(block.coll.items.length, "game", "games")
                        color: Theme.textMuted
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                    }

                    Flickable {
                        id: strip

                        readonly property real pitch: rows.tileW + Theme.dp(24)
                        readonly property real room: width - Theme.dp(Theme.edge + Theme.columnRight)

                        function place() {
                            var span = block.coll.items.length * strip.pitch - Theme.dp(24);
                            strip.contentX = Math.max(0, Math.min(block.at * strip.pitch, span - room));
                        }

                        y: rows.captionH + Theme.dp(14) - Theme.dp(10)
                        width: parent.width
                        height: rows.tileH + Theme.dp(20)
                        contentWidth: Theme.dp(Theme.edge) * 2 + block.coll.items.length * pitch
                        contentHeight: height
                        interactive: false
                        clip: false

                        Component.onCompleted: place()
                        onWidthChanged: place()

                        Connections {
                            target: block
                            function onAtChanged() {
                                strip.place();
                            }
                        }

                        Behavior on contentX {
                            id: stripEase
                            NumberAnimation {
                                duration: Theme.durMove
                                easing.type: Easing.OutCubic
                            }
                        }

                        Repeater {
                            model: block.coll.items

                            StoreTile {
                                readonly property var g: page.sources.rows[modelData] || null

                                x: Theme.dp(Theme.edge) + index * strip.pitch
                                y: Theme.dp(10)
                                width: rows.tileW
                                height: rows.tileH
                                entry: g
                                focused: block.active && index === Math.min(block.at, block.coll.items.length - 1)
                                fraction: page.fractionOf(g)
                                onPicked: page.pointTile(block.k, index)
                            }
                        }
                    }

                    Swipe {
                        flickable: strip
                        horizontal: true
                        ease: stripEase
                    }
                }
            }
        }

        Swipe {
            flickable: rows
            ease: rowsEase
        }
    }

    // ---- Downloads ----

    Item {
        id: downloads

        y: page.contentTop + jobLine.height
        width: parent.width
        height: parent.height - y
        opacity: page.tab === 1 && !page.cardOpen ? 1.0 : 0.0
        visible: opacity > 0.01
        transform: Translate {
            x: page.tab === 1 ? 0 : Theme.dp(60)

            Behavior on x {
                NumberAnimation {
                    duration: Theme.durTab
                    easing.type: Easing.OutCubic
                }
            }
        }

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durTab
                easing.type: Easing.OutCubic
            }
        }

        readonly property real headingH: Theme.dp(80)
        readonly property real lineH: Theme.dp(116)

        function lineY(i) {
            var y = 0;
            for (var k = 0; k < i; k++)
                y += page.lines[k].heading ? headingH : lineH;
            return y;
        }

        function reveal() {
            var i = page.lineIndex;
            if (i < 0 || i >= page.lines.length)
                return;
            var top = lineY(i), bottom = top + lineH + Theme.dp(8);
            if (i > 0 && page.lines[i - 1].heading)
                top -= headingH;
            Theme.reveal(list, top, bottom, list.height);
        }

        // The install folder: what the installs take, what is left.
        Rectangle {
            id: disk

            readonly property real total: page.onDisk + page.sources.freeSpace

            x: Theme.dp(Theme.edge)
            width: parent.width - x - Theme.dp(Theme.columnRight)
            height: Theme.dp(76)
            radius: Theme.dp(Theme.radiusCard)
            color: Theme.glass
            border.width: 1
            border.color: Theme.glassEdge
            visible: page.gamesDir !== ""

            Glyph {
                id: diskGlyph
                x: Theme.dp(24)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(32)
                height: width
                kind: "storage"
            }

            Label {
                id: diskPath
                anchors.left: diskGlyph.right
                anchors.leftMargin: Theme.dp(18)
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(implicitWidth, parent.width * 0.35)
                text: page.gamesDir
                elide: Text.ElideMiddle
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Rectangle {
                anchors.left: diskPath.right
                anchors.leftMargin: Theme.dp(24)
                anchors.right: diskFree.left
                anchors.rightMargin: Theme.dp(24)
                anchors.verticalCenter: parent.verticalCenter
                height: Theme.dp(8)
                radius: height / 2
                color: Qt.rgba(1, 1, 1, 0.14)

                Rectangle {
                    width: disk.total > 0 ? parent.width * Math.min(1, page.onDisk / disk.total) : 0
                    height: parent.height
                    radius: height / 2
                    color: Theme.accent
                }
            }

            Label {
                id: diskFree
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(24)
                anchors.verticalCenter: parent.verticalCenter
                text: (page.onDisk > 0 ? Format.bytes(page.onDisk) + " used" : "") + (page.sources.freeSpace > 0 ? (page.onDisk > 0 ? " · " : "") + Format.bytes(page.sources.freeSpace) + " free" : "")
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }

        Label {
            anchors.centerIn: parent
            visible: page.lines.length <= 1
            text: page.sources.busy ? "Loading…" : "Nothing installed yet"
            color: Theme.textMuted
            font.weight: Font.Light
            font.pixelSize: Theme.dp(40)
        }

        Flickable {
            id: list

            x: Theme.dp(Theme.edge) - Theme.dp(16)
            y: disk.visible ? disk.height + Theme.dp(16) : 0
            width: parent.width - x - Theme.dp(Theme.columnRight) + Theme.dp(16)
            height: parent.height - y - Theme.dp(90)
            contentWidth: width
            contentHeight: downloads.lineY(page.lines.length) + Theme.dp(40)
            interactive: false
            clip: true

            Behavior on contentY {
                id: listEase
                NumberAnimation {
                    duration: Theme.durScroll
                    easing.type: Easing.OutCubic
                }
            }

            Repeater {
                model: page.lines

                Item {
                    id: lineItem

                    readonly property var l: modelData
                    readonly property bool heading: l.heading === true
                    readonly property bool focused: page.zone === "main" && page.tab === 1 && page.lineIndex === index && page.activeFocus

                    y: downloads.lineY(index)
                    width: list.width
                    height: heading ? downloads.headingH : downloads.lineH

                    Item {
                        anchors.fill: parent
                        visible: lineItem.heading

                        Label {
                            x: Theme.dp(16)
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Theme.dp(14)
                            text: lineItem.heading ? lineItem.l.label : ""
                            font.pixelSize: Theme.dp(26)
                        }

                        Label {
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(16)
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Theme.dp(14)
                            text: lineItem.heading ? lineItem.l.meta || "" : ""
                            color: Theme.textMuted
                            font.pixelSize: Theme.dp(Theme.fontTiny)
                        }
                    }

                    StoreLine {
                        anchors.fill: parent
                        visible: !lineItem.heading
                        entry: lineItem.l.row !== undefined ? lineItem.l.game : null
                        title: lineItem.l.all ? "Update everything" : lineItem.l.update !== undefined ? lineItem.l.item.title : lineItem.l.game ? lineItem.l.game.title : ""
                        meta: {
                            var l = lineItem.l;
                            if (l.all)
                                return Format.plural(page.sources.updates.length, "update", "updates") + " from " + page.sourceName;
                            if (l.update !== undefined)
                                return [l.item.version ? "Version " + l.item.version : "", l.item.date || ""].filter(Boolean).join(" · ");
                            return l.game ? (page.isLive(l.game) ? page.statusOf(l.game) : l.game.partial ? l.game.status : l.game.pending ? "Update available" : l.game.busy ? l.game.status : "Installed") : "";
                        }
                        loud: lineItem.l.game ? lineItem.l.game.busy || lineItem.l.game.pending : lineItem.l.update !== undefined || lineItem.l.all === true
                        size: lineItem.l.game ? lineItem.l.game.sizeText : ""
                        action: {
                            var l = lineItem.l;
                            if (l.all || l.update !== undefined)
                                return "Update";
                            var g = l.game;
                            return !g ? "" : g.busy ? "Cancel" : g.partial ? "Resume" : g.pending ? "Update" : "Options";
                        }
                        glyph: "download"
                        focused: lineItem.focused
                        live: lineItem.l.game ? page.isLive(lineItem.l.game) : false
                        fraction: lineItem.l.game ? page.fractionOf(lineItem.l.game) : 0
                        onPicked: page.pointLine(index)
                    }
                }
            }
        }

        Swipe {
            flickable: list
            ease: listEase
        }

        Scrollbar {
            anchors.left: list.right
            anchors.leftMargin: Theme.dp(20)
            anchors.top: list.top
            anchors.bottom: list.bottom
            flickable: list
        }
    }

    // ---- A game's card: the console's product page, its buttons on A ----

    Item {
        id: card

        anchors.fill: parent
        opacity: page.cardOpen ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }

        Block {}

        StoreBackdrop {
            anchors.fill: parent
        }

        Item {
            id: cardArt

            x: parent.width * 0.4
            width: parent.width - x
            height: parent.height * 0.78
            scale: page.cardOpen ? 1.0 : 1.03
            transformOrigin: Item.TopRight

            Behavior on scale {
                NumberAnimation {
                    duration: Theme.durPage
                    easing.type: Easing.OutCubic
                }
            }

            StoreArt {
                anchors.fill: parent
                entry: page.cardEntry
                radius: 0
                titleSize: Theme.dp(60)
                opacity: 0.9
            }

            Rectangle {
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.width * 0.55
                gradient: Gradient {
                    orientation: Gradient.Horizontal
                    GradientStop {
                        position: 0.0
                        color: "#101115"
                    }
                    GradientStop {
                        position: 1.0
                        color: Qt.rgba(0.063, 0.067, 0.082, 0)
                    }
                }
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: parent.height * 0.5
                gradient: Gradient {
                    GradientStop {
                        position: 0.0
                        color: Qt.rgba(0.063, 0.067, 0.082, 0)
                    }
                    GradientStop {
                        position: 1.0
                        color: "#0e0f12"
                    }
                }
            }
        }

        PageTitle {
            anchors.left: parent.left
            anchors.right: parent.right
            icon: "store"
            title: "Store"
            trailing: page.sourceName
        }

        Column {
            x: Theme.dp(Theme.edge)
            y: Theme.dp(300)
            width: Math.min(Theme.dp(1000), parent.width * 0.55)
            spacing: Theme.dp(14)

            readonly property var libraryGame: page.cardEntry && page.cardEntry.game_id ? api.allGames.byId(page.cardEntry.game_id) : null

            Label {
                width: parent.width
                text: page.cardEntry ? page.cardEntry.title : ""
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
                lineHeight: 0.95
                font.weight: Font.Light
                font.pixelSize: Theme.dp(64)
            }

            Label {
                width: parent.width
                text: page.statusOf(page.cardEntry)
                color: page.cardEntry && (page.cardEntry.pending || page.cardEntry.busy) ? "#9cc8ff" : Theme.text
                elide: Text.ElideRight
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(28)
            }

            Label {
                width: parent.width
                text: [page.sizesOf(page.cardEntry), page.cardEntry && !page.cardEntry.installed && page.gamesDir ? (page.sources.freeSpace > 0 ? Format.bytes(page.sources.freeSpace) + " free in " : "Into ") + page.gamesDir : ""].filter(Boolean).join("\n")
                color: Theme.textSecondary
                wrapMode: Text.WordWrap
                lineHeight: 1.2
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Label {
                width: parent.width
                visible: text !== ""
                text: parent.libraryGame ? (parent.libraryGame.summary || parent.libraryGame.description || "") : ""
                color: Theme.textSecondary
                wrapMode: Text.WordWrap
                maximumLineCount: 4
                elide: Text.ElideRight
                lineHeight: 1.25
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Item {
                width: 1
                height: Theme.dp(18)
            }

            Flow {
                width: card.width - Theme.dp(Theme.edge + Theme.columnRight)
                spacing: Theme.dp(22)

                Repeater {
                    model: page.cardActions

                    StoreButton {
                        text: modelData.round ? "" : modelData.label
                        glyph: modelData.glyph
                        round: modelData.round === true
                        danger: modelData.act === "cancel"
                        focused: page.cardOpen && page.activeFocus && index === Math.min(page.cardIndex, page.cardActions.length - 1)
                        onPicked: page.cardIndex = index
                    }
                }
            }

            Label {
                visible: page.cardActions.length === 0
                text: "Nothing to do with this game from here."
                color: Theme.textMuted
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }

    // The console's own hint box at the store's foot: Options and the tab switch.
    Rectangle {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(36)
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(28)
        width: hintRow.implicitWidth + Theme.dp(32)
        height: Theme.dp(50)
        radius: Theme.dp(4)
        color: Qt.rgba(0.08, 0.085, 0.1, 0.92)

        Row {
            id: hintRow
            anchors.centerIn: parent
            spacing: Theme.dp(10)

            HintGlyph {
                anchors.verticalCenter: parent.verticalCenter
                glyph: "Start"
                unit: Theme.dp(30)
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: "Options"
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Item {
                width: Theme.dp(18)
                height: 1
            }

            HintGlyph {
                anchors.verticalCenter: parent.verticalCenter
                glyph: "LB"
                unit: Theme.dp(30)
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: "/"
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            HintGlyph {
                anchors.verticalCenter: parent.verticalCenter
                glyph: "RB"
                unit: Theme.dp(30)
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: "Switch Tabs"
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }
}
