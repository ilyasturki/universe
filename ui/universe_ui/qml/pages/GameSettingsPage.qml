import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../ui/Maps.js" as Maps

FocusScope {
    id: page

    objectName: "gameSettingsPage"
    focus: true

    // { game }: one game's settings; `key` (and a module setting's `settingModule`) lands the cursor on that row,
    // Advanced turned on if the row sits behind it.
    property var args: ({})
    readonly property var game: args.game || null
    readonly property var form: api.screens.gameSettings
    property string landKey: ""
    property string landModule: ""

    signal closeRequested
    signal message(string text)

    // One sidebar entry per card: the game's own, then the modules', then — while Advanced is on — the power user's.
    readonly property var groups: form.groups
    readonly property var sections: groups.map(function (g) {
        var first = form.rows[g.rows[0]] || {};
        return {
            name: g.title,
            icon: first.module ? "grid" : page.icons[g.title] || "play",
            group: g.advanced ? "Advanced" : first.module ? "Modules" : ""
        };
    })
    readonly property var icons: ({
            "Display": "screen",
            "Overlay": "gauge",
            "Launch": "sliders",
            "Desktop and library": "heart-outline",
            "Scaling": "image",
            "Environment": "terminal",
            "Sync": "refresh",
            "Upscaling": "bolt",
            "Logs": "book",
            "Artwork": "image"
        })
    property int section: 0
    readonly property var shown: section < groups.length ? [groups[section]] : []

    // Which side has the cursor: the focus follows it, so a page-wide focus call lands on the right one.
    property string zone: "side"
    readonly property var row: cards.currentRow
    readonly property bool onRows: zone === "rows"
    readonly property string originAction: row && row.origin === "game" ? "Reset" : "Override"

    readonly property var hints: editor.open ? editor.hints : menu.open ? menu.hints : [
        {
            glyph: "A",
            label: !onRows ? "Open" : row && row.type === "bool" ? "Toggle" : row && row.type === "action" ? row.action || "Select" : "Change",
            dim: onRows && (!row || row.disabled === true || row.type === "info")
        }
    ].concat(onRows ? [
        {
            glyph: "X",
            label: originAction,
            dim: !row || !row.origin
        }
    ] : []).concat([
        {
            glyph: "Y",
            label: form.showAdvanced ? "Hide advanced" : "Show advanced"
        },
        {
            glyph: "B",
            label: onRows ? "Sections" : "Back"
        },
        {
            glyph: "LT RT",
            label: "Section"
        }
    ])

    readonly property real sideMargin: Theme.dp(90)
    readonly property real sideWidth: Theme.dp(300)
    readonly property real mainRoom: width - sideMargin * 2 - sideWidth - Theme.dp(56)
    readonly property real mainWidth: Math.min(mainRoom, Theme.dp(1280))
    readonly property real mainX: sideMargin + sideWidth + Theme.dp(56) + (mainRoom - mainWidth) / 2

    // The derived game is still stale here: read the args themselves. The rows come at once; the cursor settles once
    // the page is laid out (one call however often the args change): a search hit lands, anything else starts on the sidebar.
    onArgsChanged: {
        if (!args.game)
            return;
        landKey = args.key || "";
        landModule = args.settingModule || "";
        section = 0;
        zone = "side";
        form.load(args.game.id);
        Qt.callLater(page.settle);
    }

    function settle() {
        if (!landNow())
            cards.reset();
    }

    // Advanced turned off while on one of its cards: the last card that stays.
    onGroupsChanged: if (section >= groups.length)
        section = Math.max(0, groups.length - 1)

    // A search hit: the card holding the row, the cursor on it.
    function landNow() {
        if (landKey === "")
            return false;
        var i = form.reveal(landKey, landModule);
        landKey = "";
        if (i < 0)
            return false;
        var k = form.groups.findIndex(function (g) {
            return g.rows.indexOf(i) >= 0;
        });
        section = k >= 0 ? k : 0;
        cards.index = i;
        zone = "rows";
        return true;
    }

    function stepSection(d) {
        var n = sections.length;
        if (n === 0)
            return;
        section = (section + d + n) % n;
        cards.reset();
        Sound.tick();
    }

    function toggleAdvanced() {
        Sound.panel();
        form.showAdvanced = !form.showAdvanced;
    }

    // X: a value of the game's own goes back to the global's or the default; an inherited one is written on the game.
    function resetOrOverride() {
        var r = cards.currentRow;
        if (!onRows || !r || !r.origin) {
            Sound.edge();
            return;
        }
        var ok = r.origin === "game" ? form.reset(cards.index) : form.override(cards.index);
        ok ? Sound.enter() : Sound.edge();
    }

    function editMap(index, row) {
        Sound.panel();
        menu.show(Maps.items(row), cards, cards.focusRect, row.label, function (action) {
            if (action === "add") {
                editor.prompt("Name of " + Maps.noun(row), "", function (name) {
                    name = Maps.cleanName(name);
                    if (name === "")
                        return;
                    editor.prompt("Value of " + name, "", function (value) {
                        form.setMapEntry(index, name, value);
                    });
                });
            } else if (action.indexOf("entry:") === 0) {
                var name = action.substring(6);
                menu.show(Maps.entryItems(name), cards, cards.focusRect, name, function (next) {
                    if (next === "value")
                        editor.prompt("Value of " + name, Maps.valueOf(row, name), function (value) {
                            form.setMapEntry(index, name, value);
                        });
                    else if (next === "remove") {
                        Sound.cancel();
                        form.setMapEntry(index, name, "");
                    }
                    cards.forceActiveFocus();
                });
            }
        });
    }

    function activate(index, row) {
        if (row.disabled === true || row.type === "info") {
            Sound.edge();
        } else if (row.type === "map") {
            editMap(index, row);
        } else if (row.type === "bool") {
            form.toggle(index);
            Sound.favourite(!row.value);
        } else {
            Sound.panel();
            editor.edit(row, function (value) {
                form.setValue(index, value);
            });
        }
    }

    Connections {
        target: page.form
        ignoreUnknownSignals: true
        function onMessage(text) {
            page.message(text);
        }
    }

    GameBackdrop {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
    }

    GameHeader {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        game: page.game
        label: "GAME SETTINGS"
    }

    SectionList {
        id: side

        x: page.sideMargin
        y: header.y + header.height + Theme.dp(32)
        width: page.sideWidth
        focus: page.zone === "side"
        sections: page.sections
        current: page.section
        opacity: editor.open || menu.open ? 0.5 : 1.0

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }

        onRequested: function (i) {
            page.section = i;
            cards.reset();
        }
        onEntered: page.zone = "rows"
        onEscapedUp: Sound.edge()
        onCancelled: page.closeRequested()
    }

    SettingsCards {
        id: cards

        x: page.mainX
        y: side.y
        width: page.mainWidth
        height: hintBar.y - y
        focus: page.zone === "rows"
        columns: 1
        compact: true
        rows: page.form.rows
        groups: page.shown
        dimmed: editor.open || menu.open

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedUp: Sound.edge()
        onEscapedDown: Sound.edge()
        onEscapedLeft: {
            Sound.panel();
            page.zone = "side";
        }

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                Sound.cancel();
                page.zone = "side";
            }
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        z: 3
        sideMargin: page.sideMargin
        hints: page.hints
    }

    ValueEditor {
        id: editor

        anchors.fill: parent
        cards: cards
        overhang: 0
        floor: hintBar.y
        z: 2

        onClosed: cards.forceActiveFocus()
    }

    ActionMenu {
        id: menu

        anchors.fill: parent
        z: 4

        onDismissed: cards.forceActiveFocus()
    }

    Keys.onPressed: function (event) {
        if (api.keys.isPageUp(event) || api.keys.isPageDown(event)) {
            event.accepted = true;
            return;
        }
        if (event.isAutoRepeat || editor.open || menu.open)
            return;
        if (api.keys.isFilters(event)) {
            event.accepted = true;
            page.toggleAdvanced();
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            page.resetOrOverride();
        }
    }

    Keys.onReleased: function (event) {
        if (event.isAutoRepeat || editor.open || menu.open)
            return;
        var d = api.keys.isPageUp(event) ? -1 : api.keys.isPageDown(event) ? 1 : 0;
        if (!d)
            return;
        event.accepted = true;
        page.stepSection(d);
    }
}
