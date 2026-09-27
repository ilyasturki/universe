import QtQuick
import Universe
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    property var shell: null

    readonly property int gameCount: shown.count
    readonly property int allIndex: gameCount
    readonly property bool empty: api.allGames.count === 0
    property int index: 0
    readonly property bool onAll: index === allIndex
    readonly property bool onSetup: empty && index === allIndex + 1
    readonly property var discs: empty ? ["plus", "settings"] : ["grid"]
    readonly property int last: allIndex + discs.length - 1
    readonly property var currentGame: anchor.game
    readonly property var session: api.universe.currentSession
    readonly property string playingId: session && session.id !== undefined ? session.id : ""

    readonly property bool onPlaying: currentGame !== null && currentGame.id === playingId
    readonly property bool onArriving: currentGame !== null && currentGame.installing
    readonly property var hints: onAll || onSetup ? [
        {
            glyph: "A",
            label: "OK"
        }
    ] : onArriving ? [
        {
            glyph: "A",
            label: "Manage"
        }
    ] : onPlaying ? [
        {
            glyph: "Start",
            label: "Options"
        },
        {
            glyph: "X",
            label: "Close"
        },
        {
            glyph: "A",
            label: "Resume"
        }
    ] : [
        {
            glyph: "Start",
            label: "Options"
        },
        {
            glyph: "A",
            label: "Start"
        }
    ]

    readonly property real tile: Theme.dp(Theme.tileSize)
    readonly property real gap: Theme.dp(Theme.tileGap)
    readonly property real pitch: tile + gap
    readonly property real rowX: Theme.dp(Theme.tileRowX)

    signal escapedDown
    signal pointed

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

    // An install under way sits first, as the console shows a download; its tile is the game's once it lands.
    HeadedGames {
        id: shown
        source: recent
        head: api.screens.sources.arriving
    }

    GameAnchor {
        id: anchor
        client: api.universe
        model: shown
        index: page.onAll ? -1 : page.index
        onMoved: function (next) {
            page.index = next;
        }
    }

    function step(d) {
        index = Sound.stepped(index, d, last + 1, "tick-tile");
    }

    function point(i) {
        Sound.play("tick-tile");
        index = i;
        if (!activeFocus)
            page.pointed();
        forceActiveFocus();
    }

    function activate() {
        if (onSetup) {
            Sound.play("ok");
            shell.push("pages/OnboardingPage.qml", {});
            return;
        }
        if (onAll) {
            Sound.play("ok");
            shell.push(empty ? "pages/AddGamePage.qml" : "pages/AllSoftwarePage.qml", {});
            return;
        }
        if (!currentGame) {
            Sound.play("edge");
            return;
        }
        if (onArriving) {
            Sound.play("ok");
            shell.push("pages/InstallPage.qml", {
                tab: 1
            });
            return;
        }
        var cell = row.itemAtIndex(index);
        onPlaying ? shell.resume() : shell.launch(currentGame, cell ? cell.tileArt : null);
    }

    onLastChanged: {
        index = Math.min(index, last);
        Qt.callLater(row.slideToCurrent);
    }
    onIndexChanged: row.slideToCurrent()

    Keys.onLeftPressed: step(-1)
    Keys.onRightPressed: step(1)
    Keys.onDownPressed: {
        Sound.play("tick");
        page.escapedDown();
    }
    Keys.onUpPressed: Sound.play("edge")

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            activate();
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            if (currentGame && !onArriving) {
                Sound.play("ok");
                shell.push("pages/SoftwareOptionsPage.qml", {
                    gameId: currentGame.id
                });
            } else {
                Sound.play("edge");
            }
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            if (onPlaying) {
                Sound.play("ok");
                shell.closeSoftware(currentGame);
            } else {
                Sound.play("edge");
            }
        }
    }

    Marquee {
        id: title

        readonly property real centre: page.index * page.pitch - row.contentX + page.tile / 2
        readonly property real margin: Theme.dp(32)
        readonly property real room: 2 * Math.min(centre - margin, page.width - margin - centre)

        x: centre - width / 2
        y: Theme.dp(Theme.tileRowY) - Theme.dp(66)
        maxWidth: Math.max(0, Math.min(page.pitch * 2.5, room))
        visible: page.activeFocus && (page.currentGame !== null || page.onAll || page.onSetup)
        text: page.onSetup ? "Set up" : page.onAll ? (page.empty ? "Add a game" : "View More") : (page.currentGame ? page.currentGame.title : "")
        color: Theme.accent
        font.pixelSize: Theme.dp(33)
    }

    ListView {
        id: row

        y: Theme.dp(Theme.tileRowY) - Theme.dp(30)
        width: parent.width
        height: page.tile + Theme.dp(60)
        orientation: ListView.Horizontal
        model: shown
        spacing: page.gap
        leftMargin: page.rowX
        rightMargin: page.rowX
        interactive: false
        keyNavigationEnabled: false
        highlightFollowsCurrentItem: false
        cacheBuffer: page.pitch * 4
        clip: false

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
            id: rowEase
            Ease {
                duration: Theme.durFocus
            }
        }

        delegate: Item {
            id: cell

            readonly property bool focused: page.activeFocus && index === page.index
            readonly property Item tileArt: art

            width: page.tile
            height: row.height
            // The ring reaches over the neighbours, which are later siblings.
            z: focused ? 2 : 1

            Tile {
                id: art
                y: Theme.dp(30)
                width: page.tile
                height: page.tile
                game: modelData
                focused: cell.focused && !(page.shell && page.shell.launching)
                installing: modelData.installing
                progress: modelData.installing ? modelData.progress : -1

                Touch {
                    current: cell.focused
                    menu: !modelData.installing
                    onPicked: page.point(index)
                }
            }

            Item {
                x: art.x
                y: art.y
                width: page.tile
                height: page.tile
                visible: modelData.id === page.playingId && !modelData.installing
                z: 3

                Rectangle {
                    id: playing

                    x: Theme.dp(113)
                    y: parent.height - Theme.dp(24) - height
                    width: Theme.dp(248)
                    height: Theme.dp(73)
                    radius: height / 2
                    color: "#111111"
                    border.width: Theme.dp(3)
                    border.color: "#ffffff"

                    Label {
                        anchors.centerIn: parent
                        text: "Playing"
                        color: "#ffffff"
                        font.pixelSize: Theme.dp(34)
                    }
                }

                Rectangle {
                    x: Theme.dp(26)
                    anchors.verticalCenter: playing.verticalCenter
                    width: Theme.dp(80)
                    height: width
                    radius: width / 2
                    color: Theme.disc
                    border.width: Theme.dp(3)
                    border.color: "#ffffff"

                    Image {
                        anchors.fill: parent
                        anchors.margins: Theme.dp(12)
                        source: Qt.resolvedUrl("../../../../icons/hicolor/scalable/apps/universe-ui.svg")
                        fillMode: Image.PreserveAspectFit
                        sourceSize.width: 128
                        sourceSize.height: 128
                        smooth: true
                        mipmap: true
                    }
                }
            }
        }

        footer: Item {
            width: (page.tile + page.gap) * page.discs.length
            height: row.height
            z: page.activeFocus && page.index >= page.allIndex ? 2 : 1

            Repeater {
                model: page.discs

                Item {
                    readonly property bool focused: page.activeFocus && page.index === page.allIndex + index

                    x: page.gap + index * page.pitch + (page.tile - width) / 2
                    y: Theme.dp(30) + (page.tile - height) / 2
                    width: Theme.dp(248)
                    height: width

                    Rectangle {
                        id: disc
                        anchors.fill: parent
                        radius: width / 2
                        color: Theme.disc
                    }

                    FocusOutline {
                        target: disc
                        cornerRadius: disc.radius
                        shown: parent.focused
                    }

                    Glyph {
                        anchors.centerIn: parent
                        width: Theme.dp(96)
                        height: width
                        kind: modelData
                        tint: Theme.barGrey
                        stroke: 1.6
                    }

                    Touch {
                        current: parent.focused
                        onPicked: page.point(page.allIndex + index)
                    }
                }
            }
        }
    }

    Swipe {
        flickable: row
        horizontal: true
        ease: rowEase
    }

    Label {
        anchors.horizontalCenter: parent.horizontalCenter
        y: Theme.dp(Theme.tileRowY) + page.tile + Theme.dp(20)
        width: parent.width - Theme.dp(400)
        visible: page.empty
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: "Nothing in the library yet. Add a file on this machine, a store's games, or your Lutris library."
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontSmall)
        lineHeight: 1.3
    }
}
