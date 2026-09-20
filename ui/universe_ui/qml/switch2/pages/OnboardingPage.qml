import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Forms.js" as Forms

// First-run setup as a dialog over the shell: one row list per step (found, stores, preferences, done) over a Back / Continue button pair.
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

    readonly property string backLabel: form.step > 0 ? "Back" : "Skip setup"
    readonly property string nextLabel: last ? "Finish" : "Continue"
    readonly property var hints: {
        var row = rows.currentRow;
        var label = nav.activeFocus ? (nav.index === 1 ? nextLabel : backLabel) : !row || row.heading || row.type === "info" || row.type === "static" ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? row.action || "Select" : "Change";
        return [
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
                nav.index = 1;
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
        readonly property real navRoom: nav.height + gap

        anchors.horizontalCenter: parent.horizontalCenter
        y: (parent.height - Theme.dp(Theme.hintBarHeight) - height) / 2
        width: Theme.dp(1200)
        height: pad * 2 + head.height + gap + rows.room + rows.height + loginRoom + navRoom
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
            y: head.y + head.height + card.gap + rows.room
            width: parent.width - card.pad * 2
            height: Math.min(rows.contentHeight + rows.room * 2, card.room - card.pad * 2 - head.height - card.gap - rows.room - card.loginRoom - card.navRoom)
            model: page.content
            focus: true

            onActivated: function (index, row) {
                page.activate(index, row);
            }
            onEscapedLeft: Sound.play("edge")
            onEscapedDown: {
                Sound.play("tick");
                nav.forceActiveFocus();
            }
        }

        LoginCard {
            id: qrCard

            x: card.pad
            y: rows.y + rows.height + card.gap
            width: parent.width - card.pad * 2
            source: page.form.stepId === "stores" ? page.source : ""
        }

        FocusScope {
            id: nav

            property int index: 1

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: Theme.dp(113)

            Hairline {
                anchors.bottom: parent.top
            }

            Repeater {
                model: [
                    {
                        glyph: "B",
                        label: page.backLabel
                    },
                    {
                        glyph: "X",
                        label: page.nextLabel
                    }
                ]

                Item {
                    id: button

                    readonly property bool focused: nav.activeFocus && index === nav.index

                    x: index * width
                    width: nav.width / 2
                    height: nav.height

                    Rectangle {
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: 1
                        visible: index > 0
                        color: Theme.hairlineSoft
                    }

                    FocusPill {
                        anchors.fill: parent
                        anchors.margins: Theme.dp(Theme.ringRoomTight)
                        focused: button.focused
                    }

                    Row {
                        anchors.centerIn: parent
                        spacing: Theme.dp(16)

                        HintGlyph {
                            anchors.verticalCenter: parent.verticalCenter
                            glyph: modelData.glyph
                            unit: Theme.dp(36)
                        }

                        Label {
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.label
                            color: Theme.accent
                        }
                    }
                }
            }

            Keys.onLeftPressed: index = Sound.stepped(index, -1, 2)
            Keys.onRightPressed: index = Sound.stepped(index, 1, 2)
            Keys.onUpPressed: {
                Sound.play("tick");
                rows.forceActiveFocus();
            }
            Keys.onDownPressed: Sound.play("edge")
            Keys.onPressed: function (event) {
                if (event.isAutoRepeat || shell.modal)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    nav.index === 1 ? page.advance() : page.retreat();
                }
            }
        }
    }
}
