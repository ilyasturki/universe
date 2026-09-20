import QtQuick
import "../core"
import "../sound"
import "../ui"

// First-run setup as a dialog over the launcher: one card list per step (found, stores, preferences, done), X moves on, B goes back or skips.
FocusScope {
    id: page

    focus: true

    property var args: ({})
    readonly property var form: api.screens.onboarding
    readonly property var login: api.screens.login
    readonly property var current: form.steps[form.step] || {
        title: "",
        subtitle: ""
    }
    readonly property bool last: form.step >= form.steps.length - 1
    property string source: ""

    signal closeRequested
    signal message(string text)

    readonly property var hints: editor.open ? editor.hints : [
        {
            glyph: "A",
            label: selectLabel(cards.currentRow),
            dim: !cards.currentRow || form.busy || cards.currentRow.type === "info" || cards.currentRow.type === "static"
        },
        {
            glyph: "X",
            label: last ? "Finish" : "Continue"
        },
        {
            glyph: "B",
            label: form.step > 0 ? "Back" : "Skip setup"
        }
    ]

    Component.onCompleted: form.load()

    function selectLabel(row) {
        if (!row)
            return "Select";
        if (row.type === "action")
            return row.action || "Select";
        return row.type === "bool" ? "Toggle" : row.type === "info" || row.type === "static" ? "" : "Change";
    }

    function activate(index, row) {
        if (form.busy || row.type === "info" || row.type === "static") {
            Sound.edge();
        } else if (row.key === "link") {
            Sound.enter();
            page.source = row.module;
            login.begin(row.module);
        } else if (row.key === "code") {
            Sound.panel();
            page.source = row.module;
            editor.prompt("Code from " + row.section, "", function (code) {
                login.submit(code);
            });
        } else if (row.via !== undefined) {
            form.runImport(index) ? Sound.enter() : Sound.edge();
        } else if (row.type === "bool") {
            form.toggle(index);
            Sound.favourite(!row.value);
        } else {
            Sound.panel();
            editor.edit(row, function (value) {
                form.setValue(index, value);
            });
        }
    }

    function advance() {
        Sound.enter();
        form.next();
    }

    function retreat() {
        Sound.cancel();
        if (form.step > 0)
            form.back();
        else
            form.finish();
    }

    Connections {
        target: page.form
        function onMessage(text) {
            page.message(text);
        }
        function onFinished() {
            page.closeRequested();
        }
        function onStepChanged() {
            Qt.callLater(function () {
                cards.reset();
                cards.forceActiveFocus();
            });
        }
    }

    Connections {
        target: page.login
        function onFinished(ok, text) {
            page.message(text);
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(0.02, 0.02, 0.03, 1)
        opacity: 0.62
    }

    Rectangle {
        id: panel

        readonly property real pad: Theme.dp(48)
        readonly property real gap: Theme.dp(24)
        readonly property real room: hintBar.y - Theme.dp(96)
        readonly property real loginRoom: loginCard.visible ? loginCard.height + gap : 0

        anchors.horizontalCenter: parent.horizontalCenter
        y: (hintBar.y - height) / 2
        width: Math.min(Theme.dp(1100), parent.width - Theme.dp(180))
        height: pad * 2 + head.height + gap + cards.height + loginRoom
        radius: Theme.dp(28)
        color: "#1b1d24"
        border.width: 1
        border.color: Theme.surfaceBorder

        Column {
            id: head

            x: panel.pad
            y: panel.pad
            width: parent.width - panel.pad * 2
            spacing: Theme.dp(4)

            CapsLabel {
                text: (page.form.step + 1) + " / " + page.form.steps.length
            }

            Text {
                width: parent.width
                text: page.current.title
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(36)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: page.form.busy && page.form.count === 0 ? "Looking at this machine…" : page.current.subtitle
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                elide: Text.ElideRight
            }
        }

        SettingsCards {
            id: cards

            x: panel.pad
            y: head.y + head.height + panel.gap
            width: parent.width - panel.pad * 2
            height: Math.min(cards.layout.height, panel.room - panel.pad * 2 - head.height - panel.gap - panel.loginRoom)
            focus: true
            columns: 1
            compact: true
            rows: page.form.rows
            groups: page.form.groups
            dimmed: editor.open

            onActivated: function (index, row) {
                page.activate(index, row);
            }
            onEscapedUp: Sound.edge()
            onEscapedLeft: Sound.edge()

            Keys.onPressed: function (event) {
                if (event.isAutoRepeat)
                    return;
                if (api.keys.isCancel(event)) {
                    event.accepted = true;
                    page.retreat();
                } else if (api.keys.isDetails(event)) {
                    event.accepted = true;
                    page.advance();
                }
            }
        }

        LoginCard {
            id: loginCard

            x: panel.pad
            y: cards.y + cards.height + panel.gap
            width: parent.width - panel.pad * 2
            source: page.form.stepId === "stores" ? page.source : ""
        }

        ValueEditor {
            id: editor

            anchors.fill: parent
            cards: cards
            overhang: 0
            floor: panel.height
            z: 2

            onClosed: cards.forceActiveFocus()
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        z: 3
        hints: page.hints
    }
}
