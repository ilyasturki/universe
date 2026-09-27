import QtQuick
import Universe
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    property var shell: null
    property int index: 0
    readonly property var hints: []
    readonly property var currentGame: anchor.game

    function reset() {
        index = 0;
    }

    function back() {
        return false;
    }

    RecentGames {
        id: played
        sourceModel: api.allGames
        playingId: page.shell ? page.shell.playingId : ""
    }

    GameAnchor {
        id: anchor
        client: api.universe
        model: played
        index: page.index
        onMoved: function (next) {
            page.index = next;
        }
    }

    Keys.onLeftPressed: index = Sound.stepped(index, -1, played.count)
    Keys.onRightPressed: index = Sound.stepped(index, 1, played.count)
    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event) && currentGame) {
            event.accepted = true;
            currentGame.id === shell.playingId ? shell.resume() : shell.launch(currentGame);
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            Sound.play("ok");
            shell.push("pages/SettingsPage.qml", {});
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    Label {
        x: Theme.dp(Theme.tabX)
        y: Theme.dp(Theme.barY) - height / 2
        text: "Games"
        font.weight: Font.DemiBold
        font.pixelSize: Theme.dp(Theme.fontTab)
    }

    Label {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(87)
        y: Theme.dp(Theme.barY) - height / 2
        text: Theme.clock
        font.weight: Font.Light
        font.pixelSize: Theme.dp(Theme.fontClock)
    }

    Row {
        x: Theme.dp(Theme.tileFocusX)
        y: Theme.dp(Theme.railY)
        spacing: Theme.dp(Theme.tileGap)

        Repeater {
            model: played

            TileArt {
                width: Theme.dp(index === page.index ? Theme.tileFocus : Theme.tileSize)
                height: width
                game: modelData

                FocusFrame {
                    shown: index === page.index && page.activeFocus
                    radius: Theme.dp(Theme.radiusTile)
                }
            }
        }
    }

    Label {
        x: Theme.dp(Theme.edge)
        y: Theme.dp(Theme.heroTagY)
        text: page.currentGame ? page.currentGame.title : "Nothing in the library yet"
        font.weight: Font.Light
        font.pixelSize: Theme.dp(56)
    }
}
