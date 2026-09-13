import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    property var shell: null
    signal closeRequested()
    focus: true

    readonly property var sources: api.screens.sources
    property string zone: "grid"
    property int index: 0

    readonly property string sourceName: sources.current ? sources.current.name : (sources.source || "Install")
    readonly property bool loggedIn: sources.current ? sources.current.logged_in === true : false

    readonly property var cells: {
        var rows = sources.rows;
        function group(label, idx) {
            return [{ heading: true, label: label, count: idx.length }].concat(idx.map(function(i) { return { row: i, game: rows[i] }; }));
        }
        var all = rows.map(function(r, i) { return i; });
        if (sources.query)
            return group("Results for “" + sources.query + "”", all);
        var installed = all.filter(function(i) { return rows[i].installed; }), owned = all.filter(function(i) { return !rows[i].installed; });
        return (installed.length > 0 ? group("Installed", installed) : []).concat(owned.length > 0 ? group("Owned, not installed", owned) : []);
    }
    readonly property var current: index >= 0 && index < cells.length && !cells[index].heading ? cells[index] : null

    readonly property var hints: {
        var out = [ { glyph: "Y", label: "Refresh" }, { glyph: "B", label: "Back" } ];
        if (zone === "rail")
            out.push({ glyph: "A", label: "OK" });
        else if (current)
            out.push({ glyph: "A", label: current.game.installed ? "Options" : "Install" });
        return out;
    }

    readonly property int columns: 5
    readonly property real gridX: Theme.dp(300)
    readonly property real gridW: parent ? parent.width - gridX - Theme.dp(Theme.edgeMargin) - Theme.dp(40) : 0
    readonly property real gap: Theme.dp(24)
    readonly property real cellW: (gridW - gap * (columns - 1)) / columns
    readonly property real cellH: cellW + Theme.dp(70)
    readonly property real headingH: Theme.dp(80)

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

    Component.onCompleted: sources.load()

    function firstGame() {
        return cells.map(function(c) { return !c.heading; }).indexOf(true);
    }

    onCellsChanged: {
        if (!current)
            index = Math.max(0, firstGame());
    }
    onIndexChanged: view.scrollToCurrent()

    function move(dx, dy) {
        if (!current) {
            var f = firstGame();
            if (f >= 0)
                index = f;
            return;
        }
        var here = layout.cells[index];
        var best = -1, bestD = 1e9;
        for (var i = 0; i < cells.length; i++) {
            if (cells[i].heading || i === index)
                continue;
            var c = layout.cells[i];
            if (dx !== 0) {
                if (i !== index + dx)
                    continue;
                best = i;
                break;
            }
            if (dy > 0 && c.y <= here.y || dy < 0 && c.y >= here.y)
                continue;
            var d = Math.abs(c.y - here.y) * 10 + Math.abs(c.x - here.x) / cellW;
            if (d < bestD) {
                bestD = d;
                best = i;
            }
        }
        if (best < 0) {
            if (dx < 0) {
                zone = "rail";
                rail.forceActiveFocus();
                Sound.tick();
            } else {
                Sound.edge();
            }
            return;
        }
        Sound.tick();
        index = best;
    }

    function activate() {
        if (!current) {
            Sound.edge();
            return;
        }
        var g = current.game, items = [];
        if (!g.installed) {
            items.push({ label: "Install", act: "install" });
        } else {
            if (g.pending)
                items.push({ label: "Update", act: "update" });
            if (g.game_id && api.allGames.byId(g.game_id))
                items.push({ label: "Game Settings", act: "settings" });
            if (g.game_id)
                items.push({ label: "Uninstall…", act: "uninstall" }, { label: "Remove from library…", act: "remove" });
        }
        if (items.length === 0) {
            Sound.edge();
            return;
        }
        var row = current.row, title = g.title, gameId = g.game_id;
        shell.pick({ title: title, choices: items.map(function(i) { return i.label; }), index: 0 }, function(i) {
            if (i < 0)
                return;
            var a = items[i].act;
            if (a === "install" || a === "update") {
                Sound.ok();
                sources.install(row);
            } else if (a === "settings") {
                Sound.ok();
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
                zone = "grid";
                grid.forceActiveFocus();
            });
        } else if (id === "refresh") {
            sources.refresh();
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
            Sound.ok();
            sources.refresh();
        } else if (api.keys.isCancel(event) && sources.query !== "") {
            event.accepted = true;
            Sound.back();
            sources.search("");
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "shop"
        iconColor: Theme.barOrange
        title: page.sourceName
        trailing: page.sources.busy ? "Loading…" : page.loggedIn ? "Signed in" : "Not signed in"
    }

    Rail {
        id: rail

        x: Theme.dp(120)
        y: header.height + Theme.dp(50)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "rail"
        items: {
            var out = [ { id: "search", icon: "search", label: "Search" }, { id: "refresh", icon: "refresh", label: "Refresh" } ];
            if (page.sources.query !== "")
                out.push({ id: "clear", icon: "filter", label: "Clear the search" });
            out.push({ id: "signin", icon: "key", label: page.loggedIn ? "Signed in" : "Sign in" });
            return out;
        }

        onActivated: function(id) { page.railAction(id); }
        onEscapedRight: {
            page.zone = "grid";
            grid.forceActiveFocus();
        }
    }

    JobLine {
        id: jobLine
        x: page.gridX
        y: header.height + Theme.dp(30)
        width: page.gridW
        job: page.sources.job
    }

    FocusScope {
        id: grid

        x: page.gridX
        y: header.height + Theme.dp(30) + jobLine.height
        width: page.gridW
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(10)
        focus: page.zone === "grid"

        Keys.onLeftPressed: page.move(-1, 0)
        Keys.onRightPressed: page.move(1, 0)
        Keys.onUpPressed: page.move(0, -1)
        Keys.onDownPressed: page.move(0, 1)
        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isAccept(event)) {
                event.accepted = true;
                page.activate();
            }
        }

        Text {
            anchors.centerIn: parent
            visible: page.cells.length === 0
            text: page.sources.busy ? "Loading…" : page.loggedIn ? "Nothing here yet." : "Sign in to see your games."
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(Theme.fontBody)
        }

        Flickable {
            id: view

            anchors.fill: parent
            contentWidth: width
            contentHeight: page.layout.height + Theme.dp(40)
            interactive: false
            clip: true

            function scrollToCurrent() {
                if (height <= 0 || page.index < 0 || page.index >= page.layout.cells.length)
                    return;
                var c = page.layout.cells[page.index];
                var top = c.y - Theme.dp(20), bottom = c.y + c.h + Theme.dp(20);
                if (page.index > 0 && page.cells[page.index - 1].heading)
                    top -= page.headingH;
                Theme.reveal(view, top, bottom, height);
            }

            Behavior on contentY {
                NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic }
            }

            Repeater {
                model: page.cells

                Item {
                    id: cell

                    readonly property var spot: page.layout.cells[index] || ({ x: 0, y: 0, w: 0, h: 0 })
                    readonly property bool heading: modelData.heading === true
                    readonly property var entry: heading ? null : modelData.game
                    readonly property var libraryGame: entry && entry.game_id ? api.allGames.byId(entry.game_id) : null
                    readonly property bool focused: grid.activeFocus && index === page.index

                    x: spot.x
                    y: spot.y
                    width: spot.w
                    height: spot.h

                    Item {
                        visible: cell.heading
                        anchors.fill: parent

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.verticalCenterOffset: Theme.dp(4)
                            text: cell.heading ? modelData.label : ""
                            color: Theme.text
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(Theme.fontBody)
                        }

                        Text {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.verticalCenterOffset: Theme.dp(4)
                            text: cell.heading ? modelData.count + (modelData.count === 1 ? " game" : " games") : ""
                            color: Theme.textSecondary
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }

                        Rectangle {
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Theme.dp(12)
                            anchors.left: parent.left
                            anchors.right: parent.right
                            height: 1
                            color: Theme.hairline
                        }
                    }

                    Item {
                        visible: !cell.heading
                        anchors.fill: parent

                        Tile {
                            id: art
                            width: page.cellW
                            height: page.cellW
                            game: cell.libraryGame
                            focused: cell.focused
                            lift: false
                            cornerRadius: Theme.dp(6)
                        }

                        Image {
                            id: storeImage
                            anchors.fill: art
                            z: 3
                            visible: cell.libraryGame === null && status === Image.Ready
                            source: cell.entry ? cell.entry.image : ""
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            sourceSize.width: 400
                        }

                        Text {
                            anchors.centerIn: art
                            z: 3
                            width: art.width - Theme.dp(30)
                            visible: cell.libraryGame === null && !(cell.entry && cell.entry.image && storeImage.status === Image.Ready)
                            text: cell.entry ? cell.entry.title : ""
                            color: Theme.textSecondary
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            maximumLineCount: 3
                            elide: Text.ElideRight
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }

                        Text {
                            anchors.top: art.bottom
                            anchors.topMargin: Theme.dp(14)
                            width: art.width
                            text: cell.entry ? cell.entry.title : ""
                            color: cell.focused ? Theme.accent : Theme.text
                            elide: Text.ElideRight
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }

                        Text {
                            anchors.top: art.bottom
                            anchors.topMargin: Theme.dp(44)
                            width: art.width
                            text: cell.entry ? cell.entry.status : ""
                            color: cell.entry && cell.entry.pending ? Theme.accent : Theme.textSecondary
                            elide: Text.ElideRight
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(Theme.fontTiny)
                        }
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
}
