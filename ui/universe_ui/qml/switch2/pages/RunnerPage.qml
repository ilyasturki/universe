import QtQuick
import "../core"
import "../sound"
import "../ui"

// One runner: its program and arguments, its options, a game added through it.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    signal closeRequested()
    focus: true

    readonly property var form: api.screens.runner
    readonly property var info: form.info
    readonly property bool folderOpen: folder.open

    readonly property var hints: {
        if (folderOpen)
            return folder.hints;
        var row = rows.currentRow;
        var label = !row || row.heading ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? "Select" : "Change";
        return [ { glyph: "B", label: "Back" }, { glyph: "A", label: label } ];
    }

    onArgsChanged: {
        if (args && args.runner)
            form.load(args.runner);
    }

    readonly property var content: {
        var out = [];
        var groups = form.groups, all = form.rows;
        for (var i = 0; i < groups.length; i++) {
            if (groups[i].title)
                out.push({ heading: true, label: groups[i].title, display: "" });
            for (var j = 0; j < groups[i].rows.length; j++) {
                var src = all[groups[i].rows[j]], r = {};
                for (var k in src)
                    r[k] = src[k];
                r.form = groups[i].rows[j];
                r.detail = "";
                out.push(r);
            }
        }
        return out;
    }

    function activate(index, row) {
        if (row.type === "bool") {
            form.toggle(row.form);
            Sound.select();
            return;
        }
        if (row.key === "add_file") {
            Sound.ok();
            folder.show({ title: "Game file for " + info.name, path: "", files: true }, function(path) {
                rows.forceActiveFocus();
                if (path === null || !form.setValue(row.form, path))
                    return;
                shell.prompt({ title: "Title of the game", value: form.pendingTitle() }, function(title) {
                    if (title === null)
                        return;
                    if (form.addGame(title) !== "")
                        Sound.ok();
                    else
                        Sound.edge();
                });
            });
            return;
        }
        if (row.key === "exe") {
            Sound.ok();
            folder.show({ title: row.label, path: row.value, files: true }, function(path) {
                if (path !== null)
                    form.setValue(row.form, path);
                rows.forceActiveFocus();
            });
            return;
        }
        rows.edit(row, function(value) { form.setValue(row.form, value); });
    }

    Connections {
        target: page.form
        function onMessage(text) { page.shell.showToast(text); }
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat || page.folderOpen)
            return;
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            page.closeRequested();
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
        icon: "play"
        title: page.info.name || ""
        subtitle: "Runner"
    }

    Text {
        x: Theme.dp(120)
        y: header.height + Theme.dp(24)
        width: parent.width - x - Theme.dp(120)
        visible: text !== ""
        text: (page.info.meta || "")
              + (page.info.warning
                 ? (page.info.meta ? " · " : "") + "<font color=\"" + Theme.danger + "\">" + page.info.warning + "</font>"
                 : "")
        textFormat: Text.StyledText
        color: Theme.textSecondary
        elide: Text.ElideRight
        font.family: Theme.sans
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    SettingsRows {
        id: rows

        shell: page.shell
        folder: folder
        x: Theme.dp(120)
        y: header.height + Theme.dp(84)
        width: parent.width - x - Theme.dp(120)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        model: page.content
        focus: true

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedLeft: Sound.edge()
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
