import QtQuick
import "../core"
import "../sound"

// The Game Library's grid: five square tiles a row, or three gamelist cards a row; the tabs and the count line
// ride above the first row and scroll away with it.
FocusScope {
    id: grid

    // Games, or gamelists ({ key, name, games }) when `lists`.
    property var items: []
    property bool lists: false
    property int index: 0
    property string playingId: ""

    property var tabNames: []
    property int tab: 0
    property bool tabsActive: false
    property string countText: ""
    property string sortText: ""
    property string emptyText: ""

    readonly property int count: items.length
    readonly property var current: index >= 0 && index < count ? items[index] : null
    readonly property bool cursorShown: activeFocus && !tabsActive

    signal escapedLeft
    signal escapedUp
    signal activated(int index)
    signal menuRequested(int index)
    signal infoRequested(int index)
    signal favouriteRequested(int index)
    signal tabPicked(int index)
    // The tabs hold the cursor: Left and Right step them, Down, A or B come back to the grid.
    signal tabStepped(int step)
    signal tabsLeft
    signal pointed

    readonly property int columns: lists ? 3 : 5
    readonly property real gap: Theme.dp(24)
    readonly property real tile: Math.floor((width + gap) / columns - gap)
    readonly property real cardHeight: Math.round(tile * 0.57)
    readonly property real cellW: tile + gap
    readonly property real cellH: (lists ? cardHeight : tile) + gap
    readonly property int lastRow: count > 0 ? Math.floor((count - 1) / columns) : 0
    // The tabs and the count line, from the grid's top to its first row.
    readonly property real headerHeight: Theme.dp(194)
    readonly property real radius: Theme.dp(6)

    property string heldId: ""

    function go(next) {
        index = Sound.stepped(index, next - index, count);
    }

    function point(i) {
        Sound.play("tick");
        index = i;
        if (!activeFocus)
            grid.pointed();
        forceActiveFocus();
    }

    function keyOf(item) {
        return item ? (grid.lists ? item.key : item.id) : "";
    }

    // The same game or gamelist, wherever the list put it now.
    function hold(key) {
        if (key === "")
            return;
        for (var i = 0; i < items.length; i++)
            if (keyOf(items[i]) === key) {
                index = i;
                return;
            }
    }

    onItemsChanged: {
        hold(heldId);
        if (index >= count)
            index = Math.max(0, count - 1);
        Qt.callLater(view.scrollToCurrent);
    }
    onIndexChanged: {
        heldId = keyOf(current);
        view.scrollToCurrent();
    }
    onColumnsChanged: Qt.callLater(view.scrollToCurrent)
    onHeightChanged: view.scrollToCurrent()

    Connections {
        target: api.universe
        function onSessionEnded(sessionId, id, duration) {
            if (!grid.lists)
                grid.hold(id);
        }
    }

    function tabKeys(event) {
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            grid.tabStepped(event.key === Qt.Key_Left ? -1 : 1);
        } else if (event.key === Qt.Key_Up) {
            Sound.play("edge");
        } else if (event.key === Qt.Key_Down || (!event.isAutoRepeat && (api.keys.isAccept(event) || api.keys.isCancel(event)))) {
            Sound.play(api.keys.isCancel(event) ? "back" : "tick");
            grid.tabsLeft();
        } else {
            return;
        }
        event.accepted = true;
    }

    Keys.onPressed: function (event) {
        if (tabsActive) {
            tabKeys(event);
            return;
        }
        if (event.key === Qt.Key_Right) {
            event.accepted = true;
            index % columns === columns - 1 || index >= count - 1 ? Sound.play("edge") : go(index + 1);
            return;
        }
        if (event.key === Qt.Key_Left) {
            event.accepted = true;
            if (count > 0 && index % columns !== 0) {
                go(index - 1);
            } else {
                Sound.play("tick");
                grid.escapedLeft();
            }
            return;
        }
        if (event.key === Qt.Key_Down) {
            event.accepted = true;
            Math.floor(index / columns) < lastRow ? go(Math.min(index + columns, count - 1)) : Sound.play("edge");
            return;
        }
        if (event.key === Qt.Key_Up) {
            event.accepted = true;
            if (index >= columns) {
                go(index - columns);
            } else {
                Sound.play("tick");
                grid.escapedUp();
            }
            return;
        }
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (screen) {
            event.accepted = true;
            index = Sound.paged(index, screen, columns, Math.max(1, Math.floor(view.height / cellH)), count);
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (current) {
                Sound.play("ok");
                grid.activated(index);
            } else {
                Sound.play("edge");
            }
        } else if (api.keys.isMenu(event) && !lists) {
            event.accepted = true;
            current ? grid.menuRequested(index) : Sound.play("edge");
        } else if (api.keys.isDetails(event) && !lists) {
            event.accepted = true;
            current ? grid.infoRequested(index) : Sound.play("edge");
        } else if (api.keys.isFilters(event) && !lists) {
            event.accepted = true;
            current ? grid.favouriteRequested(index) : Sound.play("edge");
        }
    }

    GridView {
        id: view

        anchors.fill: parent
        anchors.leftMargin: -Theme.dp(Theme.edge)
        anchors.rightMargin: -Theme.dp(Theme.columnRight)
        leftMargin: Theme.dp(Theme.edge)
        // The last column's gap falls in the margin, or the view fits one column fewer.
        rightMargin: Theme.dp(Theme.columnRight) - grid.gap
        bottomMargin: Theme.dp(80)
        model: grid.items
        cellWidth: grid.cellW
        cellHeight: grid.cellH
        interactive: false
        keyNavigationEnabled: false
        highlightFollowsCurrentItem: false
        clip: true
        cacheBuffer: grid.cellH * 2

        // The first row shows the tabs above it; past it, the focused row keeps a little room under the title.
        function scrollToCurrent() {
            if (height <= 0)
                return;
            var row = Math.floor(grid.index / grid.columns);
            var top = row * grid.cellH;
            var bottom = top + grid.cellH;
            var target = contentY;
            if (row === 0 || grid.count === 0)
                target = originY;
            else if (top - Theme.dp(40) < contentY)
                target = top - Theme.dp(40);
            else if (bottom + Theme.dp(20) > contentY + height)
                target = bottom + Theme.dp(20) - height;
            var high = Math.max(originY, originY + contentHeight - height + bottomMargin);
            contentY = Math.max(originY, Math.min(target, high));
        }

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        header: Item {
            width: grid.width
            height: grid.headerHeight

            Row {
                id: tabRow
                x: -Theme.dp(18)
                y: Theme.dp(43) - height / 2
                spacing: Theme.dp(66 - 36)

                Repeater {
                    model: grid.tabNames

                    TabLabel {
                        text: modelData
                        current: index === grid.tab
                        focused: grid.tabsActive && index === grid.tab
                        pad: Theme.dp(18)
                        boxHeight: Theme.dp(64)
                        onPicked: grid.tabPicked(index)
                    }
                }
            }

            Label {
                y: Theme.dp(153) - height / 2
                text: grid.countText
                font.pixelSize: Theme.dp(26)
            }

            Label {
                anchors.right: parent.right
                y: Theme.dp(153) - height / 2
                text: grid.sortText
                font.pixelSize: Theme.dp(26)
            }
        }

        delegate: Item {
            id: cell

            readonly property bool focused: grid.cursorShown && index === grid.index
            readonly property var entry: modelData
            // The list swaps between games and gamelists in one step: each delegate reads only what its kind has.
            readonly property var game: !grid.lists && entry && entry.assets ? entry : null
            readonly property var gamelist: grid.lists && entry && entry.games ? entry : null
            readonly property bool installing: game !== null && game.installing === true
            readonly property bool playing: game !== null && game.id === grid.playingId

            width: view.cellWidth
            height: view.cellHeight
            // The frame reaches over the neighbours, which are later siblings.
            z: focused ? 2 : 1

            Item {
                id: face
                width: grid.tile
                height: grid.lists ? grid.cardHeight : grid.tile

                TileArt {
                    anchors.fill: parent
                    visible: !grid.lists
                    game: cell.game
                    radius: grid.radius
                    dimmed: cell.installing
                }

                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: Math.round(parent.width * 0.06)
                    anchors.bottomMargin: Math.round(parent.width * 0.06)
                    height: Math.max(3, Theme.dp(5))
                    radius: height / 2
                    visible: cell.installing
                    color: Qt.rgba(1, 1, 1, 0.25)

                    Rectangle {
                        width: parent.width * Math.max(0.02, cell.game && cell.game.progress >= 0 ? cell.game.progress : 0)
                        height: parent.height
                        radius: parent.radius
                        color: Theme.text
                    }
                }

                Label {
                    x: Math.round(parent.width * 0.055)
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: Math.round(parent.width * (cell.installing ? 0.1 : 0.05))
                    visible: cell.installing || cell.playing
                    text: cell.installing ? "Installing" : "Playing"
                    style: Text.Raised
                    styleColor: Qt.rgba(0, 0, 0, 0.6)
                    font.pixelSize: Math.max(Theme.dp(14), Math.round(grid.tile * 0.064))
                }

                Rectangle {
                    anchors.fill: parent
                    visible: grid.lists
                    radius: grid.radius
                    color: Qt.rgba(0.1, 0.11, 0.14, 0.9)
                    border.width: 1
                    border.color: Theme.glassEdge

                    Row {
                        id: mosaic
                        x: Math.round(parent.width * 0.055)
                        y: x
                        spacing: Math.round(parent.width * 0.047)

                        readonly property real side: Math.floor((parent.width - x * 2 - spacing * 2) / 3)

                        Repeater {
                            model: grid.lists ? 3 : 0

                            TileArt {
                                readonly property var games: cell.gamelist ? cell.gamelist.games : []
                                width: mosaic.side
                                height: width
                                radius: Theme.dp(3)
                                game: index < games.length ? games[index] : null
                                opacity: index < games.length ? 1.0 : 0.0
                            }
                        }
                    }

                    Label {
                        x: mosaic.x
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: Math.round(parent.height * 0.1)
                        width: parent.width * 0.6
                        text: cell.gamelist ? cell.gamelist.name : ""
                        elide: Text.ElideRight
                        font.weight: Font.Light
                        font.pixelSize: Theme.dp(28)
                    }

                    Label {
                        anchors.right: parent.right
                        anchors.rightMargin: mosaic.x
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: Math.round(parent.height * 0.1)
                        text: cell.gamelist ? cell.gamelist.games.length + (cell.gamelist.games.length === 1 ? " game" : " games") : ""
                        font.weight: Font.DemiBold
                        font.pixelSize: Theme.dp(22)
                    }
                }

                FocusFrame {
                    target: face
                    shown: cell.focused
                    radius: grid.radius
                    gap: Theme.dp(4)
                    line: Theme.dp(3)
                }

                Touch {
                    current: cell.focused
                    menu: !grid.lists
                    onPicked: grid.point(index)
                }
            }
        }
    }

    Swipe {
        flickable: view
        ease: scrollEase
    }

    // The focused game's name under its tile, over the gap, as the tiles carry art rather than names.
    Rectangle {
        id: name

        readonly property real cellX: (grid.index % grid.columns) * grid.cellW
        readonly property real cellY: Math.floor(grid.index / grid.columns) * grid.cellH - view.contentY

        visible: !grid.lists && grid.cursorShown && grid.current !== null && cellY + grid.tile + height < grid.height
        x: Math.max(0, Math.min(cellX + (grid.tile - width) / 2, grid.width - width))
        y: cellY + grid.tile + Theme.dp(10)
        width: Math.min(nameText.implicitWidth + Theme.dp(32), grid.tile + Theme.dp(120))
        height: Theme.dp(44)
        radius: height / 2
        z: 3
        color: Qt.rgba(0.07, 0.08, 0.1, 0.92)
        border.width: 1
        border.color: Theme.glassEdge

        Label {
            id: nameText
            anchors.centerIn: parent
            width: parent.width - Theme.dp(32)
            horizontalAlignment: Text.AlignHCenter
            text: grid.current && !grid.lists && grid.current.title ? grid.current.title : ""
            elide: Text.ElideRight
            font.pixelSize: Theme.dp(Theme.fontTiny)
        }
    }

    Label {
        anchors.horizontalCenter: parent.horizontalCenter
        y: grid.headerHeight + Theme.dp(80)
        width: parent.width - Theme.dp(200)
        visible: grid.count === 0 && grid.emptyText !== ""
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: grid.emptyText
        color: Theme.textSecondary
        lineHeight: 1.3
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    // The header sits above the content's origin: the thumb measures from there.
    Item {
        id: bar

        readonly property real span: view.contentHeight
        readonly property bool needed: span > view.height + 1

        x: grid.width + Theme.dp(24)
        y: grid.headerHeight
        width: Theme.dp(3)
        height: grid.height - grid.headerHeight - Theme.dp(20)
        visible: needed

        Rectangle {
            anchors.fill: parent
            radius: width / 2
            color: Qt.rgba(1, 1, 1, 0.08)
        }

        Rectangle {
            width: parent.width
            y: bar.needed ? parent.height * (view.contentY - view.originY) / bar.span : 0
            height: bar.needed ? Math.max(Theme.dp(40), parent.height * view.height / bar.span) : 0
            radius: width / 2
            color: Qt.rgba(1, 1, 1, 0.5)
        }
    }
}
