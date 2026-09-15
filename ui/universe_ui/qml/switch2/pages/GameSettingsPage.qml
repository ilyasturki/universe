import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Details.js" as Details

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    signal closeRequested()
    focus: true

    readonly property var form: api.screens.gameSettings
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null

    readonly property var sections: form.groups.map(function(g) {
        return { label: g.title, detail: g.meta || "", group: g.caps === true ? 0 : 1 };
    })
    property int section: 0
    property string zone: "list"

    readonly property var hints: {
        var row = rows.currentRow;
        var label = zone !== "rows" ? "OK" : !row || row.heading || row.disabled ? "OK"
                  : row.type === "bool" ? "Toggle" : row.type === "action" ? "Select" : "Change";
        return [ { glyph: "B", label: "Back" }, { glyph: "A", label: label } ];
    }

    onArgsChanged: {
        if (args && args.gameId)
            form.load(args.gameId);
    }

    readonly property var content: {
        var g = form.groups[section];
        if (!g)
            return [];
        var out = g.rows.map(function(i) {
            var src = form.rows[i], r = Details.withDetail(src, src.module);
            r.form = i;
            if (src.inherited === true && !src.detail)
                r.detail += (r.detail ? " " : "") + "Inherited from the global setting.";
            return r;
        });
        if (g.title === "Artwork")
            out.push({ label: "Refresh artwork", type: "action", action: "refresh", display: "",
                       detail: "Fetch the box, the tile, the background and the logo from SteamGridDB, the description from RAWG." });
        return out;
    }

    function activate(index, row) {
        if (row.action === "refresh") {
            Sound.play("ok");
            api.universe.mediaRefresh(args.gameId, false);
            shell.showToast("Fetching artwork for " + (game ? game.title : args.gameId) + "…");
        } else if (row.type === "bool") {
            form.toggle(row.form);
            Sound.play("select");
        } else {
            rows.edit(row, function(value) { form.setValue(row.form, value); });
        }
    }

    onSectionChanged: Qt.callLater(rows.reset)

    Keys.onPressed: function(event) {
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

        onActivated: function(i) { page.section = i; }
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

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedLeft: {
            page.zone = "list";
            list.forceActiveFocus();
        }
    }
}
