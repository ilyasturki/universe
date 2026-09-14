import QtQuick
import "../core"
import "../sound"

// The game menu a held A opens: a popover beside the focused cover, which stays
// lit above the scrim as a live copy so its ring and badge come with it.
FocusScope {
    id: menu

    property bool open: false
    property var game: null
    property Item anchorItem: null
    property int index: 0

    signal playRequested(var game, Item art)
    signal detailRequested(var game)
    signal favouriteRequested(var game)
    signal settingsRequested(var game)
    signal artworkRequested(var game)
    signal recordingsRequested(var game)
    signal journalRequested(var game)
    signal stopRequested()
    signal closed()

    readonly property var hints: [
        { glyph: "A", label: "Select" },
        { glyph: "B", label: "Close" },
        { glyph: "dpad", label: "Navigate" }
    ]

    readonly property var session: api.universe.currentSession
    readonly property bool sessionRunning: session !== null && session !== undefined && session.session_id !== undefined

    readonly property var items: {
        var out = [];
        if (sessionRunning)
            out.push({ icon: "stop", label: "Stop " + session.title, action: "stop" });
        else
            out.push({ icon: "play", label: game && game.playTime > 0 ? "Continue" : "Play", action: "play" });
        out.push({ icon: "info", label: "Details", action: "details" });
        out.push({ icon: game && game.favorite ? "heart" : "heart-outline",
                   label: game && game.favorite ? "Remove from favourites" : "Add to favourites", action: "favourite" });
        out.push({ icon: "sliders", label: "Game settings", action: "settings" });
        out.push({ icon: "image", label: "Artwork", action: "artwork" });
        out.push({ icon: "film", label: "Recordings", action: "recordings" });
        out.push({ icon: "book", label: "Journal", action: "journal" });
        return out;
    }

    // The anchor's rect, taken once at show(): a hold outlasts any slide the page had going.
    property real ax: 0
    property real ay: 0
    property real aw: 0
    property real ah: 0
    // Covers the ring and halo, and the 5% a grid cover grows by.
    readonly property real copyMargin: Theme.dp(26)
    readonly property real gap: Theme.dp(44)
    readonly property bool onRight: ax + aw + gap + panel.width <= width - Theme.dp(40)
    property real slide: open ? 0.0 : 1.0

    focus: open
    visible: scrim.opacity > 0.01

    Behavior on slide {
        NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
    }

    function show(g, anchor) {
        game = g;
        anchorItem = anchor;
        index = 0;
        var p = anchor.mapToItem(menu, 0, 0);
        ax = p.x;
        ay = p.y;
        aw = anchor.width;
        ah = anchor.height;
        copy.sourceItem = anchor;
        open = true;
        forceActiveFocus();
    }

    function hide() {
        open = false;
        closed();
    }

    function activate() {
        var g = game;
        var art = anchorItem;
        var action = items[index].action;
        hide();
        if (action === "play")
            playRequested(g, art);
        else if (action === "stop")
            stopRequested();
        else if (action === "details")
            detailRequested(g);
        else if (action === "favourite")
            favouriteRequested(g);
        else if (action === "settings")
            settingsRequested(g);
        else if (action === "artwork")
            artworkRequested(g);
        else if (action === "recordings")
            recordingsRequested(g);
        else if (action === "journal")
            journalRequested(g);
    }

    onVisibleChanged: {
        if (!visible)
            copy.sourceItem = null;
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Qt.rgba(0.02, 0.02, 0.03, 1)
        opacity: menu.open ? 0.62 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }
    }

    ShaderEffectSource {
        id: copy
        live: true
        hideSource: false
        x: menu.ax - menu.copyMargin
        y: menu.ay - menu.copyMargin
        width: menu.aw + menu.copyMargin * 2
        height: menu.ah + menu.copyMargin * 2
        sourceRect: Qt.rect(-menu.copyMargin, -menu.copyMargin, width, height)
        opacity: menu.open ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }
    }

    Rectangle {
        id: panel

        width: Theme.dp(440)
        height: rows.height + Theme.dp(24)
        radius: Theme.dp(24)
        color: "#1b1d24"
        border.width: 1
        border.color: Theme.surfaceBorder
        x: (menu.onRight ? menu.ax + menu.aw + menu.gap : menu.ax - menu.gap - width)
           + (menu.onRight ? -1 : 1) * menu.slide * Theme.dp(16)
        y: Math.max(Theme.dp(40), Math.min(menu.ay + menu.ah / 2 - height / 2, menu.height - height - Theme.dp(40)))
        opacity: 1.0 - menu.slide
        scale: 1.0 - menu.slide * 0.04
        transformOrigin: menu.onRight ? Item.Left : Item.Right

        Column {
            id: rows

            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: Theme.dp(12)
            spacing: Theme.dp(4)

            Repeater {
                model: menu.items

                Rectangle {
                    id: row

                    readonly property bool focused: index === menu.index
                    readonly property color ink: focused ? Theme.onLight : Theme.text

                    width: rows.width
                    height: Theme.dp(66)
                    radius: Theme.dp(16)
                    color: focused ? Theme.text : "transparent"

                    Behavior on color {
                        ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                    }

                    MenuGlyph {
                        id: glyph

                        anchors.left: parent.left
                        anchors.leftMargin: Theme.dp(22)
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(26)
                        height: Theme.dp(26)
                        kind: modelData.icon
                        tint: row.ink
                    }

                    Text {
                        anchors.left: glyph.right
                        anchors.leftMargin: Theme.dp(18)
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(20)
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.label
                        color: row.ink
                        font.family: Theme.sans
                        font.weight: row.focused ? Font.DemiBold : Font.Medium
                        font.pixelSize: Theme.dp(25)
                        elide: Text.ElideRight
                    }
                }
            }
        }
    }

    Keys.onPressed: function(event) {
        event.accepted = true;
        // The A that opened the menu is still down; its repeats must not pick an item.
        if (event.isAutoRepeat)
            return;
        if (api.keys.isMenu(event)) {
            Sound.cancel();
            hide();
            return;
        }
        if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            var next = Math.max(0, Math.min(items.length - 1, index + (event.key === Qt.Key_Up ? -1 : 1)));
            next === index ? Sound.edge() : Sound.tick();
            index = next;
            return;
        }
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            Sound.edge();
            return;
        }
        if (api.keys.isAccept(event)) {
            activate();
            return;
        }
        if (api.keys.isCancel(event)) {
            Sound.cancel();
            hide();
            return;
        }
    }
}
