import QtQuick
import "../core"
import "../sound"
import "../ui"

// Settings › Search, as the console's search screen: the field across the top, the keyboard under it at the left,
// the hits beside it; leaving the keyboard gives the hits the whole width. A hit opens its page.
FocusScope {
    id: page

    objectName: "settingsSearchPage"
    property var shell: null
    property var args: ({})
    focus: true

    readonly property bool strip: true
    readonly property var search: api.screens.search
    property bool typing: true

    readonly property var hints: typing ? [
        {
            glyph: "Y",
            label: "Space"
        },
        {
            glyph: "X",
            label: "Clear"
        },
        {
            glyph: "B",
            label: "Delete"
        },
        {
            glyph: "A",
            label: "Type"
        }
    ].concat(search.count > 0 ? [
        {
            glyph: "Start",
            label: "Hits"
        }
    ] : []) : [
        {
            glyph: "B",
            label: "Type"
        },
        {
            glyph: "A",
            label: rows.currentRow && rows.currentRow.kind === "gamekey" ? (rows.currentRow.expanded ? "Collapse" : "Expand") : "Open"
        }
    ]

    readonly property var results: search.results
    readonly property var content: results.map(function (r) {
        var out = Object.assign({}, r);
        // The value on the right, the description under; a game's row keeps its art.
        out.icon = r.image || (r.kind === "section" ? "settings" : "");
        out.iconSlot = r.kind === "gamerow" || r.kind === "game";
        if (r.advanced)
            out.detail = "Advanced" + (r.detail ? " · " + r.detail : "");
        return out;
    })

    function toRows() {
        if (search.count === 0) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        typing = false;
        rows.forceActiveFocus();
    }

    function toKeys() {
        Sound.play("back");
        typing = true;
        panel.forceActiveFocus();
    }

    function activate(index, row) {
        if (row.kind === "gamekey") {
            Sound.play("select");
            search.expand(index);
            return;
        }
        Sound.play("ok");
        open(row.target);
    }

    // The hit's page: a runner's, a module's, a source's or a game's over this one; a section of the settings page under it.
    function open(target) {
        if (target.page === "runner")
            shell.push("pages/FormPage.qml", {
                runner: target.id,
                key: target.key
            });
        else if (target.page === "module")
            shell.push("pages/FormPage.qml", {
                module: target.id,
                key: target.key
            });
        else if (target.page === "source")
            shell.push("pages/FormPage.qml", {
                source: target.id,
                key: target.key
            });
        else if (target.page === "game")
            shell.push("pages/GameSettingsPage.qml", {
                gameId: target.id,
                key: target.key,
                settingModule: target.module
            });
        else if (target.page === "controller" && target.key)
            shell.push("pages/ControllersPage.qml", {
                key: target.key
            });
        else {
            // This page goes with the pop: the shell is held, not read from it, when the settings page lands.
            // Opened from elsewhere than Settings (the home's search), Settings comes up to take the hit.
            var held = shell;
            held.pop();
            Qt.callLater(function () {
                if (held.topPage && held.topPage.land) {
                    held.topPage.land(target);
                    return;
                }
                held.push("pages/SettingsPage.qml", {});
                Qt.callLater(function () {
                    if (held.topPage && held.topPage.land)
                        held.topPage.land(target);
                });
            });
        }
    }

    onActiveFocusChanged: {
        if (!activeFocus)
            return;
        // Back from a hit's page: the values may have changed.
        search.load();
        (typing ? panel : rows).forceActiveFocus();
    }

    Component.onCompleted: {
        panel.built = true;
        panel.reset();
        search.load();
    }

    Connections {
        target: page.search
        function onQueryChanged() {
            rows.index = 0;
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    Label {
        x: Theme.dp(Theme.edge)
        y: Theme.dp(65) - height / 2
        text: "Settings"
        font.weight: Font.DemiBold
        font.pixelSize: Theme.dp(Theme.fontTitle)
    }

    Rectangle {
        id: field

        x: Theme.dp(Theme.edge)
        y: Theme.dp(118)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: Theme.dp(70)
        color: Qt.rgba(0, 0, 0, page.typing ? 0.42 : 0.28)

        Glyph {
            id: lens
            x: Theme.dp(22)
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(30)
            height: width
            kind: "search"
        }

        Label {
            id: queryText
            x: lens.x + lens.width + Theme.dp(16)
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - x - count.width - Theme.dp(60)
            text: page.search.query
            elide: Text.ElideLeft
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(Theme.fontTitle)
        }

        Label {
            x: queryText.x
            anchors.verticalCenter: parent.verticalCenter
            visible: page.search.query === ""
            text: page.search.ready ? "Search every setting" : "Indexing…"
            color: Theme.textMuted
            font.weight: Font.Light
            font.pixelSize: Theme.dp(Theme.fontTitle)
        }

        Rectangle {
            x: queryText.x + Math.min(queryText.implicitWidth, queryText.width) + Theme.dp(2)
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(2)
            height: Theme.dp(38)
            color: Theme.text
            visible: page.typing && caret.on
        }

        Label {
            id: count
            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(22)
            anchors.verticalCenter: parent.verticalCenter
            visible: page.search.query.trim() !== ""
            text: page.search.count === 0 ? "No match" : page.search.count === 1 ? "1 hit" : page.search.count + " hits"
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: Theme.dp(2)
            color: page.typing ? Theme.text : Theme.hairline
        }
    }

    Timer {
        id: caret
        property bool on: true
        interval: 560
        running: page.typing && page.activeFocus
        repeat: true
        onTriggered: on = !on
    }

    KeyPanel {
        id: panel

        x: page.typing ? field.x : field.x - Theme.dp(60)
        y: field.y + field.height + Theme.dp(24)
        opacity: page.typing ? 1.0 : 0.0
        visible: opacity > 0.01
        enabled: page.typing
        cursorShown: page.typing && panel.activeFocus
        focus: true

        Behavior on x {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
            }
        }

        onTyped: function (value) {
            page.search.query += value;
        }
        onBackspaced: page.search.query = page.search.query.slice(0, -1)
        onAccepted: page.toRows()
        onEscapedUp: Sound.play("edge")

        Keys.onLeftPressed: panel.move(0, -1)
        // Right past the last key reaches the hits beside the keyboard.
        Keys.onRightPressed: {
            if (panel.colIndex >= panel.rows[panel.rowIndex].length - 1)
                page.toRows();
            else
                panel.move(0, 1);
        }
        Keys.onUpPressed: panel.move(-1, 0)
        Keys.onDownPressed: panel.move(1, 0)

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            event.accepted = true;
            if (api.keys.isAccept(event))
                panel.press();
            else if (api.keys.isCancel(event)) {
                if (page.search.query === "") {
                    Sound.play("back");
                    page.shell.pop();
                } else {
                    Sound.play("type");
                    page.search.query = page.search.query.slice(0, -1);
                }
            } else if (api.keys.isDetails(event)) {
                Sound.play("type");
                page.search.query = "";
            } else if (api.keys.isFilters(event)) {
                Sound.play("type");
                page.search.query += " ";
            } else if (api.keys.isMenu(event))
                page.toRows();
            else
                event.accepted = false;
        }
    }

    readonly property real besideKeys: panel.width + Theme.dp(56)

    Label {
        x: rows.x
        y: rows.y + Theme.dp(12)
        width: rows.width
        visible: page.search.query.trim() === "" || page.search.count === 0
        text: page.search.query.trim() === "" ? "Every page, every runner, module and game: by name, by what a setting does, by its value. A game's title narrows to it." : "Nothing matches that."
        color: Theme.textSecondary
        wrapMode: Text.WordWrap
        lineHeight: 1.25
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    SettingsRows {
        id: rows

        shell: page.shell
        x: field.x + (page.typing ? page.besideKeys : 0)
        y: field.y + field.height + Theme.dp(34)
        width: field.width - (page.typing ? page.besideKeys : 0) - Theme.dp(16)
        height: parent.height - y - Theme.dp(96)
        model: page.content
        visible: page.search.count > 0
        opacity: page.typing ? 0.72 : 1.0

        Behavior on x {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }
        Behavior on width {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
            }
        }

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: {
            page.typing = true;
            panel.forceActiveFocus();
        }
        onEscapedDown: Sound.play("edge")
        onPointed: page.typing = false

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                page.toKeys();
            }
        }
    }
}
