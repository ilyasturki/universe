import QtQuick
import "../core"
import "../sound"

// Settings › Search: a field, the ranked hits as one card, the keyboard under them while typing.
FocusScope {
    id: pane

    readonly property var search: api.screens.search
    property bool typing: true

    signal openRequested(var target)
    signal escapedLeft
    signal escapedUp

    readonly property var hints: typing ? (api.keys.mode === "keyboard" ? [
            {
                glyph: "A",
                label: "To the hits"
            },
            {
                glyph: "B",
                label: "Sections"
            }
        ] : [
            {
                glyph: "A",
                label: "Type"
            },
            {
                glyph: "X",
                label: "Backspace"
            },
            {
                glyph: "B",
                label: "Sections"
            }
        ]).concat(search.count > 0 && api.keys.mode !== "keyboard" ? [
        {
            glyph: "dpad",
            label: "Up to the hits"
        }
    ] : []) : [
        {
            glyph: "A",
            label: results.currentRow && results.currentRow.kind === "gamekey" ? (results.currentRow.expanded ? "Collapse" : "Expand") : "Open",
            dim: !results.currentRow
        },
        {
            glyph: "B",
            label: "Type"
        }
    ]

    readonly property real fieldHeight: Theme.dp(64)
    readonly property real keyboardWidth: Math.min(width, Theme.dp(880))
    readonly property string placeholder: search.ready ? "Search every setting" : "Indexing…"

    // The section opened: the index is rebuilt; the focus comes with the sidebar's Right or A.
    function indices(n) {
        var out = [];
        for (var i = 0; i < n; i++)
            out.push(i);
        return out;
    }

    function open() {
        typing = true;
        search.load();
    }

    function toResults() {
        if (search.count === 0) {
            Sound.edge();
            return;
        }
        Sound.panel();
        typing = false;
        results.forceActiveFocus();
    }

    function toKeyboard() {
        Sound.panel();
        typing = true;
        keyboard.forceActiveFocus();
    }

    function kbMove(dRow, dCol) {
        if (dRow < 0 && keyboard.rowIndex === 0) {
            toResults();
            return;
        }
        keyboard.move(dRow, dCol) ? Sound.kbtick() : Sound.edge();
    }

    function activate(index, row) {
        if (row.kind === "gamekey") {
            Sound.panel();
            search.expand(index);
            return;
        }
        Sound.enter();
        pane.openRequested(row.target);
    }

    onActiveFocusChanged: {
        if (activeFocus)
            (typing ? keyboard : results).forceActiveFocus();
    }

    Connections {
        target: pane.search
        function onQueryChanged() {
            results.index = 0;
        }
    }

    Item {
        id: field

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        height: pane.fieldHeight

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(16)
            color: Theme.surface
            border.width: 1
            border.color: pane.typing && pane.activeFocus ? Qt.rgba(1, 1, 1, 0.22) : Theme.surfaceBorder
        }

        MenuGlyph {
            id: glass
            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(24)
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(26)
            height: width
            kind: "search"
            tint: Theme.text
        }

        Text {
            id: queryText
            anchors.left: glass.right
            anchors.leftMargin: Theme.dp(18)
            anchors.verticalCenter: parent.verticalCenter
            text: pane.search.query
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(26)
        }

        Text {
            visible: pane.search.query === ""
            anchors.left: glass.right
            anchors.leftMargin: Theme.dp(18)
            anchors.verticalCenter: parent.verticalCenter
            text: pane.placeholder
            color: Theme.textMuted
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(26)
        }

        Rectangle {
            anchors.left: queryText.right
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(3)
            height: Theme.dp(30)
            color: Theme.text
            visible: pane.typing && pane.activeFocus && caret.on
        }

        Text {
            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(24)
            anchors.verticalCenter: parent.verticalCenter
            visible: pane.search.query.trim() !== ""
            text: pane.search.count === 0 ? "No match" : pane.search.count === 1 ? "1 hit" : pane.search.count + " hits"
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(20)
        }
    }

    Timer {
        id: caret
        property bool on: true

        interval: 560
        running: pane.typing && pane.activeFocus
        repeat: true
        onTriggered: on = !on
    }

    SettingsCards {
        id: results

        anchors.top: field.bottom
        anchors.topMargin: Theme.dp(24)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: sheet.top
        anchors.bottomMargin: Theme.dp(20)
        columns: 1
        compact: true
        rows: pane.search.results
        groups: pane.search.count > 0 ? [
            {
                title: "",
                rows: indices(pane.search.results.length)
            }
        ] : []
        dimmed: pane.typing
        opacity: pane.search.count > 0 ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }

        onActivated: function (index, row) {
            pane.activate(index, row);
        }
        onPointed: pane.typing = false
        onEscapedUp: pane.escapedUp()
        onEscapedDown: pane.toKeyboard()
        onEscapedLeft: {
            Sound.panel();
            pane.escapedLeft();
        }

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                pane.toKeyboard();
            }
        }
    }

    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        y: field.height + Theme.dp(60)
        visible: pane.search.query.trim() !== "" && pane.search.count === 0
        text: "Nothing matches that."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(24)
    }

    Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Theme.dp(80)
        anchors.rightMargin: Theme.dp(80)
        y: field.height + Theme.dp(60)
        visible: pane.search.query.trim() === ""
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: "Every setting on every page, a runner's, a module's, a game's: by name, by what it does, by its value. A game's title narrows to it."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(21)
        lineHeight: 1.25
    }

    Item {
        id: sheet

        anchors.left: parent.left
        anchors.right: parent.right
        height: Theme.dp(28) + keyboard.height + Theme.dp(20)
        y: pane.typing ? parent.height - height : parent.height
        clip: true

        Behavior on y {
            Ease {
                duration: Theme.durView
                easing.type: Easing.OutQuint
            }
        }

        Rectangle {
            anchors.fill: parent
            anchors.bottomMargin: -Theme.dp(30)
            radius: Theme.dp(24)
            color: Qt.rgba(0.071, 0.075, 0.094, 1.0)
            border.width: 1
            border.color: Theme.surfaceBorder
        }

        VirtualKeyboard {
            id: keyboard

            y: Theme.dp(28)
            anchors.horizontalCenter: parent.horizontalCenter
            width: pane.keyboardWidth
            height: implicitHeight
            keyHeight: Theme.dp(50)
            keyGap: Theme.dp(9)
            focus: true

            onCharEntered: function (value) {
                Sound.type();
                pane.search.query += value;
            }
            onBackspaced: {
                Sound.backspace();
                pane.search.query = pane.search.query.slice(0, -1);
            }
            onCleared: {
                Sound.backspace();
                pane.search.query = "";
            }

            Keys.onLeftPressed: pane.kbMove(0, -1)
            Keys.onRightPressed: pane.kbMove(0, 1)
            Keys.onUpPressed: pane.kbMove(-1, 0)
            Keys.onDownPressed: pane.kbMove(1, 0)

            onDone: pane.toResults()
            onPointed: {
                pane.typing = true;
                keyboard.forceActiveFocus();
            }

            Keys.onPressed: function (event) {
                if (keyboard.typed(event)) {
                    event.accepted = true;
                    return;
                }
                if (event.isAutoRepeat)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    keyboard.press();
                } else if (api.keys.isDetails(event)) {
                    event.accepted = true;
                    Sound.backspace();
                    pane.search.query = pane.search.query.slice(0, -1);
                } else if (api.keys.isCancel(event)) {
                    event.accepted = true;
                    Sound.cancel();
                    pane.escapedLeft();
                }
            }
        }
    }
}
