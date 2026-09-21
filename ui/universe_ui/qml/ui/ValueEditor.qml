import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: editor

    property Item cards: null
    property real overhang: Theme.dp(Theme.hintBarHeight)
    property real floor: height

    readonly property bool open: picker.open || (sheets.item !== null && sheets.item.open)
    readonly property var hints: sheets.item !== null && sheets.item.open ? sheets.item.hints : picker.open ? picker.hints : []

    signal closed

    property var done: null
    property var pendingRow: null

    readonly property string customLabel: "Type a value…"

    function edit(row, after) {
        done = after;
        pendingRow = row;
        var choices = row.choices || [];
        if (row.type === "enum" || ((row.type === "int" || row.type === "string") && choices.length > 0)) {
            // A choice's icon (a runner's logo) rides along when the row lists them.
            var opts = choices.map(function (c, i) {
                return {
                    label: c,
                    icon: row.icons && row.icons[i] ? row.icons[i] : ""
                };
            });
            if (row.type !== "enum")
                opts.push({
                    label: customLabel
                });
            var current = choices.indexOf(String(row.value));
            picker.show(cards, opts, current >= 0 ? current : (row.type === "enum" ? 0 : opts.length - 1));
        } else if (row.type === "path") {
            // A pad walks the folders; a keyboard or a mouse types the path, the folders one hop away.
            if (api.keys.mode === "pad")
                sheetsOf().paths.show(row.label, row.value, isFile(row));
            else
                sheetsOf().sheet.show(row.label, row.value, "path");
        } else {
            sheetsOf().sheet.show(row.label, row.value, row.type === "int" ? "number" : "text");
        }
    }

    function prompt(label, value, after) {
        done = after;
        pendingRow = null;
        sheetsOf().sheet.show(label, value, "text");
    }

    // Two texts at once (a variable and its value): `after(first, second)`.
    function promptPair(label, names, first, second, after) {
        done = after;
        pendingRow = null;
        sheetsOf().sheet.showPair(label, names, first, second);
    }

    function sheetsOf() {
        sheets.active = true;
        return sheets.item;
    }

    function isFile(row) {
        var key = String(row.key || "");
        if (/(_path|_file|file|exe)$/.test(key))
            return true;
        var base = String(row.value || "").split("/").pop();
        return base.indexOf(".") > 0;
    }

    function finish(value, second) {
        var after = done;
        done = null;
        closed();
        if (after)
            after(value, second);
    }

    function hide() {
        picker.hide();
        if (sheets.item) {
            sheets.item.paths.open = false;
            sheets.item.sheet.open = false;
        }
    }

    ChipPicker {
        id: picker

        x: editor.cards ? editor.cards.x + editor.cards.focusRect.x + editor.cards.focusRect.width - Theme.dp(16) - width : 0
        y: editor.cards ? Math.min(editor.floor - height - Theme.dp(20), editor.cards.y + editor.cards.focusRect.y + editor.cards.focusRect.height + Theme.dp(8)) : 0
        z: 3

        onChosen: function (index) {
            var row = editor.pendingRow || ({});
            var choices = row.choices || [];
            picker.hide();
            if (index >= 0 && index < choices.length) {
                Sound.sort();
                editor.finish(choices[index]);
            } else if (index === choices.length && row.type !== "enum") {
                Sound.panel();
                editor.sheetsOf().sheet.show(row.label, row.value, row.type === "int" ? "number" : "text");
            } else {
                editor.closed();
            }
        }
        onDismissed: {
            picker.hide();
            editor.closed();
        }
    }

    Loader {
        id: sheets

        anchors.fill: parent
        anchors.bottomMargin: -editor.overhang
        z: 5
        active: false

        sourceComponent: Item {
            property alias paths: paths
            property alias sheet: sheet
            readonly property bool open: paths.open || sheet.open
            readonly property var hints: sheet.open ? sheet.hints : paths.hints

            PathSheet {
                id: paths

                anchors.fill: parent

                onAccepted: function (path) {
                    editor.finish(path);
                }
                onTypeRequested: function (path) {
                    var row = editor.pendingRow || ({});
                    sheet.show(row.label || "", path, "path");
                }
                onDismissed: editor.closed()
            }

            KeyboardSheet {
                id: sheet

                anchors.fill: parent

                onAccepted: function (value) {
                    editor.finish(value);
                }
                onAcceptedPair: function (first, second) {
                    editor.finish(first, second);
                }
                onBrowseRequested: function (path) {
                    var row = editor.pendingRow || ({});
                    paths.show(row.label || "", path, editor.isFile(row));
                }
                onDismissed: editor.closed()
            }
        }
    }
}
