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
    // The value for all the games a game's row reaches (`row.reach`): Y in the picker, or the question after a sheet.
    property var doneAll: null
    property var pendingRow: null
    property bool customAll: false

    readonly property string customLabel: "Type a value…"

    function edit(row, after, all) {
        done = after;
        doneAll = all && row.reach ? all : null;
        pendingRow = row;
        customAll = false;
        var choices = row.choices || [];
        if (row.type === "enum" || ((row.type === "int" || row.type === "string") && choices.length > 0)) {
            var current = choices.indexOf(String(row.value));
            // A choice's icon (a runner's logo) rides along when the row lists them.
            var opts = choices.map(function (c, i) {
                return {
                    label: c,
                    image: row.icons && row.icons[i] ? row.icons[i] : "",
                    active: i === current,
                    action: String(i)
                };
            });
            if (row.type !== "enum")
                opts.push({
                    icon: "keyboard",
                    label: customLabel,
                    action: "custom",
                    gap: true
                });
            picker.show(opts, cards, cards.focusRect, "", editor.chosen, current >= 0 ? current : (row.type === "enum" ? 0 : opts.length - 1), allLabel());
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

    function allLabel() {
        return doneAll && pendingRow ? "All " + pendingRow.reach : "";
    }

    function prompt(label, value, after) {
        done = after;
        doneAll = null;
        pendingRow = null;
        customAll = false;
        sheetsOf().sheet.show(label, value, "text");
    }

    // Two texts at once (a variable and its value): `after(first, second)`, `all` the same for every game `row` reaches.
    function promptPair(label, names, first, second, after, all, row) {
        done = after;
        doneAll = all && row && row.reach ? all : null;
        pendingRow = row || null;
        customAll = false;
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

    // A sheet's value: on a row that reaches other games, which ones it is for comes next, A this game, Y all of them.
    function settle(value, second) {
        if (!doneAll || customAll) {
            finish(value, second, customAll);
            return;
        }
        picker.show([
            {
                label: "This game",
                action: "game"
            },
            {
                icon: "library",
                label: allLabel(),
                action: "all"
            }
        ], cards, cards.focusRect, "", function (action, all) {
            editor.finish(value, second, all || action === "all");
        }, 0, allLabel());
    }

    function finish(value, second, all) {
        var after = all && doneAll ? doneAll : done;
        done = null;
        doneAll = null;
        customAll = false;
        closed();
        if (after)
            after(value, second);
    }

    function chosen(action, all) {
        var row = editor.pendingRow || ({});
        var choices = row.choices || [];
        if (action === "custom") {
            Sound.panel();
            editor.customAll = all === true;
            editor.sheetsOf().sheet.show(row.label, row.value, row.type === "int" ? "number" : "text");
            return;
        }
        Sound.sort();
        editor.finish(choices[Number(action)], undefined, all);
    }

    function dismiss() {
        customAll = false;
        closed();
    }

    function hide() {
        picker.hide();
        if (sheets.item) {
            sheets.item.paths.open = false;
            sheets.item.sheet.open = false;
        }
    }

    ActionMenu {
        id: picker

        anchors.fill: parent
        z: 3

        onDismissed: editor.dismiss()
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
                    editor.settle(path);
                }
                onTypeRequested: function (path) {
                    var row = editor.pendingRow || ({});
                    sheet.show(row.label || "", path, "path");
                }
                onDismissed: editor.dismiss()
            }

            KeyboardSheet {
                id: sheet

                anchors.fill: parent

                onAccepted: function (value) {
                    editor.settle(value);
                }
                onAcceptedPair: function (first, second) {
                    editor.settle(first, second);
                }
                onBrowseRequested: function (path) {
                    var row = editor.pendingRow || ({});
                    paths.show(row.label || "", path, editor.isFile(row));
                }
                onDismissed: editor.dismiss()
            }
        }
    }
}
