import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Forms.js" as Forms

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    focus: true

    readonly property var form: api.screens.add

    readonly property var hints: {
        var row = rows.currentRow;
        return [
            {
                glyph: "B",
                label: "Back"
            },
            {
                glyph: "A",
                label: !row || row.heading ? "OK" : row.action || "Select"
            }
        ];
    }

    Component.onCompleted: form.load()

    readonly property var content: Forms.grouped(form.groups, form.rows, function (src, i) {
        return Object.assign({}, src, {
            form: i
        });
    })

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
            if (!row.available)
                shell.push("pages/SettingsPage.qml", {
                    section: "modules"
                });
            else if (row.loggedIn)
                shell.push("pages/InstallPage.qml", {});
            else
                shell.push("pages/SettingsPage.qml", {
                    section: "signin"
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
            shell.showToast("No runner set up: add one under System Settings › Runners");
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

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "plus"
        title: "Add a game"
        subtitle: api.allGames.count === 0 ? "Your first game" : "Library"
    }

    Label {
        x: Theme.dp(120)
        y: header.height + Theme.dp(24)
        width: parent.width - x - Theme.dp(120)
        text: page.form.busy ? "Reading the Lutris library…" : "A file on this machine, a store, or what Lutris already has."
        color: Theme.textSecondary
        elide: Text.ElideRight
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(120)
        y: header.height + Theme.dp(84)
        width: parent.width - x - Theme.dp(120)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        model: page.content
        focus: true

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
        onEscapedDown: Sound.play("edge")
    }
}
