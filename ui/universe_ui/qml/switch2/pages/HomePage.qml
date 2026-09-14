import QtQuick
import Universe
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    property var shell: null

    readonly property int gameCount: recent.count
    readonly property int allIndex: gameCount
    property int index: 0
    readonly property bool onAll: index === allIndex
    readonly property var currentGame: !onAll && index >= 0 && index < gameCount ? recent.get(index) : null
    readonly property var session: api.universe.currentSession
    readonly property string playingId: session && session.id !== undefined ? session.id : ""

    readonly property bool onPlaying: currentGame !== null && currentGame.id === playingId
    readonly property var hints: onAll ? [ { glyph: "A", label: "OK" } ]
        : onPlaying ? [ { glyph: "Start", label: "Options" }, { glyph: "X", label: "Close" }, { glyph: "A", label: "Resume" } ]
        : [ { glyph: "Start", label: "Options" }, { glyph: "A", label: "Start" } ]

    readonly property real tile: Theme.dp(Theme.tileSize)
    readonly property real gap: Theme.dp(Theme.tileGap)
    readonly property real pitch: tile + gap
    readonly property real rowX: Theme.dp(Theme.tileRowX)

    signal escapedDown()

    RecentGames {
        id: played
        sourceModel: api.allGames
        playingId: page.playingId
    }

    // The HOME row holds the last twelve, as the console's does; All Software has the rest.
    LimitedGames {
        id: recent
        sourceModel: played
        limit: 12
    }

    function step(d) {
        var next = Math.max(0, Math.min(allIndex, index + d));
        if (next === index) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
    }

    function activate() {
        if (onAll) {
            Sound.ok();
            shell.push("pages/AllSoftwarePage.qml", {});
            return;
        }
        if (!currentGame) {
            Sound.edge();
            return;
        }
        if (currentGame.id === playingId) {
            shell.resume();
            return;
        }
        shell.launch(currentGame);
    }

    onGameCountChanged: {
        index = Math.min(index, allIndex);
        Qt.callLater(row.slideToCurrent);
    }
    onIndexChanged: row.slideToCurrent()

    Keys.onLeftPressed: step(-1)
    Keys.onRightPressed: step(1)
    Keys.onDownPressed: {
        Sound.tick();
        page.escapedDown();
    }
    Keys.onUpPressed: Sound.edge()

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            activate();
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            if (currentGame) {
                Sound.ok();
                shell.push("pages/SoftwareOptionsPage.qml", { gameId: currentGame.id });
            } else {
                Sound.edge();
            }
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            if (currentGame && currentGame.id === playingId) {
                Sound.ok();
                shell.closeSoftware(currentGame);
            } else {
                Sound.edge();
            }
        }
    }

    Text {
        id: title

        // Centred over the focused tile; what would run past the screen's margin is elided, not shifted.
        readonly property real centre: page.index * page.pitch - row.contentX + page.tile / 2
        readonly property real margin: Theme.dp(Theme.edgeMargin)
        readonly property real room: 2 * Math.min(centre - margin, page.width - margin - centre)

        x: centre - width / 2
        y: Theme.dp(Theme.tileRowY) - Theme.dp(66)
        width: Math.max(0, Math.min(implicitWidth, page.pitch * 2.5, room))
        visible: page.activeFocus && (page.currentGame !== null || page.onAll)
        text: page.onAll ? "All Software" : (page.currentGame ? page.currentGame.title : "")
        color: Theme.accent
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
        font.family: Theme.sans
        font.pixelSize: Theme.dp(Theme.fontBody)
    }

    ListView {
        id: row

        y: Theme.dp(Theme.tileRowY) - Theme.dp(30)
        width: parent.width
        height: page.tile + Theme.dp(60)
        orientation: ListView.Horizontal
        model: page.gameCount + 1
        spacing: page.gap
        leftMargin: page.rowX
        rightMargin: page.rowX
        interactive: false
        keyNavigationEnabled: false
        highlightFollowsCurrentItem: false
        cacheBuffer: page.pitch * 4
        clip: false

        // The cursor walks the tiles in view; the row slides only when the focused one would leave
        // the margins, and then just far enough to keep it inside them, ring and title included.
        function slideToCurrent() {
            var left = page.index * page.pitch;
            var right = left + page.tile;
            var target = contentX;
            if (right > contentX + width - rightMargin)
                target = right - width + rightMargin;
            if (left < contentX + leftMargin)
                target = left - leftMargin;
            contentX = Math.max(-leftMargin, Math.min(target, Math.max(-leftMargin, contentWidth - width + rightMargin)));
        }

        Behavior on contentX {
            NumberAnimation { duration: Theme.durFocus; easing.type: Easing.OutCubic }
        }

        delegate: Item {
            id: cell

            readonly property bool isAll: index === page.allIndex
            readonly property bool isGame: index < page.gameCount
            readonly property bool focused: page.activeFocus && (page.onAll ? isAll : index === page.index)
            readonly property var game: isGame ? recent.get(index) : null

            width: page.tile
            height: row.height
            // The ring reaches over the neighbours, which are later siblings.
            z: focused ? 2 : 1

            Tile {
                id: art
                visible: !cell.isAll
                y: Theme.dp(30)
                width: page.tile
                height: page.tile
                game: cell.game
                focused: cell.focused
            }

            Text {
                visible: cell.isGame && cell.game && cell.game.id === page.playingId
                anchors.top: art.bottom
                anchors.topMargin: Theme.dp(Theme.ringRoom + 4)
                anchors.horizontalCenter: art.horizontalCenter
                text: "Playing"
                color: Theme.accent
                font.family: Theme.sans
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Item {
                visible: cell.isAll
                anchors.centerIn: art
                width: Theme.dp(236)
                height: width

                Rectangle {
                    id: disc
                    anchors.fill: parent
                    radius: width / 2
                    color: Theme.slot
                }

                FocusOutline {
                    target: disc
                    cornerRadius: disc.radius
                    shown: cell.focused
                }

                Glyph {
                    anchors.centerIn: parent
                    width: Theme.dp(96)
                    height: width
                    kind: "grid"
                    tint: Theme.barGrey
                    stroke: 1.6
                }
            }
        }
    }
}
