import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Details.js" as Details
import "Forms.js" as Forms

// A runner's, a module's or a source's settings, on the console's two columns: its cards left, their rows right.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    signal closeRequested
    focus: true

    readonly property bool strip: true
    readonly property bool runner: args.runner !== undefined
    readonly property bool source: args.source !== undefined
    readonly property var form: runner ? api.screens.runner : source ? api.screens.source : api.screens.module
    readonly property var info: form.info
    readonly property var login: api.screens.login

    // The form's cards as sub-sections; Advanced (Y) only adds rows inside them, each folded card under a heading of its own.
    readonly property var groups: form.groups
    readonly property var sections: groups.map(function (g) {
        return {
            label: g.title,
            detail: g.meta || "",
            group: 0,
            changed: g.changed === true
        };
    })
    property int section: 0
    property string zone: "list"
    readonly property var currentRow: rows.currentRow

    readonly property var hints: {
        var row = rows.currentRow;
        var label = zone !== "rows" ? "OK" : !row || row.heading || row.disabled || row.type === "info" ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? "Select" : "Change";
        var out = [];
        if (form.hasAdvanced)
            out.push({
                glyph: "Y",
                label: form.showAdvanced ? "Hide advanced" : "Show advanced"
            });
        if (runner && zone === "rows" && row && !row.heading)
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

    // `key` lands the cursor on that row, Advanced turned on if it sits behind it; a source's rows come back from a thread.
    property string landKey: ""

    Component.onDestruction: if (page.runner)
        api.screens.runner.load("")
    onArgsChanged: {
        landKey = args.key || "";
        section = 0;
        zone = "list";
        (args.runner ? api.screens.runner : args.source ? api.screens.source : api.screens.module).load(args.runner || args.source || args.module);
        Qt.callLater(landNow);
    }

    // A search hit: the card holding the row, the cursor on it.
    function landNow() {
        if (landKey === "")
            return;
        var i = form.reveal(landKey, "");
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
        var src = form.rows[i];
        var r = Object.assign({}, runner ? src : Details.withDetail(src, src.module), {
            form: i
        });
        if (runner && src.key === "exe")
            r.detail = "";
        else if (src.key === "component")
            r.display = src.tag && src.tag !== "Updated" ? src.tag : src.display;
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

    function gameMenu(row) {
        var items = [], gameId = row.gameId, title = row.label;
        if (api.allGames.byId(gameId))
            items.push({
                label: "Game Settings",
                glyph: "sliders",
                act: "settings"
            });
        if (row.installed)
            items.push({
                label: "Uninstall…",
                glyph: "trash",
                act: "uninstall"
            });
        items.push({
            label: "Remove from Library…",
            glyph: "eye-off",
            act: "remove",
            gap: items.length > 0
        });
        shell.menu(title, items, function (a) {
            if (a === "settings") {
                Sound.play("ok");
                shell.push("pages/GameSettingsPage.qml", {
                    gameId: gameId
                });
            } else if (a === "uninstall") {
                var via = api.universe.uninstallVia(gameId);
                shell.dialogAsk({
                    message: "Uninstall " + title + "?",
                    detail: (via ? via + " removes the files" : "The install folder goes to the trash") + "; the hours and the journal stay.",
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

    // X on a runner's row: its own program or gamescope switch goes back to what was found or the global's.
    function resetRow() {
        var row = rows.currentRow;
        if (!runner || zone !== "rows" || !row || row.heading || !form.resettable(row)) {
            Sound.play("edge");
            return;
        }
        Sound.play(form.reset(row.form) ? "select" : "edge");
    }

    function activate(index, row) {
        if (row.map === true) {
            Sound.play("ok");
            Forms.addEntry(shell, row, function (name, value) {
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
        } else if (row.key === "component") {
            shell.componentOptions(row.component, row.label);
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

    onSectionChanged: Qt.callLater(rows.reset)

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
        } else if (api.keys.isDetails(event) && page.runner) {
            event.accepted = true;
            page.resetRow();
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    Label {
        x: Theme.dp(98)
        y: Theme.dp(40)
        text: (page.runner ? "Runner" : page.source ? "Source" : "Module").toUpperCase()
        color: Theme.textMuted
        font.letterSpacing: Theme.dp(2)
        font.pixelSize: Theme.dp(Theme.fontTiny)
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        title: page.info.name || ""
    }

    SectionList {
        id: list

        x: Theme.dp(172)
        y: header.height + Theme.dp(10)
        width: Theme.dp(430)
        height: parent.height - y - Theme.dp(96)
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

    Column {
        id: about

        x: rows.x
        y: header.height + Theme.dp(24)
        width: rows.width
        spacing: Theme.dp(8)

        Label {
            id: metaLine
            width: parent.width
            visible: text !== ""
            text: (page.info.meta || "") + (page.info.warning ? (page.info.meta ? " · " : "") + "<font color=\"" + Theme.danger + "\">" + page.info.warning + "</font>" : "")
            textFormat: Text.StyledText
            color: Theme.textSecondary
            elide: Text.ElideRight
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Label {
            id: description
            width: parent.width
            visible: text !== ""
            text: page.info.description || ""
            color: Theme.textMuted
            wrapMode: Text.WordWrap
            maximumLineCount: 3
            elide: Text.ElideRight
            lineHeight: 1.2
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }

    // The rows start under the form's own lines, when it has any.
    readonly property real rowsTop: about.y + (metaLine.visible || description.visible ? about.height + Theme.dp(28) : 0)
    readonly property real floor: parent.height - Theme.dp(96) - (qrCard.visible ? qrCard.height + Theme.dp(24) : 0)

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(672)
        y: page.rowsTop
        width: parent.width - x - Theme.dp(Theme.columnRight + 16)
        height: page.floor - y
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

    SettingsLoginCard {
        id: qrCard

        x: rows.x
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(96)
        width: rows.width
        source: page.source ? page.args.source : ""
    }
}
