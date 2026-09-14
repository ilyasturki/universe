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
    property int index: 0

    readonly property var shown: {
        var out = rows.filter(function(r) { return filterId === "" || r.gameId === filterId; });
        return oldestFirst ? out.reverse() : out;
    }
    readonly property var current: index >= 0 && index < shown.length ? shown[index] : null
    readonly property string filterName: {
        if (filterId === "")
            return "All";
        var g = api.allGames.byId(filterId);
        return g ? g.title : filterId;
    }

    readonly property var games: Feed.channels(rows)

    readonly property var hints: zone === "rail"
        ? [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]
        : [ { glyph: "Start", label: "Options", dim: shown.length === 0 }, { glyph: "B", label: "Back" }, { glyph: "A", label: shown.length > 0 ? "Play" : "OK", dim: shown.length === 0 } ]

    signal closeRequested()

    readonly property int columns: 5
    readonly property real thumbWidth: Theme.dp(290)
    readonly property real thumbHeight: Math.round(thumbWidth * 9 / 16)
    readonly property real gap: Theme.dp(12)
    readonly property real gridX: Theme.dp(253)
    readonly property real gridY: Theme.dp(192)

    focus: true

    Component.onCompleted: store.loadAll()
    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (args.gameId)
            filterId = args.gameId;
    }

    onShownChanged: {
        if (index >= shown.length)
            index = Math.max(0, shown.length - 1);
        grid.scrollToCurrent();
    }
    onIndexChanged: grid.scrollToCurrent()
    onCurrentChanged: {
        if (current)
            store.select(current.session);
    }

    function move(d) {
        var next = index + d;
        if (next < 0 || next >= shown.length) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
    }

    function play() {
        if (!current) {
            Sound.edge();
            return;
        }
        Sound.ok();
        shell.push("pages/PlayerPage.qml", { session: current.session, gameId: current.gameId });
    }

    function options() {
        if (!current) {
            Sound.edge();
            return;
        }
        Sound.ok();
        var row = current;
        var items = [{ label: "Play", act: "play" }];
        if (row.hasJournal)
            items.push({ label: "Open journal entry", act: "journal" });
        items.push({ label: "Remove recording…", act: "remove" });
        shell.pick({ title: row.gameTitle + " · " + row.dateText, choices: items.map(function(i) { return i.label; }) }, function(i) {
            if (i < 0)
                return;
            if (items[i].act === "play")
                play();
            else if (items[i].act === "journal")
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
                    index = 0;
                }
            });
        } else if (id === "sort") {
            shell.pick({ title: "Sort", choices: ["Newest First", "Oldest First"], index: oldestFirst ? 1 : 0 }, function(i) {
                if (i >= 0) {
                    oldestFirst = i === 1;
                    index = 0;
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
                Sound.edge();
        }
    }

    Text {
        anchors.centerIn: grid
        visible: page.shown.length === 0
        text: page.rows.length === 0 ? "No recordings yet. A session recorded by the capture module lands here." : "No recordings for this game yet."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(Theme.fontBody)
    }

    FocusScope {
        id: grid

        x: page.gridX
        y: page.gridY
        width: page.columns * page.thumbWidth + (page.columns - 1) * page.gap
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "grid"

        readonly property real pitchY: page.thumbHeight + page.gap
        readonly property int lastRow: page.shown.length > 0 ? Math.floor((page.shown.length - 1) / page.columns) : 0
        // The view clips; it reaches this far past the cells so the focus ring is never cut.
        readonly property real room: Theme.dp(Theme.ringRoom)

        function scrollToCurrent() {
            if (height <= 0)
                return;
            var row = Math.floor(page.index / page.columns);
            var top = row * pitchY, bottom = top + page.thumbHeight + room * 2;
            Theme.reveal(view, top, bottom, height);
        }

        Keys.onLeftPressed: {
            if (page.index % page.columns === 0) {
                Sound.tick();
                page.zone = "rail";
            } else {
                page.move(-1);
            }
        }
        Keys.onRightPressed: page.move(1)
        Keys.onUpPressed: {
            if (page.index >= page.columns)
                page.move(-page.columns);
            else
                Sound.edge();
        }
        Keys.onDownPressed: {
            if (Math.floor(page.index / page.columns) < lastRow)
                page.move(Math.min(page.columns, page.shown.length - 1 - page.index));
            else
                Sound.edge();
        }

        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isAccept(event)) {
                event.accepted = true;
                page.play();
            } else if (api.keys.isMenu(event)) {
                event.accepted = true;
                page.options();
            }
        }

        Flickable {
            id: view

            anchors.fill: parent
            anchors.margins: -grid.room
            contentWidth: width
            contentHeight: (grid.lastRow + 1) * grid.pitchY + grid.room * 2
            interactive: false
            clip: true

            Behavior on contentY {
                NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic }
            }

            Repeater {
                model: page.shown

                Item {
                    id: cell

                    readonly property var row: modelData
                    readonly property var frames: page.frameMap[row.session] || null
                    readonly property bool focused: grid.activeFocus && index === page.index

                    x: grid.room + (index % page.columns) * (page.thumbWidth + page.gap)
                    y: grid.room + Math.floor(index / page.columns) * grid.pitchY
                    width: page.thumbWidth
                    height: page.thumbHeight
                    z: focused ? 2 : 1

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

                            Behavior on opacity {
                                NumberAnimation { duration: Theme.durFade; easing.type: Easing.OutCubic }
                            }
                        }
                    }

                    FocusOutline {
                        target: body
                        cornerRadius: body.radius
                        shown: cell.focused
                    }

                    Tile {
                        x: Theme.dp(8)
                        y: Theme.dp(8)
                        width: Theme.dp(40)
                        height: Theme.dp(40)
                        cornerRadius: Theme.dp(4)
                        game: api.allGames.byId(cell.row.gameId)
                        outlineShown: false
                        lift: false
                    }

                    Rectangle {
                        anchors.right: parent.right
                        anchors.bottom: parent.bottom
                        anchors.margins: Theme.dp(6)
                        width: durationText.implicitWidth + Theme.dp(18)
                        height: Theme.dp(34)
                        radius: Theme.dp(3)
                        color: Qt.rgba(0, 0, 0, 0.55)

                        Text {
                            id: durationText
                            anchors.centerIn: parent
                            text: cell.row.durationText
                            color: "#ffffff"
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(Theme.fontTiny)
                        }
                    }
                }
            }
        }

        Scrollbar {
            anchors.right: parent.right
            anchors.rightMargin: -Theme.dp(96)
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            flickable: view
        }
    }
}
