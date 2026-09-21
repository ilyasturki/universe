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

    // The form's cards as sections: the game's own, the modules', then — while Advanced is on (Y) — the power user's;
    // a basic section's own advanced rows come under an Advanced heading of their own.
    readonly property var groups: form.groups
    readonly property var sections: groups.map(function (g) {
        return {
            label: g.title,
            detail: g.meta || "",
            group: g.advanced ? 2 : g.caps === true ? 0 : 1,
            groupLabel: g.advanced ? "Advanced" : ""
        };
    })
    property int section: 0
    property string zone: "list"
    property string landKey: ""
    property string landModule: ""

    readonly property var hints: {
        var row = rows.currentRow;
        var label = zone !== "rows" ? "OK" : !row || row.heading || row.disabled ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? "Select" : "Change";
        var out = [
            {
                glyph: "Y",
                label: form.showAdvanced ? "Hide advanced" : "Show advanced"
            }
        ];
        if (zone === "rows" && row && row.origin)
            out.push({
                glyph: "X",
                label: row.origin === "game" ? "Reset" : "Override"
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

    // Advanced turned off while on one of its sections: the last one that stays.
    onGroupsChanged: if (section >= groups.length) {
        section = Math.max(0, groups.length - 1);
        list.index = section;
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

    // An inherited value names where it comes from ahead of its detail, where the eye lands first: `Global · …`, `Default · …`.
    function row(i) {
        var src = form.rows[i], r = Details.withDetail(src, src.module);
        r.form = i;
        var from = src.origin === "global" ? "Global" : src.origin === "default" ? "Default" : "";
        if (from)
            r.detail = from + (r.detail ? " · " + r.detail : "");
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

    // X: a value of the game's own goes back to the global's or the default; an inherited one is written on the game.
    function resetOrOverride() {
        var row = rows.currentRow;
        if (zone !== "rows" || !row || !row.origin) {
            Sound.play("edge");
            return;
        }
        var ok = row.origin === "game" ? form.reset(row.form) : form.override(row.form);
        Sound.play(ok ? "select" : "edge");
    }

    function activate(index, row) {
        if (row.type === "bool") {
            form.toggle(row.form);
            Sound.play("select");
        } else if (row.type === "map") {
            Sound.play("ok");
            Forms.editMap(shell, row, function (name, value) {
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
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            Sound.play("select");
            form.showAdvanced = !form.showAdvanced;
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            page.resetOrOverride();
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
