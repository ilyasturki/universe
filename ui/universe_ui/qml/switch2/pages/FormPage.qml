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
        return [ { glyph: "B", label: "Back" }, { glyph: "A", label: label } ];
    }

    Component.onDestruction: if (page.runner) api.screens.runner.load("")
    onArgsChanged: (args.runner ? api.screens.runner : args.source ? api.screens.source : api.screens.module).load(args.runner || args.source || args.module)

    readonly property var content: Forms.grouped(form.groups, form.rows, function(src, i) {
        var r = Object.assign({}, runner ? src : Details.withDetail(src, src.module), { form: i });
        if (runner && src.key === "exe")
            r.detail = "";
        else if (src.key === "game")
            r.icon = src.image, r.iconSlot = true;
        else if (src.key === "enabled")
            r.detail = info.warning ? "Cannot be enabled: " + info.warning.replace(/^unavailable:?\s*/, "") : Details.enabledSentence(info.name, info.source === true);
        else if (src.key === "logged_in") {
            r.display = src.detail;
            r.detail = "";
        }
        else if (src.key === "link")
            r.display = login.source === args.source && login.url ? "Ready" : "";
        return r;
    })

    function gameMenu(row) {
        var items = [], gameId = row.gameId, title = row.label;
        if (api.allGames.byId(gameId))
            items.push({ label: "Game Settings", act: "settings" });
        if (row.installed)
            items.push({ label: "Uninstall…", act: "uninstall" });
        items.push({ label: "Remove from library…", act: "remove" });
        shell.menu(title, items, function(a) {
            if (a === "settings") {
                Sound.play("ok");
                shell.push("pages/GameSettingsPage.qml", { gameId: gameId });
            } else if (a === "uninstall") {
                shell.dialogAsk({ message: "Uninstall " + title + "?", detail: "The install folder goes to the trash; the hours and the journal stay.",
                                  buttons: ["Cancel", "Uninstall"], danger: 1 }, function(k) { if (k === 1) form.uninstall(gameId); });
            } else if (a === "remove") {
                shell.dialogAsk({ message: "Remove " + title + " from the library?", detail: "The entry is archived; the files are left where they are.",
                                  buttons: ["Cancel", "Remove"], danger: 1 }, function(k) { if (k === 1) form.remove(gameId); });
            }
        });
    }

    function activate(index, row) {
        if (row.type === "bool") {
            form.toggle(row.form);
            Sound.play("select");
        } else if (row.key === "link" && source) {
            Sound.play("ok");
            login.begin(args.source);
        } else if (row.key === "code" && source) {
            shell.prompt({ title: "Code from " + (info.name || args.source), value: "" }, function(value) {
                if (value !== null && value !== "")
                    login.submit(value);
            });
        } else if (row.key === "game") {
            Sound.play("ok");
            gameMenu(row);
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

    Connections {
        target: page.login
        function onFinished(ok, text) { page.shell.showToast(text); }
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
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20) - (qrCard.visible ? qrCard.height + Theme.dp(20) : 0)
        model: page.content
        focus: true

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedLeft: Sound.play("edge")
    }

    Item {
        id: qrCard

        x: rows.x
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight) + Theme.dp(20)
        width: rows.width
        height: Theme.dp(330)
        visible: page.source && page.login.source === page.args.source && (page.login.url !== "" || page.login.status !== "")

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(6)
            color: Theme.card
            border.width: 1
            border.color: Theme.hairline
        }

        Loader {
            id: qr
            x: Theme.dp(24)
            y: Theme.dp(24)
            width: Theme.dp(282)
            height: width
            active: page.login.url !== ""
            sourceComponent: Base.QrCode { matrix: page.login.matrix }
        }

        Column {
            x: qr.active ? qr.x + qr.width + Theme.dp(30) : Theme.dp(30)
            y: Theme.dp(30)
            width: parent.width - x - Theme.dp(30)
            spacing: Theme.dp(14)

            Label {
                width: parent.width
                text: "Scan to sign in on your phone"
            }

            Label {
                width: parent.width
                text: page.login.url
                color: Theme.accent
                wrapMode: Text.WrapAnywhere
                maximumLineCount: 4
                elide: Text.ElideRight
                font.pixelSize: Theme.dp(Theme.fontTiny)
            }

            Label {
                width: parent.width
                text: page.login.status
                color: Theme.textSecondary
                wrapMode: Text.WordWrap
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }
}
