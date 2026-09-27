import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Forms.js" as Forms

// Add Game, as a Settings page: a file on this machine, a store, or what Lutris already has.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    signal closeRequested
    focus: true

    readonly property var form: api.screens.add
    readonly property bool strip: true
    property int part: 0
    // "parts" or "rows"
    property string zone: "parts"

    readonly property var hints: {
        var row = rows.currentRow;
        return [
            {
                glyph: "B",
                label: "Back"
            },
            {
                glyph: "A",
                label: zone !== "rows" || !row || row.heading ? "OK" : row.action || "Select"
            }
        ];
    }

    Component.onCompleted: form.load()

    readonly property var content: Forms.grouped(form.groups, form.rows, function (src, i) {
        return Object.assign({}, src, {
            form: i
        });
    })
    readonly property var parts: Forms.parts(content, "Game File")
    readonly property var partRows: parts[Math.max(0, Math.min(part, parts.length - 1))].rows

    function activate(index, row) {
        if (form.busy) {
            Sound.play("edge");
        } else if (row.key === "pick_file") {
            Sound.play("ok");
            shell.browse({
                title: "Game file",
                path: "",
                files: true
            }, function (path) {
                if (path !== null && form.setFile(path))
                    pickRunner();
            });
        } else if (row.key === "store") {
            Sound.play("ok");
            if (row.available && row.loggedIn) {
                api.screens.sources.pick(row.source);
                shell.push("pages/InstallPage.qml", {});
            } else
                shell.push("pages/FormPage.qml", {
                    source: row.source
                });
        } else if (row.key === "lutris") {
            importLutris();
        }
    }

    function pickRunner() {
        var choices = form.runnerChoices;
        if (choices.length === 0) {
            form.cancel();
            Sound.play("edge");
            shell.showToast("No runner set up: add one under Settings › Runners");
            return;
        }
        shell.pick({
            title: "Runner",
            choices: choices,
            index: form.runnerIndex
        }, function (i) {
            if (i < 0) {
                form.cancel();
                return;
            }
            form.pickRunner(i);
            shell.prompt({
                title: "Title of the game",
                value: form.pendingTitle()
            }, function (title) {
                if (title === null)
                    form.cancel();
                else
                    form.addGame(title) !== "" ? Sound.play("ok") : Sound.play("edge");
            });
        });
    }

    function importLutris() {
        if (form.lutrisError !== "") {
            Sound.play("edge");
            shell.showToast("Lutris was not found: " + form.lutrisError);
            return;
        }
        if (!form.lutris) {
            Sound.play("ok");
            form.previewLutris();
            return;
        }
        var n = form.lutris.imported.length;
        if (n === 0) {
            Sound.play("edge");
            shell.showToast("Nothing new in Lutris");
            return;
        }
        shell.dialogAsk({
            message: "Import " + n + (n === 1 ? " game" : " games") + " from Lutris?",
            detail: "Their hours and artwork come along. Games already in the library are left as they are.",
            buttons: ["Cancel", "Import"]
        }, function (k) {
            if (k === 1)
                form.importLutris();
        });
    }

    onPartChanged: Qt.callLater(rows.reset)

    Connections {
        target: page.form
        function onMessage(text) {
            page.shell.showToast(text);
        }
        // A preview that just came back answers the press that asked for it.
        function onLutrisChanged() {
            if (page.form.lutris && !page.form.busy && rows.currentRow && rows.currentRow.key === "lutris" && rows.activeFocus)
                page.importLutris();
        }
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isCancel(event) && page.zone === "rows") {
            event.accepted = true;
            Sound.play("back");
            page.zone = "parts";
            partList.forceActiveFocus();
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        id: title
        anchors.left: parent.left
        anchors.right: parent.right
        title: "Add Game"
    }

    SectionList {
        id: partList

        x: Theme.dp(172)
        y: title.height + Theme.dp(10)
        width: Theme.dp(430)
        height: parent.height - y - Theme.dp(96)
        sections: page.parts
        focus: page.zone === "parts"

        onActivated: function (i) {
            page.part = i;
        }
        onPointed: page.zone = "parts"
        onEscapedRight: {
            page.zone = "rows";
            rows.forceActiveFocus();
        }
    }

    Label {
        id: status
        x: rows.x
        y: title.height + Theme.dp(24)
        width: rows.width
        height: visible ? implicitHeight + Theme.dp(20) : 0
        visible: text !== ""
        text: page.form.busy ? "Reading the Lutris library…" : api.allGames.count === 0 ? "Your first game: a file on this machine, a store, or what Lutris already has." : ""
        color: Theme.textSecondary
        elide: Text.ElideRight
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(672)
        y: title.height + Theme.dp(24) + status.height
        width: parent.width - x - Theme.dp(Theme.columnRight + 16)
        height: parent.height - y - Theme.dp(96)
        model: page.partRows
        focus: page.zone === "rows"

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedDown: Sound.play("edge")
        onPointed: page.zone = "rows"
        onEscapedLeft: {
            page.zone = "parts";
            partList.forceActiveFocus();
        }
    }
}
