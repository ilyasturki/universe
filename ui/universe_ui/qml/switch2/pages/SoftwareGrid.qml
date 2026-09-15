import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: grid

    // A JS array of games or groups, or a game proxy model.
    property var games: []
    property bool groups: false
    property int index: 0
    property bool escapesLeft: true
    readonly property bool listed: Array.isArray(games)
    readonly property int count: listed ? games.length : games ? games.count : 0
    readonly property var current: index < 0 ? null : listed ? (index < games.length ? games[index] : null) : (games && index < games.count ? games.get(index) : null)
    readonly property bool cursorShown: activeFocus

    signal escapedLeft()
    signal activated(int index)
    signal optionsRequested(int index)

    readonly property int columns: 6
    readonly property real tile: Theme.dp(237)
    readonly property real gap: Theme.dp(18)
    readonly property real pitch: tile + gap
    readonly property real inset: Theme.dp(Theme.ringRoom)
    readonly property real cornerRadius: Math.round(Theme.dp(Theme.radiusTile) * tile / Theme.dp(Theme.tileSize))
    readonly property real cellHeight: groups ? pitch + inset + Theme.dp(64) : pitch
    readonly property int lastRow: count > 0 ? Math.floor((count - 1) / columns) : 0

    implicitWidth: columns * pitch

    function go(next) { index = Sound.stepped(index, next - index, count); }

    Keys.onRightPressed: index % columns === columns - 1 || index === count - 1 ? Sound.play("edge") : go(index + 1)
    Keys.onLeftPressed: {
        if (index % columns !== 0) {
            go(index - 1);
        } else if (escapesLeft) {
            Sound.play("tick");
            grid.escapedLeft();
        } else {
            Sound.play("edge");
        }
    }
    Keys.onDownPressed: Math.floor(index / columns) < lastRow ? go(Math.min(index + columns, count - 1)) : Sound.play("edge")
    Keys.onUpPressed: index >= columns ? go(index - columns) : Sound.play("edge")

    Keys.onPressed: function(event) {
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
        } else if (api.keys.isMenu(event) && !groups) {
            event.accepted = true;
            if (current) {
                Sound.play("ok");
                grid.optionsRequested(index);
            } else {
                Sound.play("edge");
            }
        }
    }

    onCountChanged: {
        if (index >= count)
            index = Math.max(0, count - 1);
        view.scrollToCurrent();
    }
    onIndexChanged: view.scrollToCurrent()
    onHeightChanged: view.scrollToCurrent()

    Label {
        anchors.centerIn: parent
        anchors.verticalCenterOffset: -Theme.dp(60)
        width: parent.width - Theme.dp(200)
        visible: grid.count === 0
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: grid.groups ? "Groups gather your games: your favourites, each platform, each tag you give a game."
                          : "No software matches."
        lineHeight: 1.3
    }

    GridView {
        id: view

        anchors.fill: parent
        anchors.margins: -grid.inset
        model: grid.games
        cellWidth: grid.pitch
        cellHeight: grid.cellHeight
        leftMargin: grid.inset
        rightMargin: grid.inset
        topMargin: grid.inset
        bottomMargin: grid.groups ? grid.inset : Theme.dp(120)
        interactive: false
        keyNavigationEnabled: false
        highlightFollowsCurrentItem: false
        clip: true
        cacheBuffer: grid.cellHeight * 2

        function scrollToCurrent() {
            if (height <= 0 || grid.count === 0)
                return;
            if (contentHeight + topMargin + bottomMargin <= height) {
                contentY = -topMargin;
                return;
            }
            var rowTop = Math.floor(grid.index / grid.columns) * cellHeight;
            var target = contentY;
            if (rowTop - grid.inset < contentY)
                target = rowTop - grid.inset;
            else if (rowTop + cellHeight + grid.inset > contentY + height)
                target = rowTop + cellHeight + grid.inset - height;
            contentY = Math.max(-topMargin, Math.min(target, contentHeight - height + bottomMargin));
        }

        Behavior on contentY { Ease {} }

        delegate: Item {
            id: cell

            readonly property bool focused: grid.cursorShown && index === grid.index
            readonly property var entry: modelData

            width: view.cellWidth
            height: view.cellHeight
            // The ring reaches over the neighbours, which are later siblings.
            z: focused ? 2 : 1

            Loader {
                active: !grid.groups
                width: grid.tile
                height: grid.tile
                sourceComponent: Tile {
                    cornerRadius: grid.cornerRadius
                    game: cell.entry
                    focused: cell.focused
                }
            }

            Item {
                id: mosaic
                visible: grid.groups
                width: grid.tile
                height: grid.tile

                Rectangle {
                    id: mosaicBase
                    anchors.fill: parent
                    radius: grid.cornerRadius
                    color: Theme.slot
                }

                Grid {
                    anchors.fill: parent
                    anchors.margins: Theme.dp(8)
                    columns: 2
                    spacing: Theme.dp(6)

                    Repeater {
                        model: grid.groups ? 4 : 0

                        Tile {
                            readonly property var list: grid.groups && cell.entry ? cell.entry.games : []
                            width: (mosaic.width - Theme.dp(16) - Theme.dp(6)) / 2
                            height: width
                            cornerRadius: Theme.dp(5)
                            game: index < list.length ? list[index] : null
                            outlineShown: false
                            opacity: index < list.length ? 1.0 : 0.0
                        }
                    }
                }

                FocusOutline {
                    target: mosaicBase
                    cornerRadius: mosaicBase.radius
                    shown: cell.focused && grid.groups
                }
            }

            Column {
                visible: grid.groups
                anchors.top: mosaic.bottom
                anchors.topMargin: grid.inset + Theme.dp(4)
                width: grid.tile
                spacing: Theme.dp(2)

                Label {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: grid.groups && cell.entry ? cell.entry.name : ""
                    color: cell.focused ? Theme.accent : Theme.text
                    elide: Text.ElideRight
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }

                Label {
                    width: parent.width
                    horizontalAlignment: Text.AlignHCenter
                    text: grid.groups && cell.entry ? cell.entry.games.length + (cell.entry.games.length === 1 ? " game" : " games") : ""
                    color: Theme.textSecondary
                    font.pixelSize: Theme.dp(Theme.fontTiny)
                }
            }
        }
    }

    Item {
        id: card

        readonly property bool shown: !grid.groups && grid.cursorShown && grid.current !== null
                                      && grid.index + grid.columns >= grid.count
        readonly property real cellX: (grid.index % grid.columns) * grid.pitch
        readonly property real cellY: Math.floor(grid.index / grid.columns) * grid.cellHeight - view.contentY - grid.inset

        visible: shown
        x: Math.max(-Theme.dp(80), Math.min(cellX - Theme.dp(42), grid.width - width))
        y: cellY + grid.tile + grid.inset + Theme.dp(4)
        width: cardText.implicitWidth + Theme.dp(84)
        height: Theme.dp(82)
        z: 3

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(8)
            color: Theme.card
            border.width: 1
            border.color: "#e0e0e0"
        }

        Canvas {
            x: card.cellX - card.x + grid.tile / 2 - width / 2
            y: -height + 1
            width: Theme.dp(28)
            height: Theme.dp(14)
            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                ctx.fillStyle = String(Theme.card);
                ctx.beginPath();
                ctx.moveTo(0, height);
                ctx.lineTo(width / 2, 0);
                ctx.lineTo(width, height);
                ctx.closePath();
                ctx.fill();
            }
        }

        Label {
            id: cardText
            anchors.centerIn: parent
            width: Math.min(implicitWidth, Theme.dp(760))
            text: grid.current && !grid.groups ? grid.current.title : ""
            color: Theme.accent
            elide: Text.ElideRight
        }
    }

    Scrollbar {
        x: grid.width + Theme.dp(100)
        height: grid.height
        flickable: view
    }
}
