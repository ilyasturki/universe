import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    // { runner } | { module } | { source }: one runner's, one module's or one source's settings (a game's is GameSettingsPage);
    // `key` (and a module setting's `settingModule`) lands the cursor on that row, Advanced turned on if it sits behind it.
    property var args: ({})
    readonly property string runner: args.runner || ""
    readonly property string module: args.module || ""
    readonly property string source: args.source || ""
    readonly property var form: formOf(args)
    readonly property var info: form.info === undefined ? null : form.info
    readonly property var login: api.screens.login
    property int returnIndex: -1
    property string landKey: ""
    property string landModule: ""

    function formOf(a) {
        return a.runner ? api.screens.runner : a.source ? api.screens.source : api.screens.module;
    }

    signal closeRequested
    signal settingsRequested(var game)
    signal message(string text)

    // One sidebar entry per card; Advanced only adds rows inside them.
    readonly property var groups: form.groups
    readonly property var sections: groups.map(function (g) {
        return {
            name: g.title,
            icon: page.icons[g.title] || "sliders",
            changed: g.changed === true
        };
    })
    readonly property var icons: ({
            "Runner": "play",
            "Options": "sliders",
            "Games": "library",
            "Settings": "sliders",
            "Sign-in": "user"
        })

    readonly property var row: body.currentRow
    readonly property bool inRows: body.inRows
    readonly property string rowAction: row && row.entry ? "Remove" : "Reset"
    readonly property bool canReset: inRows && row !== null && form.resettable(row)

    readonly property var hints: editor.open ? editor.hints : menu.open ? menu.hints : [
        {
            glyph: "A",
            label: !inRows ? "Open" : row && row.type === "bool" ? "Toggle" : row && row.key === "add_file" ? "Pick a file" : row && row.type === "action" ? row.action || "Select" : "Change",
            dim: inRows && (!row || row.disabled === true || row.type === "info")
        }
    ].concat(inRows ? [
        {
            glyph: "Start",
            label: "More",
            dim: moreItems.length === 0
        }
    ] : []).concat([
        {
            glyph: "B",
            label: "Back"
        }
    ])

    // X and Y do these straight away; More lists them.
    readonly property var moreItems: (canReset && page.runner !== "" ? [
            {
                icon: "refresh",
                label: rowAction,
                action: "reset"
            }
        ] : []).concat(form.hasAdvanced ? [
        {
            icon: "sliders",
            label: form.showAdvanced ? "Hide advanced" : "Show advanced",
            action: "advanced"
        }
    ] : [])

    function openMenu() {
        if (!inRows || moreItems.length === 0) {
            Sound.edge();
            return;
        }
        Sound.panel();
        menu.show(moreItems, body.cards, body.cards.focusRect, "", function (action) {
            body.cards.forceActiveFocus();
            if (action === "reset")
                page.resetRow();
            else if (action === "advanced")
                page.toggleAdvanced();
        });
    }

    readonly property real sideMargin: Theme.dp(90)
    readonly property bool hasLogo: info !== null && info.icon !== undefined && String(info.icon) !== "" && logo.status === Image.Ready

    // A runner's form reloads on every library change; unloaded once its page is gone.
    Component.onDestruction: if (page.runner !== "")
        api.screens.runner.load("")

    // The derived runner/module/source are still stale here: read the args themselves. The cursor settles once the page is
    // laid out: a search hit lands, the row a game's settings were opened from comes back, anything else starts on the sidebar.
    onArgsChanged: {
        var id = args.runner || args.module || args.source || "";
        var form = formOf(args);
        landKey = args.key || "";
        landModule = args.settingModule || "";
        body.section = 0;
        body.zone = "side";
        if (id !== "")
            form.load(id);
        Qt.callLater(page.settle);
    }

    function settle() {
        if (page.returnIndex >= 0 && page.runner !== "") {
            body.landOn(page.returnIndex);
            page.returnIndex = -1;
        } else if (!landNow())
            body.reset();
    }

    // A search hit: the card holding the row, the cursor on it; a source's rows come back from a thread, so it lands once they are there.
    function landNow() {
        if (landKey === "")
            return false;
        var i = form.reveal(landKey, landModule);
        if (i < 0)
            return false;
        landKey = "";
        body.landOn(i);
        return true;
    }

    function toggleAdvanced() {
        Sound.panel();
        form.showAdvanced = !form.showAdvanced;
    }

    function resetRow() {
        if (!canReset) {
            Sound.edge();
            return;
        }
        form.reset(body.cards.index) ? Sound.enter() : Sound.edge();
    }

    function gameActions(row) {
        var out = [];
        if (api.allGames.byId(row.gameId))
            out.push({
                icon: "sliders",
                label: "Game settings",
                action: "settings"
            });
        if (row.installed)
            out.push({
                icon: "trash",
                label: "Uninstall…",
                action: "uninstall",
                danger: true
            });
        out.push({
            icon: "eye-off",
            label: "Remove from library…",
            action: "remove",
            danger: true
        });
        return out;
    }

    function gameAction(row, action) {
        var cards = body.cards;
        if (action === "settings") {
            cards.forceActiveFocus();
            page.returnIndex = cards.index;
            page.settingsRequested(api.allGames.byId(row.gameId));
        } else if (action === "uninstall") {
            Sound.panel();
            menu.confirm("Keep it", "trash", "Trash the install folder", "Uninstall " + row.label + "?", cards, cards.focusRect, function () {
                Sound.enter();
                form.uninstall(row.gameId);
            });
        } else if (action === "remove") {
            Sound.panel();
            menu.confirm("Keep it", "eye-off", "Remove from the library", "Remove " + row.label + "?", cards, cards.focusRect, function () {
                Sound.enter();
                form.remove(row.gameId);
            });
        } else {
            cards.forceActiveFocus();
        }
    }

    function activate(index, row) {
        var cards = body.cards;
        if (row.disabled === true || row.type === "info") {
            Sound.edge();
        } else if (row.map === true) {
            Sound.panel();
            editor.promptPair(row.label.replace(/…$/, ""), row.fields, "", "", function (name, value) {
                form.setMapEntry(index, name, value) ? Sound.enter() : Sound.edge();
            });
        } else if (row.key === "game") {
            Sound.panel();
            menu.show(gameActions(row), cards, cards.focusRect, row.label, function (action) {
                page.gameAction(row, action);
            });
        } else if (row.type === "bool") {
            form.toggle(index);
            Sound.favourite(!row.value);
            // Switching a module on that cannot guess one of its settings: the cursor goes straight to it.
            if (row.key === "enabled" && !row.value && form.setupIndex !== undefined) {
                var setup = form.setupIndex();
                if (setup >= 0)
                    body.landOn(setup);
            }
        } else if (row.key === "link" && source !== "") {
            Sound.enter();
            login.begin(source);
        } else if (row.key === "code" && source !== "") {
            Sound.panel();
            editor.prompt("Code from " + (info ? info.name : source), "", function (code) {
                login.submit(code);
            });
        } else if (row.key === "add_file") {
            Sound.panel();
            editor.edit({
                type: "path",
                key: "add_file",
                label: "Game file for " + info.name,
                value: ""
            }, function (path) {
                if (form.setValue(index, path))
                    editor.prompt("Title of the game", form.pendingTitle(), function (title) {
                        form.addGame(title) !== "" ? Sound.enter() : Sound.edge();
                    });
            });
        } else {
            Sound.panel();
            editor.edit(row, function (value) {
                form.setValue(index, value);
            });
        }
    }

    Connections {
        target: page.form
        ignoreUnknownSignals: true
        function onMessage(text) {
            page.message(text);
        }
        function onRowsChanged() {
            page.landNow();
        }
    }

    Connections {
        target: page.login
        function onFinished(ok, text) {
            page.message(text);
        }
    }

    Item {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: Math.max(Theme.dp(88), head.height)

        Image {
            id: logo
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            height: parent.height
            width: height
            source: page.info && page.info.icon ? Qt.resolvedUrl("../" + page.info.icon) : ""
            asynchronous: true
            fillMode: Image.PreserveAspectFit
            sourceSize.height: 256
            smooth: true
            mipmap: true
            visible: page.hasLogo
        }

        Column {
            id: head
            anchors.left: parent.left
            anchors.leftMargin: page.hasLogo ? logo.width + Theme.dp(28) : 0
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(4)

            CapsLabel {
                text: page.runner !== "" ? "RUNNER" : page.source !== "" ? "SOURCE" : "MODULE"
            }

            Text {
                width: parent.width
                text: page.info ? page.info.name || "" : ""
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(42)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: page.info ? (page.info.meta || "") + (page.info.warning ? (page.info.meta ? " · " : "") + "<font color=\"#e0655a\">" + page.info.warning + "</font>" : "") : ""
                textFormat: Text.StyledText
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: page.info ? page.info.description || "" : ""
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                wrapMode: Text.WordWrap
                maximumLineCount: 3
                elide: Text.ElideRight
            }
        }
    }

    CardSections {
        id: body

        anchors.fill: parent
        focus: true
        columnsTop: header.y + header.height + Theme.dp(32)
        floor: loginCard.visible ? loginCard.y - Theme.dp(24) : hintBar.y
        sideMargin: page.sideMargin
        rows: page.form.rows
        groups: page.groups
        sections: page.sections
        dimmed: editor.open || menu.open

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onCancelled: page.closeRequested()
    }

    LoginCard {
        id: loginCard

        anchors.bottom: hintBar.top
        anchors.bottomMargin: Theme.dp(24)
        x: body.mainX
        width: body.mainWidth
        source: page.source
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        z: 3
        sideMargin: page.sideMargin
        hints: page.hints
    }

    ValueEditor {
        id: editor

        anchors.fill: parent
        cards: body.cards
        overhang: 0
        floor: hintBar.y
        z: 2

        onClosed: body.cards.forceActiveFocus()
    }

    ActionMenu {
        id: menu

        anchors.fill: parent
        z: 4

        onDismissed: body.cards.forceActiveFocus()
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat || editor.open || menu.open)
            return;
        if (api.keys.isMenu(event)) {
            event.accepted = true;
            page.openMenu();
        } else if (api.keys.isFilters(event) && form.hasAdvanced) {
            event.accepted = true;
            page.toggleAdvanced();
        } else if (api.keys.isDetails(event) && page.runner !== "") {
            event.accepted = true;
            page.resetRow();
        }
    }
}
