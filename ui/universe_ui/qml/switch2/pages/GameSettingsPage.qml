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

    // The basic cards as sections; the advanced ones together behind one Advanced entry.
    readonly property var basicGroups: form.basicGroups
    readonly property var sections: basicGroups.map(function (g) {
        return {
            label: g.title,
            detail: g.meta || "",
            group: g.caps === true ? 0 : 1
        };
    }).concat(form.hasAdvanced ? [
        {
            label: "Advanced",
            detail: "Settings for power users",
            group: 2
        }
    ] : [])
    readonly property bool onAdvanced: form.hasAdvanced && section === form.basicGroups.length
    property int section: 0
    property string zone: "list"
    property string landKey: ""
    property string landModule: ""

    readonly property var hints: {
        var row = rows.currentRow;
        var label = zone !== "rows" ? "OK" : !row || row.heading || row.disabled ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? "Select" : "Change";
        return [
            {
                glyph: "B",
                label: "Back"
            },
            {
                glyph: "A",
                label: label
            }
        ];
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
        var basic = form.basicGroups;
        var k = basic.findIndex(function (g) {
            return g.rows.indexOf(i) >= 0;
        });
        section = k >= 0 ? k : basic.length;
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
        if (src.inherited === true)
            r.detail += (r.detail ? " " : "") + "Inherited from the global setting.";
        return r;
    }

    readonly property var content: {
        if (onAdvanced)
            return Forms.grouped(form.advancedGroups, form.rows, function (src, i) {
                return page.row(i);
            });
        var g = form.basicGroups[section];
        return g ? g.rows.map(page.row) : [];
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
        onEscapedLeft: {
            page.zone = "list";
            list.forceActiveFocus();
        }
    }
}
