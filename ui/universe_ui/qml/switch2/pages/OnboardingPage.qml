import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Forms.js" as Forms

// First-run setup as a dialog over the shell: one row list per step (found, stores, preferences, done) over a Back / Continue button pair.
FocusScope {
    id: page

    objectName: "onboardingPage"
    property var shell: null
    property var args: ({})
    readonly property bool overlay: true
    focus: true

    readonly property var form: api.screens.onboarding
    readonly property var login: api.screens.login
    readonly property bool last: form.step >= form.steps.length - 1
    property string source: ""
    // A store's sign-in card is up: the list keeps that store's rows alone, so they fit beside it.
    readonly property bool signing: qrCard.visible

    readonly property string backLabel: form.step > 0 ? "Back" : form.added ? "Close" : "Skip setup"
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

    readonly property var content: Forms.grouped(signing ? form.groups.filter(function (g) {
        return g.rows.some(function (i) {
            return form.rows[i].module === page.source;
        });
    }) : form.groups, form.rows, function (src, i) {
        return Object.assign({}, src, {
            form: i
        });
    })

    function activate(index, row) {
        if (row.type === "info" || row.type === "static") {
            Sound.play("edge");
        } else if (row.key === "link") {
            Sound.play("ok");
            page.source = row.module;
            login.begin(row.module);
        } else if (row.key === "code") {
            page.source = row.module;
            shell.prompt({
                title: row.prompt,
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

    // B on the first step asks before the setup is skipped, unless games came in: then it only closes.
    function retreat() {
        if (form.step > 0 || form.added) {
            Sound.play("back");
            form.step > 0 ? form.back() : form.finish();
            return;
        }
        shell.dialogAsk({
            message: "Skip setup?",
            detail: "It runs again from Settings › About whenever you like.",
            buttons: ["Keep going", "Skip"],
            index: 0
        }, function (i) {
            if (i === 1)
                form.finish();
        });
    }

    function settle() {
        rows.reset();
        nav.index = 1;
        form.idle ? nav.forceActiveFocus() : rows.forceActiveFocus();
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
            Qt.callLater(page.settle);
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

        // HOME stays drawn under the overlay, and live to a finger but for this.
        Block {}
    }

    Rectangle {
        id: card

        readonly property real pad: Theme.dp(56)
        readonly property real gap: Theme.dp(24)
        readonly property real room: parent.height - Theme.dp(Theme.hintBarHeight) - Theme.dp(120)
        readonly property real loginRoom: qrCard.visible ? qrCard.height + gap : 0
        readonly property real navRoom: nav.height + gap
        // A store's heading and two rows, the one in focus and the next: what the list keeps while the card is up.
        readonly property real signRows: rows.headingHeight + rows.rowHeight * 2
        readonly property real rowsRoom: room - pad * 2 - head.height - gap - rows.room - loginRoom - navRoom

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
                text: page.form.title
                font.pixelSize: Theme.dp(Theme.fontTitle)
                elide: Text.ElideRight
            }

            Label {
                width: parent.width
                visible: text !== ""
                text: page.form.loading ? "Looking at this machine…" : page.form.subtitle
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
                elide: Text.ElideRight
            }
        }

        SettingsRows {
            id: rows

            objectName: "setupRows"
            shell: page.shell
            x: card.pad
            y: head.y + head.height + card.gap + rows.room
            width: parent.width - card.pad * 2
            // Whole rows only: a row cut in half reads as a layout fault, the scrollbar says there is more.
            height: Math.min(rows.contentHeight + rows.room * 2, Math.max(page.signing ? card.signRows : 0, Math.floor((card.rowsRoom - rows.room * 2) / rows.rowHeight) * rows.rowHeight + rows.room * 2))
            // The settings page's bar hugs the screen's edge; the card's keeps to its margin.
            scrollbarRoom: card.pad / 2
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
            // The code shrinks before the rows of the store it signs in to are cut.
            qrSize: Math.max(Theme.dp(150), Math.min(Theme.dp(282), card.room - card.pad * 2 - head.height - card.gap - rows.room - card.signRows - card.navRoom - card.gap - Theme.dp(48)))
        }

        FocusScope {
            id: nav

            property int index: 1

            objectName: "setupNav"
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
