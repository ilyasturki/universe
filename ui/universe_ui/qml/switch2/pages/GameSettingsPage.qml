import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Details.js" as Details
import "Forms.js" as Forms

FocusScope {
    id: page

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
            form.toggle(row.form);
            Sound.play("select");
        } else if (row.map === true) {
            Sound.play("ok");
            Forms.addEntry(shell, row, function (name, value) {
                form.setMapEntry(row.form, name, value);
            });
        } else {
            rows.edit(row, function (value) {
                form.setValue(row.form, value);
            });
        }
    }

    onSectionChanged: Qt.callLater(rows.reset)

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
        width: Theme.dp(1023)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        model: page.content
        focus: page.zone === "rows"

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedDown: Sound.play("edge")
        onEscapedLeft: {
            page.zone = "list";
            list.forceActiveFocus();
        }
    }
}
