import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../ui" as Base
import "Details.js" as Details
import "Forms.js" as Forms

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    focus: true

    readonly property bool runner: args.runner !== undefined
    readonly property bool source: args.source !== undefined
    readonly property var form: runner ? api.screens.runner : source ? api.screens.source : api.screens.module
    readonly property var info: form.info
    readonly property var login: api.screens.login

    readonly property var hints: {
        var row = rows.currentRow;
        var label = !row || row.heading || row.disabled || row.type === "info" ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? "Select" : "Change";
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

    // `key` lands the cursor on that row, the Advanced row opened if it sits behind it; a source's rows come back from a thread.
    property string landKey: ""

    Component.onDestruction: if (page.runner)
        api.screens.runner.load("")
    onArgsChanged: {
        landKey = args.key || "";
        (args.runner ? api.screens.runner : args.source ? api.screens.source : api.screens.module).load(args.runner || args.source || args.module);
        Qt.callLater(landNow);
    }

    function landNow() {
        if (landKey === "")
            return;
        var i = form.reveal(landKey, "");
        if (i < 0)
            return;
        landKey = "";
        Qt.callLater(function () {
            var at = Forms.rowOf(content, i);
            if (at >= 0)
                rows.index = at;
        });
    }

    readonly property var content: Forms.grouped(form.groups, form.rows, function (src, i) {
        var r = Object.assign({}, runner ? src : Details.withDetail(src, src.module), {
            form: i
        });
        if (runner && src.key === "exe")
            r.detail = "";
        else if (src.key === "game")
            r.icon = src.image, r.iconSlot = true;
        else if (src.key === "enabled")
            r.detail = info.warning ? "Cannot be enabled: " + info.warning.replace(/^unavailable:?\s*/, "") : Details.enabledSentence(info.name, info.source === true);
        else if (src.key === "logged_in") {
            r.display = src.detail;
            r.detail = "";
        } else if (src.key === "link")
            r.display = login.source === args.source && login.url ? "Ready" : "";
        return r;
    })

    function gameMenu(row) {
        var items = [], gameId = row.gameId, title = row.label;
        if (api.allGames.byId(gameId))
            items.push({
                label: "Game Settings",
                act: "settings"
            });
        if (row.installed)
            items.push({
                label: "Uninstall…",
                act: "uninstall"
            });
        items.push({
            label: "Remove from library…",
            act: "remove"
        });
        shell.menu(title, items, function (a) {
            if (a === "settings") {
                Sound.play("ok");
                shell.push("pages/GameSettingsPage.qml", {
                    gameId: gameId
                });
            } else if (a === "uninstall") {
                shell.dialogAsk({
                    message: "Uninstall " + title + "?",
                    detail: "The install folder goes to the trash; the hours and the journal stay.",
                    buttons: ["Cancel", "Uninstall"],
                    danger: 1
                }, function (k) {
                    if (k === 1)
                        form.uninstall(gameId);
                });
            } else if (a === "remove") {
                shell.dialogAsk({
                    message: "Remove " + title + " from the library?",
                    detail: "The entry is archived; the files are left where they are.",
                    buttons: ["Cancel", "Remove"],
                    danger: 1
                }, function (k) {
                    if (k === 1)
                        form.remove(gameId);
                });
            }
        });
    }

    function activate(index, row) {
        if (row.key === "advanced") {
            Sound.play("ok");
            form.showAdvanced = !form.showAdvanced;
            if (form.showAdvanced)
                Qt.callLater(function () {
                    rows.index = Forms.firstAfter(content, Forms.rowOf(content, row.form));
                });
        } else if (row.type === "map") {
            Sound.play("ok");
            Forms.editMap(shell, row, function (name, value) {
                form.setMapEntry(row.form, name, value);
            });
        } else if (row.type === "bool") {
            form.toggle(row.form);
            Sound.play("select");
        } else if (row.key === "link" && source) {
            Sound.play("ok");
            login.begin(args.source);
        } else if (row.key === "code" && source) {
            shell.prompt({
                title: "Code from " + (info.name || args.source),
                value: ""
            }, function (value) {
                if (value !== null && value !== "")
                    login.submit(value);
            });
        } else if (row.key === "game") {
            Sound.play("ok");
            gameMenu(row);
        } else if (row.key === "add_file") {
            Sound.play("ok");
            shell.browse({
                title: "Game file for " + info.name,
                path: "",
                files: true
            }, function (path) {
                if (path === null || !form.setValue(row.form, path))
                    return;
                shell.prompt({
                    title: "Title of the game",
                    value: form.pendingTitle()
                }, function (title) {
                    if (title !== null)
                        form.addGame(title) !== "" ? Sound.play("ok") : Sound.play("edge");
                });
            });
        } else {
            rows.edit(row, function (value) {
                form.setValue(row.form, value);
            });
        }
    }

    Connections {
        target: page.form
        ignoreUnknownSignals: true
        function onMessage(text) {
            page.shell.showToast(text);
        }
        function onRowsChanged() {
            page.landNow();
        }
    }

    Connections {
        target: page.login
        function onFinished(ok, text) {
            page.shell.showToast(text);
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: page.runner ? "play" : "settings"
        title: page.info.name || ""
        subtitle: page.runner ? "Runner" : page.source ? "Source" : "Module"
    }

    Label {
        id: metaLine
        x: Theme.dp(120)
        y: header.height + Theme.dp(24)
        width: parent.width - x - Theme.dp(120)
        visible: text !== ""
        text: (page.info.meta || "") + (page.info.warning ? (page.info.meta ? " · " : "") + "<font color=\"" + Theme.danger + "\">" + page.info.warning + "</font>" : "")
        textFormat: Text.StyledText
        color: Theme.textSecondary
        elide: Text.ElideRight
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    Label {
        id: description
        x: Theme.dp(120)
        y: metaLine.y + (metaLine.visible ? metaLine.height + Theme.dp(8) : 0)
        width: parent.width - x - Theme.dp(120)
        visible: text !== ""
        text: page.info.description || ""
        color: Theme.textSecondary
        wrapMode: Text.WordWrap
        maximumLineCount: 3
        elide: Text.ElideRight
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(120)
        y: header.height + Theme.dp(84) + (description.visible ? description.height + Theme.dp(8) : 0)
        width: parent.width - x - Theme.dp(120)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20) - (qrCard.visible ? qrCard.height + Theme.dp(20) : 0)
        model: page.content
        focus: true

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
    }

    LoginCard {
        id: qrCard

        x: rows.x
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight) + Theme.dp(20)
        width: rows.width
        source: page.source ? page.args.source : ""
    }
}
