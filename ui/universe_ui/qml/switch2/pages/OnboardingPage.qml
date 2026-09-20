import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Forms.js" as Forms

// First-run setup as a dialog over the shell: one row list per step (found, stores, preferences, done), X moves on, B goes back or skips.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property bool overlay: true
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

    Rectangle {
        anchors.fill: parent
        color: Theme.scrim
    }

    Rectangle {
        id: card

        readonly property real pad: Theme.dp(56)
        readonly property real gap: Theme.dp(24)
        readonly property real room: parent.height - Theme.dp(Theme.hintBarHeight) - Theme.dp(120)
        readonly property real loginRoom: qrCard.visible ? qrCard.height + gap : 0

        anchors.horizontalCenter: parent.horizontalCenter
        y: (parent.height - Theme.dp(Theme.hintBarHeight) - height) / 2
        width: Theme.dp(1200)
        height: pad * 2 + head.height + gap + rows.height + loginRoom
        radius: Theme.dp(6)
        color: Theme.card

        Column {
            id: head

            x: card.pad
            y: card.pad
            width: parent.width - card.pad * 2
            spacing: Theme.dp(6)

            Label {
                text: (page.form.step + 1) + " / " + page.form.steps.length
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontTiny)
            }

            Label {
                width: parent.width
                text: page.current.title
                font.pixelSize: Theme.dp(Theme.fontTitle)
                elide: Text.ElideRight
            }

            Label {
                width: parent.width
                visible: text !== ""
                text: page.form.busy && page.form.count === 0 ? "Looking at this machine…" : page.current.subtitle
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
                elide: Text.ElideRight
            }
        }

        SettingsRows {
            id: rows

            shell: page.shell
            x: card.pad
            y: head.y + head.height + card.gap
            width: parent.width - card.pad * 2
            height: Math.min(rows.contentHeight + rows.room * 2, card.room - card.pad * 2 - head.height - card.gap - card.loginRoom)
            model: page.content
            focus: true

            onActivated: function (index, row) {
                page.activate(index, row);
            }
            onEscapedLeft: Sound.play("edge")
        }

        LoginCard {
            id: qrCard

            x: card.pad
            y: rows.y + rows.height + card.gap
            width: parent.width - card.pad * 2
            source: page.form.stepId === "stores" ? page.source : ""
        }
    }
}
