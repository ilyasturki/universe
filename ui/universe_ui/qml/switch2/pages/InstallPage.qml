import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../core/Format.js" as Format

// Store: what you own and could install, as tiles with a size to download. Manage: the disk, what is
// installing or paused, what is installed — rows with a size and an action.
FocusScope {
    id: page

    property var shell: null
    signal closeRequested()
    focus: true

    readonly property var sources: api.screens.sources
    property int tab: 0
    property string zone: "main"
    property int index: 0
    property int rowIndex: 0

    readonly property string sourceName: sources.current ? sources.current.name : (sources.source || "Install")
    readonly property bool loggedIn: sources.current ? sources.current.logged_in === true : false
    readonly property string gamesDir: sources.current && sources.current.games_dir ? sources.current.games_dir : ""
    readonly property bool running: sources.job !== null && sources.job !== undefined && (sources.job.ok === null || sources.job.ok === undefined)

    // Store: not installed (a stopped download among them), or the search's results.
    readonly property var cells: {
        var rows = sources.rows;
        function group(label, idx) {
            return [{ heading: true, label: label, count: idx.length }].concat(idx.map(function(i) { return { row: i, game: rows[i] }; }));
        }
        var all = rows.map(function(r, i) { return i; });
        if (sources.query)
            return group("Results for “" + sources.query + "”", all);
        var owned = all.filter(function(i) { return !rows[i].installed; });
        return owned.length > 0 ? group("Owned, not installed", owned) : [];
    }
    readonly property var current: index >= 0 && index < cells.length && !cells[index].heading ? cells[index] : null

    // Manage: the jobs first, then the installs.
    readonly property var lines: {
        var rows = sources.rows, out = [], jobs = [], installed = [], running = 0, paused = 0;
        for (var i = 0; i < rows.length; i++) {
            if (rows[i].busy || rows[i].partial) {
                jobs.push(i);
                rows[i].busy ? running++ : paused++;
            } else if (rows[i].installed)
                installed.push(i);
        }
        function group(label, meta, idx) {
            return [{ heading: true, label: label, meta: meta }].concat(idx.map(function(i) { return { row: i, game: rows[i] }; }));
        }
        if (jobs.length > 0)
            out = group("Installing", [running > 0 ? running + " running" : "", paused > 0 ? paused + " paused" : ""].filter(Boolean).join(" · "), jobs);
        return out.concat(group("Installed", Format.plural(installed.length, "game", "games") + (page.onDisk > 0 ? " · " + Format.bytes(page.onDisk) : ""), installed));
    }
    readonly property var currentLine: rowIndex >= 0 && rowIndex < lines.length && !lines[rowIndex].heading ? lines[rowIndex] : null
    readonly property real onDisk: {
        var listed = sources.rows;
        return listed.reduce(function(sum, r) { return sum + (r.installed ? r.disk_size : 0); }, 0);
    }
    readonly property string libraryLine: sources.error !== "" ? sourceName + " unreachable · listing from " + (sources.libraryAge || "before")
        : sources.busy ? "Loading…" : (loggedIn ? "Signed in" : "Not signed in") + (sources.libraryAge ? " · refreshed " + sources.libraryAge : "")

    readonly property var hints: {
        var out = [ { glyph: "Y", label: "Refresh" } ];
        if (running)
            out.push({ glyph: "X", label: "Cancel " + (sources.job.label.indexOf("Updating") === 0 ? "update" : "install") });
        out.push({ glyph: "B", label: "Back" });
        if (zone === "rail")
            out.push({ glyph: "A", label: "OK" });
        else if (tab === 0 && current)
            out.push({ glyph: "A", label: current.game.action });
        else if (tab === 1 && currentLine)
            out.push({ glyph: "A", label: "Options" });
        return out;
    }

    readonly property int columns: 5
    readonly property real gridX: Theme.dp(300)
    readonly property real gridW: parent ? parent.width - gridX - Theme.dp(Theme.edgeMargin) - Theme.dp(40) : 0
    readonly property real gap: Theme.dp(24)
    readonly property real cellW: (gridW - gap * (columns - 1)) / columns
    readonly property real cellH: cellW + Theme.dp(Theme.ringRoom + 76)
    readonly property real headingH: Theme.dp(80)
    readonly property real lineH: Theme.dp(123)
    readonly property real room: Theme.dp(Theme.ringRoom)
    readonly property real contentY: header.height + tabs.height + Theme.dp(10)

    readonly property var layout: {
        var out = [], y = 0, col = 0;
        for (var i = 0; i < cells.length; i++) {
            if (cells[i].heading) {
                if (col > 0) {
                    y += cellH + gap;
                    col = 0;
                }
                out.push({ x: 0, y: y, w: gridW, h: headingH });
                y += headingH;
                continue;
            }
            out.push({ x: col * (cellW + gap), y: y, w: cellW, h: cellH });
            col++;
            if (col === columns) {
                col = 0;
                y += cellH + gap;
            }
        }
        return { cells: out, height: col > 0 ? y + cellH : y };
    }

    function lineY(i) {
        var y = 0;
        for (var k = 0; k < i; k++)
            y += lines[k].heading ? headingH : lineH;
        return y;
    }
    readonly property real linesHeight: lineY(lines.length)

    Component.onCompleted: sources.load()

    function firstGame() {
        return cells.map(function(c) { return !c.heading; }).indexOf(true);
    }

    function firstLine() {
        return lines.map(function(c) { return !c.heading; }).indexOf(true);
    }

    onCellsChanged: {
        if (!current)
            index = Math.max(0, firstGame());
    }
    onLinesChanged: {
        if (!currentLine)
            rowIndex = Math.max(0, firstLine());
    }
    onIndexChanged: {
        view.scrollToCurrent();
        if (current)
            sources.peek(current.row);
    }
    onRowIndexChanged: list.scrollToCurrent()

    function focusMain() {
        zone = "main";
        (tab === 0 ? grid : manage).forceActiveFocus();
    }

    function move(dx, dy) {
        if (!current) {
            index = Math.max(0, firstGame());
            return;
        }
        var here = layout.cells[index];
        var best = -1, bestD = 1e9;
        if (dx !== 0) {
            var n = index + dx;
            best = n >= 0 && n < cells.length && !cells[n].heading ? n : -1;
        } else {
            for (var i = 0; i < cells.length; i++) {
                var c = layout.cells[i];
                if (cells[i].heading || i === index || dy > 0 && c.y <= here.y || dy < 0 && c.y >= here.y)
                    continue;
                var d = Math.abs(c.y - here.y) * 10 + Math.abs(c.x - here.x) / cellW;
                if (d < bestD) {
                    bestD = d;
                    best = i;
                }
            }
        }
        if (best < 0) {
            if (dx < 0)
                toRail();
            else
                Sound.play("edge");
            return;
        }
        Sound.play("tick");
        index = best;
    }

    function stepLine(d) {
        var stops = lines.map(function(l, i) { return i; }).filter(function(i) { return !lines[i].heading; });
        var pos = stops.indexOf(rowIndex) + d;
        if (pos < 0 || pos >= stops.length) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        rowIndex = stops[pos];
    }

    function toRail() {
        zone = "rail";
        rail.forceActiveFocus();
        Sound.play("tick");
    }

    function askInstall(g, row) {
        var parts = [];
        if (g.download_size > 0)
            parts.push(Format.bytes(g.download_size) + " to download");
        if (g.disk_size > 0)
            parts.push(Format.bytes(g.disk_size) + " on disk");
        var free = sources.freeSpace > 0 ? Format.bytes(sources.freeSpace) + " free" + (gamesDir ? " in " + gamesDir : "") : gamesDir ? "Into " + gamesDir : "";
        shell.dialogAsk({ message: "Install " + g.title + "?", detail: [parts.length > 0 ? parts.join(" · ") : "Size not known yet", free].filter(Boolean).join("\n"), buttons: ["Not now", "Install"] },
                        function(k) { if (k === 1) Sound.play(sources.install(row) !== "" ? "ok" : "edge"); });
    }

    function cancel() {
        if (!running) {
            Sound.play("edge");
            return;
        }
        sources.cancel() ? Sound.play("back") : Sound.play("edge");
    }

    function activate(g, row) {
        if (running && g.busy) {
            page.cancel();
            return;
        }
        if (!g.installed && !g.partial) {
            askInstall(g, row);
            return;
        }
        var items = [];
        if (g.partial)
            items.push({ label: "Resume", act: "resume" });
        else if (g.pending)
            items.push({ label: "Update", act: "update" });
        if (g.installed && g.game_id && api.allGames.byId(g.game_id))
            items.push({ label: "Game Settings", act: "settings" });
        if (g.installed && g.game_id)
            items.push({ label: "Uninstall…", act: "uninstall" }, { label: "Remove from library…", act: "remove" });
        if (items.length === 0) {
            Sound.play("edge");
            return;
        }
        var title = g.title, gameId = g.game_id;
        shell.menu(title, items, function(a) {
            if (a === "resume" || a === "update") {
                Sound.play(sources.install(row) !== "" ? "ok" : "edge");
            } else if (a === "settings") {
                Sound.play("ok");
                shell.push("pages/GameSettingsPage.qml", { gameId: gameId });
            } else if (a === "uninstall") {
                shell.dialogAsk({ message: "Uninstall " + title + "?", detail: "The install folder goes to the trash; the hours and the journal stay.",
                                  buttons: ["Cancel", "Uninstall"], danger: 1 }, function(k) { if (k === 1) sources.uninstall(gameId); });
            } else if (a === "remove") {
                shell.dialogAsk({ message: "Remove " + title + " from the library?", detail: "The entry is archived; the files are left where they are.",
                                  buttons: ["Cancel", "Remove"], danger: 1 }, function(k) { if (k === 1) sources.remove(gameId); });
            }
        });
    }

    function railAction(id) {
        if (id === "search") {
            shell.prompt({ title: "Search " + sourceName, value: sources.query }, function(q) {
                if (q === null)
                    return;
                sources.search(q);
                if (tab !== 0)
                    tabs.step(-1);
                focusMain();
            });
        } else if (id === "signin") {
            shell.push("pages/SettingsPage.qml", { section: "signin" });
        } else if (id === "clear") {
            sources.search("");
        }
    }

    Connections {
        target: page.sources
        function onMessage(text) { page.shell.showToast(text); }
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isFilters(event)) {
            event.accepted = true;
            Sound.play("ok");
            sources.refresh();
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            page.cancel();
        } else if (api.keys.isPrevPage(event)) {
            event.accepted = true;
            tabs.step(-1);
        } else if (api.keys.isNextPage(event)) {
            event.accepted = true;
            tabs.step(1);
        } else if (api.keys.isCancel(event) && sources.query !== "") {
            event.accepted = true;
            Sound.play("back");
            sources.search("");
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "shop"
        iconColor: Theme.barOrange
        title: page.sourceName
        trailing: page.libraryLine
    }

    Tabs {
        id: tabs
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        names: ["Store", "Manage"]
        onChanged: function(i) {
            page.tab = i;
            page.focusMain();
        }
    }

    Rail {
        id: rail

        x: Theme.dp(120)
        y: page.contentY + Theme.dp(20)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "rail"
        items: {
            var out = [ { id: "search", icon: "search", label: "Search" } ];
            if (page.sources.query !== "")
                out.push({ id: "clear", icon: "filter", label: "Clear the search" });
            out.push({ id: "signin", icon: "key", label: page.loggedIn ? "Signed in" : "Sign in" });
            return out;
        }

        onActivated: function(id) { page.railAction(id); }
        onEscapedRight: page.focusMain()
    }

    JobLine {
        id: jobLine
        x: page.gridX
        y: page.contentY
        width: page.gridW
        job: page.sources.job
    }

    // Store
    FocusScope {
        id: grid

        x: page.gridX
        y: page.contentY + jobLine.height
        width: page.gridW
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(10)
        focus: page.zone === "main" && page.tab === 0
        visible: page.tab === 0

        Keys.onLeftPressed: page.move(-1, 0)
        Keys.onRightPressed: page.move(1, 0)
        Keys.onUpPressed: page.move(0, -1)
        Keys.onDownPressed: page.move(0, 1)
        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isAccept(event)) {
                event.accepted = true;
                page.current ? page.activate(page.current.game, page.current.row) : Sound.play("edge");
            }
        }

        Label {
            anchors.centerIn: parent
            visible: page.cells.length === 0
            text: page.sources.busy ? "Loading…" : !page.loggedIn ? "Sign in to see your games." : page.sources.rows.length > 0 ? "Everything you own is installed." : "Nothing here yet."
            color: Theme.textMuted
        }

        Flickable {
            id: view

            anchors.fill: parent
            anchors.margins: -page.room
            contentWidth: width
            contentHeight: page.layout.height + page.room * 2 + Theme.dp(40)
            interactive: false
            clip: true

            function scrollToCurrent() {
                if (height <= 0 || page.index < 0 || page.index >= page.layout.cells.length)
                    return;
                var c = page.layout.cells[page.index];
                var top = c.y, bottom = c.y + c.h + page.room * 2;
                if (page.index > 0 && page.cells[page.index - 1].heading)
                    top -= page.headingH;
                Theme.reveal(view, top, bottom, height);
            }

            Behavior on contentY { Ease {} }

            Repeater {
                model: page.cells

                Loader {
                    id: cell

                    readonly property var row: modelData
                    readonly property var spot: page.layout.cells[index] || ({ x: 0, y: 0, w: 0, h: 0 })
                    readonly property bool heading: row.heading === true
                    readonly property var entry: heading ? null : row.game
                    readonly property var libraryGame: entry && entry.game_id ? api.allGames.byId(entry.game_id) : null
                    readonly property bool focused: grid.activeFocus && index === page.index

                    x: page.room + spot.x
                    y: page.room + spot.y
                    width: spot.w
                    height: spot.h
                    z: focused ? 2 : 1
                    active: spot.y + spot.h > view.contentY - page.cellH && spot.y < view.contentY + view.height + page.cellH
                    sourceComponent: heading ? headingCell : gameCell
                }
            }

            Component {
                id: headingCell

                Item {
                    readonly property Item cell: parent

                    Label {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: Theme.dp(4)
                        text: cell.row.label
                    }

                    Label {
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: Theme.dp(4)
                        text: Format.plural(cell.row.count, "game", "games")
                        color: Theme.textSecondary
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }

                    Hairline {
                        anchors.bottomMargin: Theme.dp(12)
                        color: Theme.hairline
                    }
                }
            }

            Component {
                id: gameCell

                Item {
                    readonly property Item cell: parent
                    readonly property var g: cell.entry
                    readonly property bool inProgress: g !== null && (g.busy || g.partial)
                    readonly property real fraction: !g ? 0 : g.busy && page.running && page.sources.job.game === g.id && page.sources.job.total > 0
                                                          ? page.sources.job.done / page.sources.job.total
                                                          : g.disk_size > 0 ? g.partial_bytes / g.disk_size : 0

                    Tile {
                        id: art
                        width: page.cellW
                        height: page.cellW
                        game: cell.libraryGame
                        focused: cell.focused
                        cornerRadius: Theme.dp(6)
                    }

                    Image {
                        id: storeImage
                        anchors.fill: art
                        z: 3
                        visible: cell.libraryGame === null && status === Image.Ready
                        source: cell.libraryGame === null && g ? g.image : ""
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        sourceSize.width: 400
                    }

                    Label {
                        anchors.centerIn: art
                        z: 3
                        width: art.width - Theme.dp(30)
                        visible: cell.libraryGame === null && !(g && g.image && storeImage.status === Image.Ready)
                        text: g ? g.title : ""
                        color: Theme.textSecondary
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        maximumLineCount: 3
                        elide: Text.ElideRight
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }

                    // A download under way or paused: its state painted over the tile's foot.
                    Rectangle {
                        anchors.left: art.left
                        anchors.right: art.right
                        anchors.bottom: art.bottom
                        z: 4
                        height: Theme.dp(64)
                        visible: inProgress
                        color: Qt.rgba(0.176, 0.176, 0.176, 0.72)

                        Label {
                            x: Theme.dp(14)
                            y: Theme.dp(8)
                            width: parent.width - Theme.dp(28)
                            text: !g ? "" : g.busy ? g.status : "Paused"
                            color: "#ffffff"
                            elide: Text.ElideRight
                            font.pixelSize: Theme.dp(20)
                        }

                        Rectangle {
                            x: Theme.dp(14)
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Theme.dp(12)
                            width: parent.width - Theme.dp(28)
                            height: Theme.dp(6)
                            radius: height / 2
                            color: Qt.rgba(1, 1, 1, 0.35)

                            Rectangle {
                                width: parent.width * Math.min(1, fraction)
                                height: parent.height
                                radius: height / 2
                                color: "#ffffff"

                                Behavior on width { Ease { duration: Theme.durQuick } }
                            }
                        }
                    }

                    Label {
                        anchors.top: art.bottom
                        anchors.topMargin: Theme.dp(Theme.ringRoom + 6)
                        width: art.width
                        text: g ? g.title : ""
                        color: cell.focused ? Theme.accent : Theme.text
                        elide: Text.ElideRight
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }

                    Label {
                        anchors.top: art.bottom
                        anchors.topMargin: Theme.dp(Theme.ringRoom + 36)
                        width: art.width
                        text: !g ? "" : inProgress ? g.status : g.sizeText ? g.sizeText + " to download" : g.status
                        color: g && (g.pending || g.busy) ? Theme.accent : Theme.textSecondary
                        elide: Text.ElideRight
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                    }
                }
            }
        }

        Scrollbar {
            anchors.right: parent.right
            anchors.rightMargin: -Theme.dp(40)
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            flickable: view
        }
    }

    // Manage
    FocusScope {
        id: manage

        x: page.gridX
        y: page.contentY + jobLine.height
        width: page.gridW
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(10)
        focus: page.zone === "main" && page.tab === 1
        visible: page.tab === 1

        Keys.onUpPressed: page.stepLine(-1)
        Keys.onDownPressed: page.stepLine(1)
        Keys.onLeftPressed: page.toRail()
        Keys.onRightPressed: Sound.play("edge")
        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isAccept(event)) {
                event.accepted = true;
                page.currentLine ? page.activate(page.currentLine.game, page.currentLine.row) : Sound.play("edge");
            }
        }

        // The install folder: what the installs take, what is left.
        Rectangle {
            id: disk
            width: parent.width
            height: Theme.dp(76)
            radius: Theme.dp(Theme.radiusRow)
            color: Theme.card
            border.width: 1
            border.color: Theme.hairlineSoft
            visible: page.gamesDir !== ""

            readonly property real total: page.onDisk + page.sources.freeSpace

            Glyph {
                id: diskGlyph
                x: Theme.dp(24)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(32)
                height: width
                kind: "folder"
                tint: Theme.artInk
            }

            Label {
                id: diskPath
                anchors.left: diskGlyph.right
                anchors.leftMargin: Theme.dp(20)
                anchors.verticalCenter: parent.verticalCenter
                text: page.gamesDir
                elide: Text.ElideMiddle
                width: Math.min(implicitWidth, parent.width * 0.4)
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Rectangle {
                anchors.left: diskPath.right
                anchors.leftMargin: Theme.dp(24)
                anchors.right: diskFree.left
                anchors.rightMargin: Theme.dp(24)
                anchors.verticalCenter: parent.verticalCenter
                height: Theme.dp(10)
                radius: height / 2
                color: Theme.hairlineSoft

                Rectangle {
                    width: disk.total > 0 ? parent.width * Math.min(1, page.onDisk / disk.total) : 0
                    height: parent.height
                    radius: height / 2
                    color: Theme.accentStrong
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
            text: page.sources.busy ? "Loading…" : "Nothing installed yet."
            color: Theme.textMuted
        }

        Flickable {
            id: list

            anchors.fill: parent
            anchors.topMargin: disk.visible ? disk.height + Theme.dp(10) : 0
            anchors.leftMargin: -page.room
            anchors.rightMargin: -page.room
            anchors.bottomMargin: -page.room
            contentWidth: width
            contentHeight: page.linesHeight + page.room * 2 + Theme.dp(40)
            interactive: false
            clip: true

            function scrollToCurrent() {
                if (height <= 0 || page.rowIndex < 0 || page.rowIndex >= page.lines.length)
                    return;
                var top = page.lineY(page.rowIndex), bottom = top + page.lineH + page.room * 2;
                if (page.rowIndex > 0 && page.lines[page.rowIndex - 1].heading)
                    top -= page.headingH;
                Theme.reveal(list, top, bottom, height);
            }

            Behavior on contentY { Ease {} }

            Repeater {
                model: page.lines

                Item {
                    id: line

                    readonly property var entry: modelData
                    readonly property bool heading: entry.heading === true
                    readonly property var g: heading ? null : entry.game
                    readonly property var libraryGame: g && g.game_id ? api.allGames.byId(g.game_id) : null
                    readonly property bool focused: manage.activeFocus && index === page.rowIndex
                    readonly property bool live: g !== null && g.busy && page.running && page.sources.job.game === g.id
                    readonly property real fraction: !g ? 0 : live ? (page.sources.job.total > 0 ? page.sources.job.done / page.sources.job.total : 0)
                                                     : g.partial && g.disk_size > 0 ? g.partial_bytes / g.disk_size : 0
                    readonly property string meta: !g ? "" : live ? page.sources.job.message.replace(page.sources.job.label + " · ", "")
                                                   : g.partial ? g.status : g.pending ? "Update available" : g.busy ? g.status : "Installed"

                    x: page.room
                    y: page.room + page.lineY(index)
                    width: list.width - page.room * 2
                    height: heading ? page.headingH : page.lineH

                    Item {
                        visible: line.heading
                        anchors.fill: parent

                        Label {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.verticalCenterOffset: Theme.dp(4)
                            text: line.heading ? line.entry.label : ""
                        }

                        Label {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.verticalCenterOffset: Theme.dp(4)
                            text: line.heading ? line.entry.meta : ""
                            color: Theme.textSecondary
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }

                        Hairline {
                            anchors.bottomMargin: Theme.dp(12)
                            color: Theme.hairline
                        }
                    }

                    Item {
                        visible: !line.heading
                        anchors.fill: parent

                        FocusPill {
                            anchors.fill: parent
                            focused: line.focused
                        }

                        Hairline {
                            visible: !line.focused
                            color: Theme.hairlineSoft
                        }

                        Tile {
                            id: thumb
                            x: Theme.dp(24)
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(92)
                            height: width
                            game: line.libraryGame
                            focused: false
                            outlineShown: false
                            cornerRadius: Theme.dp(6)

                            Image {
                                anchors.fill: parent
                                z: 3
                                visible: line.libraryGame === null && status === Image.Ready
                                source: line.libraryGame === null && line.g ? line.g.image : ""
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                sourceSize.width: 200
                            }
                        }

                        Label {
                            id: title
                            anchors.left: thumb.right
                            anchors.leftMargin: Theme.dp(28)
                            anchors.right: size.left
                            anchors.rightMargin: Theme.dp(28)
                            y: Theme.dp(22)
                            text: line.g ? line.g.title : ""
                            color: line.focused ? Theme.accent : Theme.text
                            elide: Text.ElideRight
                        }

                        Label {
                            anchors.left: title.left
                            anchors.right: title.right
                            anchors.top: title.bottom
                            anchors.topMargin: Theme.dp(2)
                            text: line.meta
                            color: line.g && (line.g.busy || line.g.pending) ? Theme.accent : Theme.textSecondary
                            elide: Text.ElideRight
                            font.pixelSize: Theme.dp(Theme.fontTiny)
                        }

                        Rectangle {
                            anchors.left: title.left
                            anchors.right: title.right
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Theme.dp(14)
                            height: Theme.dp(6)
                            radius: height / 2
                            visible: line.g !== null && (line.g.busy || line.g.partial)
                            color: Theme.hairlineSoft

                            Rectangle {
                                width: parent.width * Math.min(1, line.fraction)
                                height: parent.height
                                radius: height / 2
                                color: line.live ? Theme.accentStrong : Theme.textMuted

                                Behavior on width { Ease { duration: Theme.durQuick } }
                            }
                        }

                        Label {
                            id: size
                            anchors.right: action.left
                            anchors.rightMargin: Theme.dp(28)
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(180)
                            horizontalAlignment: Text.AlignRight
                            text: line.g ? line.g.sizeText : ""
                            font.features: { "tnum": 1 }
                        }

                        Label {
                            id: action
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(24)
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(240)
                            horizontalAlignment: Text.AlignRight
                            text: !line.g ? "" : line.g.busy ? "Cancel" : line.g.partial ? "Resume" : line.g.pending ? "Update" : "Options"
                            color: line.g && line.g.busy ? Theme.danger : line.focused ? Theme.accent : Theme.textSecondary
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }
                    }
                }
            }
        }

        Scrollbar {
            anchors.right: parent.right
            anchors.rightMargin: -Theme.dp(40)
            anchors.top: list.top
            anchors.bottom: parent.bottom
            flickable: list
        }
    }
}
