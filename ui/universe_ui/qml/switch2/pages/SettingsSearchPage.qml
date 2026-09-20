import QtQuick
import "../core"
import "../sound"
import "../ui"

// Settings › Search: the query line, the ranked hits, the keyboard under them while typing; a hit opens its page.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    focus: true

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
            var held = shell;
            held.pop();
            Qt.callLater(function () {
                if (held.topPage && held.topPage.land)
                    held.topPage.land(target);
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

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "search"
        title: "Search"
        subtitle: "System Settings"
    }

    Item {
        id: field

        x: Theme.dp(120)
        y: header.height + Theme.dp(24)
        width: parent.width - x * 2
        height: Theme.dp(96)

        Label {
            id: queryText
            x: Theme.dp(8)
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - Theme.dp(240)
            text: page.search.query
            elide: Text.ElideLeft
            font.pixelSize: Theme.dp(40)
        }

        Label {
            x: Theme.dp(8)
            anchors.verticalCenter: parent.verticalCenter
            visible: page.search.query === ""
            text: page.search.ready ? "Search every setting" : "Indexing…"
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(40)
        }

        Rectangle {
            x: queryText.x + Math.min(queryText.implicitWidth, queryText.width) + Theme.dp(4)
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(3)
            height: Theme.dp(48)
            color: Theme.accent
            visible: page.typing && caret.on
        }

        Rectangle {
            anchors.bottom: parent.bottom
            width: parent.width
            height: Theme.dp(3)
            color: page.typing ? Theme.text : Theme.hairline
        }

        Label {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            visible: page.search.query.trim() !== ""
            text: page.search.count === 0 ? "No match" : page.search.count === 1 ? "1 hit" : page.search.count + " hits"
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
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

    Label {
        x: Theme.dp(120)
        y: field.y + field.height + Theme.dp(40)
        width: parent.width - x * 2
        visible: page.search.query.trim() === "" || page.search.count === 0
        text: page.search.query.trim() === "" ? "Every page, every runner, module and game: by name, by what a setting does, by its value. A game's title narrows to it." : "Nothing matches that."
        color: Theme.textSecondary
        wrapMode: Text.WordWrap
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(120)
        y: field.y + field.height + Theme.dp(30)
        width: parent.width - x * 2
        height: (page.typing ? panel.y : parent.height - Theme.dp(Theme.hintBarHeight)) - y - Theme.dp(20)
        model: page.content
        visible: page.search.count > 0
        opacity: page.typing ? 0.6 : 1.0

        Behavior on opacity {
            Ease {}
        }

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                page.toKeys();
            }
        }
    }

    KeyPanel {
        id: panel

        anchors.left: parent.left
        anchors.right: parent.right
        scale: 0.8
        cursorShown: page.typing && panel.activeFocus
        y: page.typing ? parent.height - Theme.dp(Theme.hintBarHeight) - height : parent.height
        focus: true

        Behavior on y {
            Ease {}
        }

        onTyped: function (value) {
            page.search.query += value;
        }
        onBackspaced: page.search.query = page.search.query.slice(0, -1)
        onAccepted: page.toRows()
        onEscapedUp: page.toRows()

        Keys.onLeftPressed: panel.move(0, -1)
        Keys.onRightPressed: panel.move(0, 1)
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
}
