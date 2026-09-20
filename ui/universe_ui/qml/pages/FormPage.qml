import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../ui/Maps.js" as Maps

FocusScope {
    id: page

    focus: true

    // { game } | { runner } | { module } | { source }: one game's, one runner's, one module's or one source's settings;
    // `key` (and a module setting's `settingModule`) lands the cursor on that row, the Advanced row opened if it sits behind it.
    property var args: ({})
    readonly property var game: args.game || null
    readonly property string runner: args.runner || ""
    readonly property string module: args.module || ""
    readonly property string source: args.source || ""
    readonly property var form: formOf(args)
    readonly property var info: game || form.info === undefined ? null : form.info
    readonly property var login: api.screens.login
    property int returnIndex: -1
    property string landKey: ""
    property string landModule: ""

    function formOf(a) {
        return a.game ? api.screens.gameSettings : a.runner ? api.screens.runner : a.source ? api.screens.source : api.screens.module;
    }

    signal closeRequested
    signal settingsRequested(var game)
    signal message(string text)

    readonly property var hints: editor.open ? editor.hints : menu.open ? menu.hints : [
        {
            glyph: "A",
            label: cards.currentRow && cards.currentRow.type === "bool" ? "Toggle" : cards.currentRow && cards.currentRow.key === "add_file" ? "Pick a file" : cards.currentRow && cards.currentRow.type === "action" ? cards.currentRow.action || "Select" : "Change",
            dim: !cards.currentRow || cards.currentRow.disabled === true || cards.currentRow.type === "info"
        },
        {
            glyph: "dpad",
            label: "Navigate"
        },
        {
            glyph: "B",
            label: "Back"
        }
    ]

    readonly property real sideMargin: Theme.dp(90)
    readonly property bool hasLogo: info !== null && info.icon !== undefined && String(info.icon) !== "" && logo.status === Image.Ready

    // A runner's form reloads on every library change; unloaded once its page is gone.
    Component.onDestruction: if (page.runner !== "")
        api.screens.runner.load("")

    // The derived game/runner/module/source are still stale here: read the args themselves.
    onArgsChanged: {
        var id = args.game ? args.game.id : args.runner || args.module || args.source || "";
        var form = formOf(args);
        landKey = args.key || "";
        landModule = args.settingModule || "";
        if (id !== "")
            form.load(id);
        Qt.callLater(function () {
            cards.reset();
            if (page.returnIndex >= 0 && page.runner !== "") {
                cards.index = page.returnIndex;
                page.returnIndex = -1;
            }
            page.landNow();
        });
    }

    // A source's rows come back from a thread: land once they are there.
    function landNow() {
        if (landKey === "")
            return;
        var i = form.reveal(landKey, landModule);
        if (i < 0)
            return;
        landKey = "";
        Qt.callLater(function () {
            cards.index = i;
        });
    }

    function editMap(index, row) {
        Sound.panel();
        menu.show(Maps.items(row), cards, cards.focusRect, row.label, function (action) {
            if (action === "add") {
                editor.prompt("Name of " + Maps.noun(row), "", function (name) {
                    name = Maps.cleanName(name);
                    if (name === "")
                        return;
                    editor.prompt("Value of " + name, "", function (value) {
                        form.setMapEntry(index, name, value);
                    });
                });
            } else if (action.indexOf("entry:") === 0) {
                var name = action.substring(6);
                menu.show(Maps.entryItems(name), cards, cards.focusRect, name, function (next) {
                    if (next === "value")
                        editor.prompt("Value of " + name, Maps.valueOf(row, name), function (value) {
                            form.setMapEntry(index, name, value);
                        });
                    else if (next === "remove") {
                        Sound.cancel();
                        form.setMapEntry(index, name, "");
                    }
                    cards.forceActiveFocus();
                });
            }
        });
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
        if (row.disabled === true || row.type === "info") {
            Sound.edge();
        } else if (row.key === "advanced") {
            Sound.panel();
            form.showAdvanced = !form.showAdvanced;
            if (form.showAdvanced)
                Qt.callLater(cards.stepInto);
        } else if (row.type === "map") {
            editMap(index, row);
        } else if (row.key === "game") {
            Sound.panel();
            menu.show(gameActions(row), cards, cards.focusRect, row.label, function (action) {
                page.gameAction(row, action);
            });
        } else if (row.type === "bool") {
            form.toggle(index);
            Sound.favourite(!row.value);
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

    GameBackdrop {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        visible: page.game !== null
        game: page.game
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

        GameHeader {
            anchors.fill: parent
            visible: page.game !== null
            game: page.game
            label: "GAME SETTINGS"
        }

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
            visible: page.game === null

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

    SettingsCards {
        id: cards

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(page.game ? 32 : 40)
        anchors.bottom: loginCard.visible ? loginCard.top : hintBar.top
        anchors.bottomMargin: loginCard.visible ? Theme.dp(24) : 0
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        focus: true
        compact: true
        rows: page.form.rows
        groups: page.form.groups
        dimmed: editor.open || menu.open

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedUp: Sound.edge()
        onEscapedLeft: Sound.edge()

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                page.closeRequested();
            }
        }
    }

    LoginCard {
        id: loginCard

        anchors.bottom: hintBar.top
        anchors.bottomMargin: Theme.dp(24)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
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
        cards: cards
        overhang: 0
        floor: hintBar.y
        z: 2

        onClosed: cards.forceActiveFocus()
    }

    ActionMenu {
        id: menu

        anchors.fill: parent
        z: 4

        onDismissed: cards.forceActiveFocus()
    }
}
