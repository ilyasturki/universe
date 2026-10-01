import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../ui" as Base
import "Home.js" as Home

// The console's Trophies for one game: the tally at the top, the list at the left, the lit one told in full at the right.
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
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
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

    // The console's grades of rarity, by the share of players who earned it.
    function rarityName(r) {
        return r < 0 ? "" : r < 5 ? "Ultra rare" : r < 15 ? "Very rare" : r < 50 ? "Rare" : "Common";
    }

    function step(d) {
        var next = Math.max(0, Math.min(rows.length - 1, index + d));
        if (rows.length === 0 || next === index) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        index = next;
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
            step(screen * 5);
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
            Sound.play("edge");
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
        trailing: page.store.loading ? "Asking the store…" : page.store.fetchedText !== "" ? "From the store on " + page.store.fetchedText : ""
    }

    Row {
        id: tally

        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(6)
        spacing: Theme.dp(64)
        visible: page.store.total > 0

        Column {
            spacing: Theme.dp(2)

            Label {
                text: "Progress"
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Label {
                text: page.progress + "%"
                font.weight: Font.Light
                font.pixelSize: Theme.dp(44)
            }
        }

        Column {
            spacing: Theme.dp(2)

            Label {
                text: "Earned"
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Label {
                text: page.store.unlocked + "/" + page.store.total
                font.weight: Font.Light
                font.pixelSize: Theme.dp(44)
            }
        }
    }

    Rectangle {
        x: tally.x
        y: tally.y + tally.height + Theme.dp(16)
        width: list.width
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
        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(150)
        width: list.width
        visible: page.game !== null && page.rows.length > 0
        text: page.game ? page.game.title.toUpperCase() : ""
        color: Theme.textSecondary
        elide: Text.ElideRight
        font.letterSpacing: Theme.dp(1)
        font.pixelSize: Theme.dp(Theme.fontSmall)
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

        x: Theme.dp(Theme.edge) - room
        y: header.height + Theme.dp(196) - room
        width: Math.min(Theme.dp(760), (parent.width - Theme.dp(Theme.edge + Theme.columnRight)) * 0.5) + room * 2
        height: parent.height - y - Theme.dp(40)
        model: page.rows
        currentIndex: page.index
        interactive: false
        clip: true
        spacing: Theme.dp(16)
        highlightFollowsCurrentItem: true
        highlightMoveDuration: Theme.durScroll
        preferredHighlightBegin: room
        preferredHighlightEnd: height - room
        highlightRangeMode: ListView.ApplyRange
        header: Item {
            height: list.room
        }
        footer: Item {
            height: list.room
        }

        delegate: Item {
            width: list.width
            height: Theme.dp(132)

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

    Item {
        id: detail

        readonly property var trophy: page.current

        x: list.x + list.width + Theme.dp(56)
        y: header.height + Theme.dp(196)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: parent.height - y - Theme.dp(40)
        visible: trophy !== null

        Rectangle {
            anchors.fill: parent
            anchors.margins: -Theme.dp(24)
            radius: Theme.dp(Theme.radiusCard + 2)
            color: Qt.rgba(0.07, 0.075, 0.1, 0.72)
            border.width: 1
            border.color: Theme.glassEdge
        }

        Row {
            id: lead
            width: parent.width
            spacing: Theme.dp(28)

            Base.AchievementBadge {
                width: Theme.dp(128)
                height: width
                icon: detail.trophy ? detail.trophy.icon : ""
                unlocked: detail.trophy ? detail.trophy.unlocked : false
                checked: detail.trophy ? detail.trophy.unlocked : false
                tint: Theme.text
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - Theme.dp(128 + 28)
                text: detail.trophy ? detail.trophy.name : ""
                wrapMode: Text.WordWrap
                maximumLineCount: 3
                elide: Text.ElideRight
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(36)
            }
        }

        Label {
            id: detailsCaption
            y: lead.height + Theme.dp(34)
            text: "Details"
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Column {
            anchors.top: detailsCaption.bottom
            anchors.topMargin: Theme.dp(10)
            width: parent.width

            Repeater {
                model: {
                    var t = detail.trophy;
                    if (!t)
                        return [];
                    var out = [
                        {
                            glyph: "info",
                            value: t.description || "No description.",
                            caption: "Description"
                        },
                        {
                            glyph: t.unlocked ? "trophy" : "lock",
                            value: t.unlocked ? "Earned " + t.dateText : t.hidden ? "Hidden until earned" : "Not earned yet",
                            caption: "Status"
                        }
                    ];
                    if (t.rarity >= 0)
                        out.push({
                            glyph: "pulse",
                            value: page.rarityName(t.rarity) + "  |  " + t.rarityText + " earned",
                            caption: "Rarity"
                        });
                    out.push({
                        glyph: "",
                        value: page.game ? page.game.title : "",
                        caption: "Trophy set"
                    });
                    return out;
                }

                Item {
                    width: parent.width
                    height: Math.max(Theme.dp(112), texts.height + Theme.dp(36))

                    Rectangle {
                        x: Theme.dp(72)
                        anchors.bottom: parent.bottom
                        width: parent.width - x
                        height: 1
                        color: Theme.hairline
                        visible: index < 3
                    }

                    Glyph {
                        x: Theme.dp(4)
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(40)
                        height: width
                        visible: modelData.glyph !== ""
                        kind: modelData.glyph
                    }

                    TileArt {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(48)
                        height: width
                        radius: Theme.dp(6)
                        visible: modelData.glyph === ""
                        game: page.game
                    }

                    Column {
                        id: texts
                        x: Theme.dp(72)
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x
                        spacing: Theme.dp(6)

                        Label {
                            width: parent.width
                            text: modelData.value
                            wrapMode: Text.WordWrap
                            maximumLineCount: 3
                            elide: Text.ElideRight
                            lineHeight: 1.15
                            font.pixelSize: Theme.dp(29)
                        }

                        Label {
                            text: modelData.caption
                            color: Theme.textMuted
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }
                    }
                }
            }
        }
    }
}
