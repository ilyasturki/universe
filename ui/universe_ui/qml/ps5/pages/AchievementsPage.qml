import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Home.js" as Home
import "../ui/Trophy.js" as Trophy

// The console's Trophies for one game: the tally at the top, then one wide card a trophy.
FocusScope {
    id: page

    objectName: "achievements"
    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var store: api.screens.achievements
    readonly property var hints: []
    readonly property bool strip: false

    signal closeRequested

    focus: true

    // "default" (the store's: unlocked newest first, then locked most common first), "rare", "common", "name".
    property string order: "default"
    property int index: 0

    readonly property var rows: {
        var out = store.rows.filter(function (r) {
            return !r.hidden;
        });
        var folded = store.rows.filter(function (r) {
            return r.hidden;
        });
        if (order === "rare" || order === "common") {
            var sign = order === "rare" ? 1 : -1;
            out.sort(function (a, b) {
                var x = a.rarity < 0 ? (sign > 0 ? 101 : -1) : a.rarity;
                var y = b.rarity < 0 ? (sign > 0 ? 101 : -1) : b.rarity;
                return sign * (x - y);
            });
        } else if (order === "name") {
            out.sort(function (a, b) {
                return a.name.toLowerCase() < b.name.toLowerCase() ? -1 : 1;
            });
        }
        return out.concat(folded);
    }
    readonly property int progress: store.total > 0 ? Math.round(100 * store.unlocked / store.total) : 0

    readonly property var orders: [
        {
            id: "default",
            label: "Default Order"
        },
        {
            id: "rare",
            label: "Rarest First"
        },
        {
            id: "common",
            label: "Most Common First"
        },
        {
            id: "name",
            label: "Name (A–Z)"
        }
    ]

    function step(d) {
        var next = Math.max(0, Math.min(rows.length - 1, index + d));
        if (rows.length === 0 || next === index) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        index = next;
    }

    function openTrophy(t) {
        if (!t || t.hidden) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        var when = Trophy.earned(t.unlockedAt, t.dateText);
        var earned = !t.unlocked ? "Not earned yet" : when === "" ? "Earned" : "Earned " + when.charAt(0).toLowerCase() + when.slice(1);
        shell.dialogAsk({
            message: t.name,
            detail: [t.description, [earned, Trophy.rarity(t.rarity)].filter(Boolean).join("  ·  ")].filter(Boolean).join("\n\n"),
            buttons: ["OK"]
        });
    }

    function refresh() {
        if (store.loading) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        store.refresh();
    }

    function options() {
        Sound.play("ok");
        var items = orders.map(function (o) {
            return {
                label: o.label,
                check: o.id === page.order
            };
        }).concat([
            {
                label: "Ask the Store Again",
                detail: store.fetchedText !== "" ? "Updated " + store.fetchedText : "",
                glyph: "refresh",
                gap: true
            }
        ]);
        shell.showMenu({
            title: "Sort by",
            items: items,
            index: Math.max(0, orders.findIndex(function (o) {
                return o.id === page.order;
            }))
        }, function (i) {
            if (i < 0)
                return;
            if (i < page.orders.length) {
                page.order = page.orders[i].id;
                page.index = 0;
            } else {
                page.refresh();
            }
        });
    }

    Component.onDestruction: store.unload()

    onArgsChanged: {
        index = 0;
        if (args && args.gameId)
            store.load(args.gameId);
    }

    onRowsChanged: {
        if (index >= rows.length)
            index = Math.max(0, rows.length - 1);
    }

    Keys.onPressed: function (event) {
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (vertical) {
            event.accepted = true;
            step(event.key === Qt.Key_Up ? -1 : 1);
            return;
        }
        if (screen) {
            event.accepted = true;
            step(screen * Math.max(1, Math.floor(list.height / (list.cardHeight + list.spacing)) - 1));
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            event.accepted = true;
            step((api.keys.isFirst(event) ? -1 : 1) * rows.length);
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            refresh();
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            options();
        } else if (api.keys.isAccept(event)) {
            event.accepted = true;
            openTrophy(rows[index]);
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            event.accepted = true;
            Sound.play("edge");
        }
    }

    ArtBackdrop {
        anchors.fill: parent
        target: Home.art(page.game)
        dim: 0.62
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
        title: "Trophies"
        trailing: page.store.loading ? "Asking the store…" : ""
    }

    Label {
        id: tally

        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(6)
        visible: page.store.total > 0
        text: page.store.unlocked + "/" + page.store.total + "  ·  " + page.progress + "%"
        font.weight: Font.Light
        font.pixelSize: Theme.dp(36)
    }

    Rectangle {
        id: bar

        x: tally.x
        y: tally.y + tally.height + Theme.dp(12)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: Theme.dp(6)
        radius: height / 2
        visible: tally.visible
        color: Qt.rgba(1, 1, 1, 0.16)

        Rectangle {
            width: parent.width * page.progress / 100
            height: parent.height
            radius: parent.radius
            color: Theme.trophyGold

            Behavior on width {
                NumberAnimation {
                    duration: Theme.durPage
                    easing.type: Easing.OutCubic
                }
            }
        }
    }

    Label {
        anchors.centerIn: parent
        width: parent.width * 0.6
        visible: page.rows.length === 0
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: page.store.loading ? "Asking the store…" : page.store.error !== "" ? page.store.error : "This game lists no trophies."
        color: page.store.error !== "" ? Theme.danger : Theme.textMuted
    }

    ListView {
        id: list

        readonly property real room: Theme.dp(Theme.ringGap + Theme.ringLine + 8)
        readonly property real cardHeight: Theme.dp(100)

        x: Theme.dp(Theme.edge) - room
        y: bar.y + bar.height + Theme.dp(28) - room
        width: parent.width - Theme.dp(Theme.edge + Theme.columnRight) + room * 2
        height: parent.height - y - Theme.dp(40)
        model: page.rows
        currentIndex: page.index
        interactive: false
        clip: true
        spacing: Theme.dp(12)
        highlightFollowsCurrentItem: false
        onCurrentIndexChanged: Theme.reveal(list, currentIndex * (cardHeight + spacing) - room, currentIndex * (cardHeight + spacing) + cardHeight + room, height)
        header: Item {
            height: list.room
        }
        footer: Item {
            height: list.room
        }

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        delegate: Item {
            width: list.width
            height: list.cardHeight

            TrophyCard {
                x: list.room
                width: parent.width - list.room * 2
                height: parent.height
                trophy: modelData
                focused: page.activeFocus && index === page.index
                onPicked: {
                    Sound.play("tick");
                    page.index = index;
                    page.forceActiveFocus();
                }
            }
        }
    }

    Swipe {
        flickable: list
        ease: scrollEase
    }

    Scrollbar {
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(4)
        anchors.top: list.top
        anchors.topMargin: list.room
        anchors.bottom: list.bottom
        anchors.bottomMargin: list.room
        flickable: list
    }
}
