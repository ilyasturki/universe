import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Forms.js" as Forms

// First-run setup: one row list per step (discover, stores, import, preferences, done), X moves on, B goes back or skips.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    focus: true

    readonly property var form: api.screens.onboarding
    readonly property var login: api.screens.login
    readonly property var current: form.steps[form.step] || {
        title: "",
        subtitle: ""
    }
    readonly property bool last: form.step >= form.steps.length - 1
    property string source: ""

    readonly property var hints: {
        var row = rows.currentRow;
        var label = !row || row.heading || row.type === "info" || row.type === "static" ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? row.action || "Select" : "Change";
        return [
            {
                glyph: "B",
                label: form.step > 0 ? "Back" : "Skip setup"
            },
            {
                glyph: "X",
                label: last ? "Finish" : "Continue"
            },
            {
                glyph: "A",
                label: label
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
        if (form.busy || row.type === "info" || row.type === "static") {
            Sound.play("edge");
        } else if (row.key === "link") {
            Sound.play("ok");
            page.source = row.module;
            login.begin(row.module);
        } else if (row.key === "code") {
            page.source = row.module;
            shell.prompt({
                title: "Code from " + row.section,
                value: ""
            }, function (value) {
                if (value !== null && value !== "")
                    login.submit(value);
            });
        } else if (row.via !== undefined) {
            Sound.play(form.runImport(row.form) ? "ok" : "edge");
        } else if (row.type === "bool") {
            form.toggle(row.form);
            Sound.play("select");
        } else {
            rows.edit(row, function (value) {
                form.setValue(row.form, value);
            });
        }
    }

    function advance() {
        Sound.play("ok");
        form.next();
    }

    function retreat() {
        Sound.play("back");
        if (form.step > 0)
            form.back();
        else
            form.finish();
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat || shell.modal)
            return;
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            retreat();
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            advance();
        }
    }

    Connections {
        target: page.form
        function onMessage(text) {
            page.shell.showToast(text);
        }
        function onFinished() {
            page.shell.pop();
        }
        function onStepChanged() {
            Qt.callLater(function () {
                rows.reset();
                rows.forceActiveFocus();
            });
        }
    }

    Connections {
        target: page.login
        function onFinished(ok, text) {
            page.shell.showToast(text);
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "settings"
        title: page.current.title
        subtitle: "Set up · " + (page.form.step + 1) + " of " + page.form.steps.length
    }

    Label {
        id: description
        x: Theme.dp(120)
        y: header.height + Theme.dp(24)
        width: parent.width - x - Theme.dp(120)
        text: page.form.busy && page.form.count === 0 ? "Looking at this machine…" : page.current.subtitle
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
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20) - (qrCard.visible ? qrCard.height + Theme.dp(20) : 0)
        model: page.content
        focus: true

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
    }

    LoginCard {
        id: qrCard

        x: rows.x
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight) + Theme.dp(20)
        width: rows.width
        source: page.form.stepId === "stores" ? page.source : ""
    }
}
