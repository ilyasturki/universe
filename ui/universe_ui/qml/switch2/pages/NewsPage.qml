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

    property int tab: 0
    property string filterId: ""
    property string zone: "grid"
    property int index: 0

    readonly property var articles: rows.filter(function(r) { return filterId === "" || r.gameId === filterId; })

    readonly property var channels: Feed.channels(rows)

    readonly property var list: tab === 0 ? articles : channels
    readonly property var current: index >= 0 && index < list.length ? list[index] : null

    readonly property var hints: zone === "rail"
        ? [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]
        : [ { glyph: "Start", label: "Options", dim: current === null || tab !== 0 }, { glyph: "B", label: "Back" }, { glyph: "A", label: tab === 0 ? "Read" : "Open", dim: current === null } ]

    signal closeRequested()

    readonly property int columns: 3
    readonly property real cardWidth: Theme.dp(489)
    readonly property real imageHeight: Math.round(cardWidth * 9 / 16)
    readonly property real captionHeight: Theme.dp(105)
    readonly property real cardHeight: imageHeight + captionHeight
    readonly property real gap: Theme.dp(18)
    readonly property real gridX: Theme.dp(253)
    readonly property real gridY: Theme.dp(226)
    readonly property real channelHeight: Theme.dp(130)

    focus: true

    Component.onCompleted: store.loadAll()
    Component.onDestruction: store.unload()

    onArgsChanged: {
        if (args.gameId)
            filterId = args.gameId;
    }

    onArticlesChanged: clamp()
    onChannelsChanged: clamp()
    onTabChanged: {
        index = 0;
        view.contentY = 0;
    }
    onIndexChanged: scrollToCurrent()

    function clamp() {
        if (index >= list.length)
            index = Math.max(0, list.length - 1);
        scrollToCurrent();
    }

    function scrollToCurrent() {
        if (view.height <= 0)
            return;
        var top, bottom;
        if (tab === 0) {
            var row = Math.floor(index / columns);
            top = row * (cardHeight + gap);
            bottom = top + cardHeight + grid.room * 2;
        } else {
            top = index * channelHeight;
            bottom = top + channelHeight + grid.room * 2;
        }
        Theme.reveal(view, top, bottom, view.height);
    }

    function move(d) {
        var next = index + d;
        if (next < 0 || next >= list.length) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
    }

    function options() {
        if (!current || tab !== 0) {
            Sound.edge();
            return;
        }
        Sound.ok();
        var row = current;
        var pending = row.state === "pending";
        var items = pending ? [] : [{ label: "Read", act: "read" }];
        if (row.hasRecording)
            items.push({ label: "Watch the recording", act: "recording" });
        items.push({ label: pending ? "Cancel the writing…" : "Remove entry…", act: "remove" });
        shell.pick({ title: row.gameTitle + " · " + row.dateText, choices: items.map(function(i) { return i.label; }) }, function(i) {
            if (i < 0)
                return;
            if (items[i].act === "read")
                shell.push("pages/ArticlePage.qml", { session: row.session, gameId: row.gameId });
            else if (items[i].act === "recording")
                shell.push("pages/PlayerPage.qml", { session: row.session, gameId: row.gameId });
            else
                Removal.entry(shell, api.screens, row, function() {});
        });
    }

    function activate() {
        if (!current) {
            Sound.edge();
            return;
        }
        Sound.ok();
        if (tab === 0) {
            shell.push("pages/ArticlePage.qml", { session: current.session, gameId: current.gameId });
        } else {
            filterId = current.id;
            tabs.index = 0;
            tab = 0;
        }
    }

    function railAction(id) {
        if (id === "filter") {
            var ids = [""].concat(channels.map(function(c) { return c.id; }));
            var choices = ["All"].concat(channels.map(function(c) { return c.title; }));
            shell.pick({ title: "Show", choices: choices, index: Math.max(0, ids.indexOf(filterId)) }, function(i) {
                if (i >= 0) {
                    filterId = ids[i];
                    index = 0;
                    if (tab !== 0) {
                        tabs.index = 0;
                        tab = 0;
                    }
                }
            });
        }
    }

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

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "news"
        title: "News"
        trailing: page.filterId !== "" ? page.filterName() : ""
        hairline: false
    }

    function filterName() {
        var g = api.allGames.byId(filterId);
        return g ? g.title : filterId;
    }

    Tabs {
        id: tabs
        anchors.top: header.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        height: Theme.dp(70)
        names: ["Articles", "Discover"]
        onChanged: function(i) { page.tab = i; }
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
            if (page.list.length > 0)
                page.zone = "grid";
            else
                Sound.edge();
        }
    }

    Text {
        anchors.centerIn: grid
        visible: page.list.length === 0
        text: page.rows.length === 0 ? "No entries yet. The journal writes one after each session." : "No entries for this game yet."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(Theme.fontBody)
    }

    FocusScope {
        id: grid

        x: page.gridX
        y: page.gridY
        width: page.columns * page.cardWidth + (page.columns - 1) * page.gap
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        focus: page.zone === "grid"

        readonly property int lastRow: page.articles.length > 0 ? Math.floor((page.articles.length - 1) / page.columns) : 0
        // The view clips; it reaches this far past the cards so the focus ring is never cut.
        readonly property real room: Theme.dp(Theme.ringRoom)

        Keys.onLeftPressed: {
            if (page.tab !== 0 || page.index % page.columns === 0) {
                Sound.tick();
                page.zone = "rail";
            } else {
                page.move(-1);
            }
        }
        Keys.onRightPressed: page.tab === 0 ? page.move(1) : Sound.edge()
        Keys.onUpPressed: {
            if (page.tab !== 0)
                page.move(-1);
            else if (page.index >= page.columns)
                page.move(-page.columns);
            else
                Sound.edge();
        }
        Keys.onDownPressed: {
            if (page.tab !== 0)
                page.move(1);
            else if (Math.floor(page.index / page.columns) < lastRow)
                page.move(Math.min(page.columns, page.articles.length - 1 - page.index));
            else
                Sound.edge();
        }

        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isAccept(event)) {
                event.accepted = true;
                page.activate();
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
            contentHeight: page.tab === 0
                ? (grid.lastRow + 1) * (page.cardHeight + page.gap) + grid.room * 2
                : page.channels.length * page.channelHeight + grid.room * 2
            interactive: false
            clip: true

            Behavior on contentY {
                NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic }
            }

            Repeater {
                model: page.tab === 0 ? page.articles : []

                Item {
                    id: card

                    readonly property var row: modelData
                    readonly property bool focused: grid.activeFocus && index === page.index
                    readonly property string picture: row.images && row.images.length > 0 ? row.images[0] : ""
                    readonly property var game: api.allGames.byId(row.gameId)

                    x: grid.room + (index % page.columns) * (page.cardWidth + page.gap)
                    y: grid.room + Math.floor(index / page.columns) * (page.cardHeight + page.gap)
                    width: page.cardWidth
                    height: page.cardHeight
                    z: focused ? 2 : 1

                    Rectangle {
                        id: body
                        anchors.fill: parent
                        radius: Theme.dp(6)
                        color: Theme.card
                    }

                    FocusOutline {
                        target: body
                        cornerRadius: body.radius
                        shown: card.focused
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
                            game: card.game
                            cornerRadius: 0
                            outlineShown: false
                            lift: false
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
                        lift: false
                    }

                    Text {
                        x: badge.x + badge.width + Theme.dp(20)
                        y: page.imageHeight + Theme.dp(14)
                        width: parent.width - x - Theme.dp(18)
                        height: page.captionHeight - Theme.dp(14)
                        verticalAlignment: Text.AlignVCenter
                        text: card.row.title
                        color: Theme.text
                        wrapMode: Text.WordWrap
                        maximumLineCount: 2
                        elide: Text.ElideRight
                        lineHeight: 1.15
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }
                }
            }

            Repeater {
                model: page.tab === 1 ? page.channels : []

                Item {
                    id: channel

                    readonly property bool focused: grid.activeFocus && index === page.index

                    x: grid.room
                    y: grid.room + index * page.channelHeight
                    width: grid.width
                    height: page.channelHeight

                    Rectangle {
                        id: pill
                        anchors.fill: parent
                        anchors.bottomMargin: Theme.dp(6)
                        radius: Theme.dp(Theme.radiusRow)
                        color: Theme.focusFill
                        visible: channel.focused
                    }

                    FocusOutline {
                        target: pill
                        cornerRadius: pill.radius
                        gap: 0
                        shown: channel.focused
                    }

                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.bottomMargin: Theme.dp(6)
                        height: 1
                        visible: !channel.focused
                        color: Theme.hairlineSoft
                    }

                    Tile {
                        id: channelArt
                        x: Theme.dp(24)
                        anchors.verticalCenter: pill.verticalCenter
                        width: Theme.dp(84)
                        height: Theme.dp(84)
                        cornerRadius: Theme.dp(6)
                        game: api.allGames.byId(modelData.id)
                        outlineShown: false
                        lift: false
                    }

                    Text {
                        x: channelArt.x + channelArt.width + Theme.dp(28)
                        anchors.verticalCenter: pill.verticalCenter
                        width: parent.width - x - Theme.dp(300)
                        text: modelData.title
                        color: Theme.text
                        elide: Text.ElideRight
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontBody)
                    }

                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(30)
                        anchors.verticalCenter: pill.verticalCenter
                        text: modelData.count + (modelData.count === 1 ? " entry" : " entries")
                        color: Theme.accent
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontBody)
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
