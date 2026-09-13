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
    readonly property bool folderOpen: folder.open

    readonly property var sections: {
        var out = [];
        var groups = form.groups;
        for (var i = 0; i < groups.length; i++)
            out.push({ label: groups[i].title, detail: groups[i].meta || "", group: groups[i].caps === true ? 0 : 1 });
        return out;
    }
    property int section: 0
    property string zone: "list"

    readonly property var hints: {
        if (folderOpen)
            return folder.hints;
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
        var out = [];
        var groups = form.groups, all = form.rows;
        if (section < 0 || section >= groups.length)
            return out;
        var g = groups[section];
        for (var j = 0; j < g.rows.length; j++) {
            var src = all[g.rows[j]], r = Details.withDetail(src, src.module);
            r.form = g.rows[j];
            if (src.inherited === true && !src.detail)
                r.detail += (r.detail ? " " : "") + "Inherited from the global setting.";
            out.push(r);
        }
        if (g.title === "Artwork")
            out.push({ label: "Refresh artwork", type: "action", action: "refresh", display: "",
                       detail: "Fetch the box, the tile, the background and the logo from SteamGridDB, the description from RAWG." });
        return out;
    }

    function activate(index, row) {
        if (row.action === "refresh") {
            Sound.ok();
            api.universe.mediaRefresh(args.gameId, false);
            shell.showToast("Fetching artwork for " + (game ? game.title : args.gameId) + "…");
            return;
        }
        if (row.type === "bool") {
            form.toggle(row.form);
            Sound.select();
            return;
        }
        rows.edit(row, function(value) { form.setValue(row.form, value); });
    }

    onSectionChanged: Qt.callLater(rows.reset)

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat || page.folderOpen)
            return;
        if (api.keys.isCancel(event) && page.zone === "rows") {
            event.accepted = true;
            Sound.back();
            page.zone = "list";
            list.forceActiveFocus();
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
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
        current: page.section
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
        folder: folder
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
        onEscapedUp: Sound.edge()
    }

    FolderPage {
        id: folder
        z: 5
        onTypeRequested: function(path) {
            page.shell.prompt({ title: "Path", value: path, path: true }, function(v) { folder.finish(v); });
        }
    }
}
