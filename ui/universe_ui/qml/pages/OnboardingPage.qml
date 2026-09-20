import QtQuick
import "../core"
import "../sound"
import "../ui"

// First-run setup: one card list per step (discover, stores, import, preferences, done), X moves on, B goes back or skips.
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

    readonly property real sideMargin: Theme.dp(90)

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
        if (form.step > 0) {
            Sound.cancel();
            form.back();
        } else {
            Sound.cancel();
            form.finish();
        }
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

    Item {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: Theme.dp(88)

        Column {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(4)

            Row {
                spacing: Theme.dp(14)

                CapsLabel {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "SET UP · " + (page.form.step + 1) + " OF " + page.form.steps.length
                }

                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.dp(6)

                    Repeater {
                        model: page.form.steps.length

                        Rectangle {
                            required property int index
                            width: Theme.dp(index === page.form.step ? 22 : 8)
                            height: Theme.dp(8)
                            radius: height / 2
                            color: index <= page.form.step ? Theme.text : Qt.rgba(1, 1, 1, 0.18)

                            Behavior on width {
                                Ease {}
                            }
                        }
                    }
                }
            }

            Text {
                width: parent.width
                text: page.current.title
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(42)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: page.form.busy && page.form.count === 0 ? "Looking at this machine…" : page.current.subtitle
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                elide: Text.ElideRight
            }
        }
    }

    SettingsCards {
        id: cards

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(40)
        anchors.bottom: loginCard.visible ? loginCard.top : hintBar.top
        anchors.bottomMargin: loginCard.visible ? Theme.dp(24) : 0
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
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

        anchors.bottom: hintBar.top
        anchors.bottomMargin: Theme.dp(24)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        source: page.form.stepId === "stores" ? page.source : ""
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        z: 3
        sideMargin: page.sideMargin
        hints: page.hints
    }

    ValueEditor {
        id: editor

        anchors.fill: parent
        cards: cards
        overhang: 0
        floor: hintBar.y
        z: 2

        onClosed: cards.forceActiveFocus()
    }
}
