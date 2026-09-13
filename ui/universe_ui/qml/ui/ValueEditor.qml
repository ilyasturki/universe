import QtQuick
import "../core"
import "../sound"

// The control a settings row opens, by its type: a list for an enum or for listed choices
// (with a row to type another), a keypad for an integer, the folder picker for a path, the
// keyboard for the rest. Fills the page; the list drops from the focused row of `cards`.
FocusScope {
    id: editor

    property Item cards: null
    // A page under the tabs' hint bar lets the sheets overhang it; one with its own keeps
    // them inside and lowers the floor the list stays above.
    property real overhang: Theme.dp(Theme.hintBarHeight)
    property real floor: height

    readonly property bool open: picker.open || sheet.open || paths.open
    readonly property var hints: sheet.open ? sheet.hints : paths.open ? paths.hints : picker.open ? picker.hints : []

    // edit() answers with the row's index; prompt() with its tag.
    signal accepted(int index, var value)
    signal prompted(string tag, string value)
    signal closed()

    property int pendingIndex: -1
    property string pendingTag: ""
    property var pendingRow: null

    readonly property string customLabel: "Type a value…"

    function edit(index, row) {
        pendingIndex = index;
        pendingTag = "";
        pendingRow = row;
        var choices = row.choices || [];
        if (row.type === "enum" || ((row.type === "int" || row.type === "string") && choices.length > 0)) {
            var opts = choices.map(function(c) { return { label: c }; });
            if (row.type !== "enum")
                opts.push({ label: customLabel });
            var current = choices.indexOf(String(row.value));
            picker.show(cards, opts, current >= 0 ? current : (row.type === "enum" ? 0 : opts.length - 1));
            return;
        }
        if (row.type === "path") {
            paths.show(row.label, row.value, isFile(row));
            return;
        }
        sheet.show(row.label, row.value, row.type === "int" ? "number" : "text");
    }

    function prompt(tag, label, value) {
        pendingIndex = -1;
        pendingTag = tag;
        pendingRow = null;
        sheet.show(label, value, "text");
    }

    // A key named as a file, or a value with an extension, picks files; the rest pick folders.
    function isFile(row) {
        var key = String(row.key || "");
        if (/(_path|_file|file|exe)$/.test(key))
            return true;
        var base = String(row.value || "").split("/").pop();
        return base.indexOf(".") > 0;
    }

    function finish(value) {
        if (pendingTag !== "")
            prompted(pendingTag, value);
        else if (pendingIndex >= 0)
            accepted(pendingIndex, value);
        closed();
    }

    function hide() {
        picker.hide();
        paths.open = false;
        sheet.open = false;
    }

    ChipPicker {
        id: picker

        // Drops from the focused row, its right edge on the row's value.
        x: editor.cards ? editor.cards.x + editor.cards.focusRect.x + editor.cards.focusRect.width - Theme.dp(16) - width : 0
        y: editor.cards ? Math.min(editor.floor - height - Theme.dp(20),
                                   editor.cards.y + editor.cards.focusRect.y + editor.cards.focusRect.height + Theme.dp(8)) : 0
        z: 3

        onChosen: function(index) {
            var row = editor.pendingRow || ({});
            var choices = row.choices || [];
            picker.hide();
            if (index >= 0 && index < choices.length) {
                Sound.sort();
                editor.finish(choices[index]);
            } else if (index === choices.length && row.type !== "enum") {
                Sound.panel();
                sheet.show(row.label, row.value, row.type === "int" ? "number" : "text");
            } else {
                editor.closed();
            }
        }
        onDismissed: {
            picker.hide();
            editor.closed();
        }
    }

    PathSheet {
        id: paths

        anchors.fill: parent
        anchors.bottomMargin: -editor.overhang
        z: 5

        onAccepted: function(path) { editor.finish(path); }
        onTypeRequested: function(path) {
            var row = editor.pendingRow || ({});
            sheet.show(row.label || "", path, "path");
        }
        onDismissed: editor.closed()
    }

    KeyboardSheet {
        id: sheet

        anchors.fill: parent
        anchors.bottomMargin: -editor.overhang
        z: 5

        onAccepted: function(value) { editor.finish(value); }
        onDismissed: editor.closed()
    }
}
