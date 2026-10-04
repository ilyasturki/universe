import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Details.js" as Details
import "Forms.js" as Forms

FocusScope {
    id: page

    objectName: "gameSettingsPage"
    property var shell: null
    property var args: ({})
    signal closeRequested
    focus: true

    readonly property var form: api.screens.gameSettings
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null

    // The form's cards as sections: the game's own, then the modules'; Advanced (Y) only adds rows inside them,
    // each folded card under a heading of its own.
    readonly property var groups: form.groups
    readonly property var sections: groups.map(function (g) {
        return {
            label: g.title,
            detail: g.meta || "",
            group: g.caps === true ? 0 : 1,
            changed: g.changed === true
        };
    })
    property int section: 0
    property string zone: "list"
    property string landKey: ""
    property string landModule: ""
    readonly property var currentRow: rows.currentRow

    readonly property var hints: {
        var row = rows.currentRow;
        var label = zone !== "rows" ? "OK" : !row || row.heading || row.disabled ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? "Select" : "Change";
        var out = [];
        if (form.hasAdvanced)
            out.push({
                glyph: "Y",
                label: form.showAdvanced ? "Hide advanced" : "Show advanced"
            });
        if (zone === "rows" && row && !row.heading)
            out.push({
                glyph: "X",
                label: row.entry ? "Remove" : "Reset",
                dim: !form.resettable(row)
            }, {
                glyph: "Start",
                label: "Options",
                dim: optionItems().length === 0
            });
        return out.concat([
            {
                glyph: "B",
                label: "Back"
            },
            {
                glyph: "A",
                label: label
            }
        ]);
    }

    onArgsChanged: {
        landKey = args && args.key ? args.key : "";
        landModule = args && args.settingModule ? args.settingModule : "";
        if (args && args.gameId)
            form.load(args.gameId);
        Qt.callLater(landNow);
    }

    // A search hit: the section holding the row, the cursor on it.
    function landNow() {
        if (landKey === "")
            return;
        var i = form.reveal(landKey, landModule);
        if (i < 0)
            return;
        landKey = "";
        var k = form.groups.findIndex(function (g) {
            return g.rows.indexOf(i) >= 0;
        });
        section = k >= 0 ? k : 0;
        list.index = section;
        zone = "rows";
        rows.forceActiveFocus();
        Qt.callLater(function () {
            var at = Forms.rowOf(content, i);
            if (at >= 0)
                rows.index = at;
        });
    }

    function row(i) {
        var src = form.rows[i], r = Details.withDetail(src, src.module);
        r.form = i;
        return r;
    }

    readonly property var content: {
        var g = groups[section];
        if (!g)
            return [];
        return Forms.grouped([Object.assign({}, g, {
                title: ""
            })], form.rows, function (src, i) {
            return page.row(i);
        });
    }

    // X: a value of the game's own goes back to the global's or the default; a variable the game set goes out of its map.
    function resetRow() {
        var row = rows.currentRow;
        if (zone !== "rows" || !row || row.heading || !form.resettable(row)) {
            Sound.play("edge");
            return;
        }
        Sound.play(form.reset(row.form) ? "select" : "edge");
    }

    function activate(index, row) {
        if (row.type === "bool") {
            Sound.play(row.value ? "deselect" : "select");
            form.toggle(row.form);
        } else if (row.map === true) {
            Sound.play("ok");
            Forms.addEntry(shell, row, function (name, value) {
                form.setMapEntry(row.form, name, value);
            }, Forms.allGames(row, function (name, value) {
                form.setMapEntryAll(row.form, name, value);
            }));
        } else {
            rows.edit(row, function (value) {
                form.setValue(row.form, value);
            }, Forms.allGames(row, function (value) {
                form.setValueAll(row.form, value);
            }));
        }
    }

    // +: what X and Y do, a switch flipped for all the games it reaches, the game's own value made every game's.
    function optionItems() {
        var row = zone === "rows" && rows.currentRow !== null && !rows.currentRow.heading ? rows.currentRow : null;
        var all = Forms.allGames(row, null);
        var items = [];
        if (row && form.resettable(row))
            items.push({
                label: row.entry ? "Remove" : "Reset",
                act: "reset"
            });
        if (all && row.type === "bool")
            items.push({
                label: (row.value ? "Turn off for " : "Turn on for ") + all.label.charAt(0).toLowerCase() + all.label.slice(1),
                act: "flipAll"
            });
        if (all && form.promotable(row))
            items.push({
                label: "Apply to " + all.label.charAt(0).toLowerCase() + all.label.slice(1),
                act: "promote"
            });
        if (form.hasAdvanced)
            items.push({
                label: form.showAdvanced ? "Hide advanced" : "Show advanced",
                act: "advanced"
            });
        return items;
    }

    function options() {
        var items = optionItems();
        if (items.length === 0) {
            Sound.play("edge");
            return;
        }
        var row = rows.currentRow;
        Sound.play("ok");
        shell.menu(game ? game.title : "Game Settings", items, function (act) {
            if (act === "reset")
                page.resetRow();
            else if (act === "flipAll")
                Sound.play(form.toggleAll(row.form) ? "select" : "edge");
            else if (act === "promote")
                Sound.play(form.promote(row.form) ? "select" : "edge");
            else if (act === "advanced")
                form.showAdvanced = !form.showAdvanced;
        });
    }

    onSectionChanged: Qt.callLater(rows.reset)

    Connections {
        target: page.form
        ignoreUnknownSignals: true
        function onMessage(text) {
            page.shell.showToast(text);
        }
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isCancel(event) && page.zone === "rows") {
            event.accepted = true;
            Sound.play("back");
            page.zone = "list";
            list.forceActiveFocus();
        } else if (api.keys.isFilters(event) && form.hasAdvanced) {
            event.accepted = true;
            Sound.play("select");
            form.showAdvanced = !form.showAdvanced;
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            page.resetRow();
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            page.options();
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
        title: page.game ? page.game.title : ""
        subtitle: "Game Settings"
    }

    SectionList {
        id: list

        x: Theme.dp(130)
        y: header.height + Theme.dp(20)
        width: Theme.dp(470)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        sections: page.sections
        focus: page.zone === "list"

        onActivated: function (i) {
            page.section = i;
        }
        onPointed: page.zone = "list"
        onEscapedRight: {
            page.zone = "rows";
            rows.forceActiveFocus();
        }
    }

    Rectangle {
        x: Theme.dp(639)
        y: header.height + Theme.dp(20)
        width: 1
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        color: Theme.hairline
    }

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(705)
        y: header.height + Theme.dp(64)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        model: page.content
        focus: page.zone === "rows"

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedDown: Sound.play("edge")
        onPointed: page.zone = "rows"
        onEscapedLeft: {
            page.zone = "list";
            list.forceActiveFocus();
        }
    }
}
