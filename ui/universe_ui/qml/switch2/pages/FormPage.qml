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
    focus: true

    readonly property bool runner: args.runner !== undefined
    readonly property var form: runner ? api.screens.runner : api.screens.module
    readonly property var info: form.info

    readonly property var hints: {
        var row = rows.currentRow;
        var label = !row || row.heading || row.disabled ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? "Select" : "Change";
        return [ { glyph: "B", label: "Back" }, { glyph: "A", label: label } ];
    }

    onArgsChanged: (args.runner ? api.screens.runner : api.screens.module).load(args.runner || args.module)

    readonly property var content: Forms.grouped(form.groups, form.rows, function(src, i) {
        var r = Object.assign({}, runner ? src : Details.withDetail(src, src.module), { form: i });
        if (runner)
            r.detail = "";
        else if (src.key === "enabled")
            r.detail = info.warning ? "Cannot be enabled: " + info.warning.replace(/^unavailable:?\s*/, "") : Details.enabledSentence(info.name, info.kind ? info.kind.join(" · ") : "");
        return r;
    })

    function activate(index, row) {
        if (row.type === "bool") {
            form.toggle(row.form);
            Sound.play("select");
        } else if (row.key === "add_file") {
            Sound.play("ok");
            shell.browse({ title: "Game file for " + info.name, path: "", files: true }, function(path) {
                if (path === null || !form.setValue(row.form, path))
                    return;
                shell.prompt({ title: "Title of the game", value: form.pendingTitle() }, function(title) {
                    if (title !== null)
                        form.addGame(title) !== "" ? Sound.play("ok") : Sound.play("edge");
                });
            });
        } else {
            rows.edit(row, function(value) { form.setValue(row.form, value); });
        }
    }

    Connections {
        target: page.form
        ignoreUnknownSignals: true
        function onMessage(text) { page.shell.showToast(text); }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: page.runner ? "play" : "settings"
        title: page.info.name || ""
        subtitle: page.runner ? "Runner" : "Module"
    }

    Label {
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
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(120)
        y: header.height + Theme.dp(84)
        width: parent.width - x - Theme.dp(120)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        model: page.content
        focus: true

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedLeft: Sound.play("edge")
    }
}
