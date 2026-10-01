import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Forms.js" as Forms

// First-run setup, as the console's setup screens: a centred column over the system background, the step's rows,
// Back and Continue under them.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    focus: true

    readonly property bool strip: true
    readonly property var form: api.screens.onboarding
    readonly property var login: api.screens.login
    readonly property var current: form.steps[form.step] || {
        title: "",
        subtitle: ""
    }
    readonly property bool last: form.step >= form.steps.length - 1
    property string source: ""

    readonly property string backLabel: form.step > 0 ? "Back" : "Skip Setup"
    readonly property string nextLabel: last ? "Finish" : "Continue"
    readonly property var hints: {
        var row = rows.currentRow;
        var label = nav.activeFocus ? (nav.index === 1 ? nextLabel : backLabel) : !row || row.heading || row.type === "info" || row.type === "static" ? "OK" : row.type === "bool" ? "Toggle" : row.type === "action" ? row.action || "Select" : "Change";
        return [
            {
                glyph: "B",
                label: backLabel
            },
            {
                glyph: "X",
                label: nextLabel
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

    Backdrop {
        anchors.fill: parent
    }

    Item {
        id: column

        readonly property real gap: Theme.dp(28)
        readonly property real floor: page.height - Theme.dp(96)
        readonly property real loginRoom: qrCard.visible ? qrCard.height + gap : 0

        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.min(Theme.dp(1100), page.width - Theme.dp(Theme.edge * 2))
        height: page.height

        Row {
            id: steps
            anchors.horizontalCenter: parent.horizontalCenter
            y: Theme.dp(92)
            spacing: Theme.dp(12)

            Repeater {
                model: page.form.steps.length

                Rectangle {
                    width: index === page.form.step ? Theme.dp(34) : Theme.dp(10)
                    height: Theme.dp(10)
                    radius: height / 2
                    color: index <= page.form.step ? Theme.text : Qt.rgba(1, 1, 1, 0.25)

                    Behavior on width {
                        NumberAnimation {
                            duration: Theme.durPage
                            easing.type: Easing.OutCubic
                        }
                    }
                }
            }
        }

        Label {
            id: title
            anchors.horizontalCenter: parent.horizontalCenter
            y: steps.y + Theme.dp(46)
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: page.current.title
            elide: Text.ElideRight
            font.weight: Font.Light
            font.pixelSize: Theme.dp(52)
        }

        Label {
            id: subtitle
            anchors.horizontalCenter: parent.horizontalCenter
            y: title.y + title.height + Theme.dp(14)
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            visible: text !== ""
            text: page.form.busy && page.form.count === 0 ? "Looking at this machine…" : page.current.subtitle
            color: Theme.textSecondary
            elide: Text.ElideRight
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        readonly property real rowsTop: subtitle.y + subtitle.height + Theme.dp(44)

        SettingsRows {
            id: rows

            shell: page.shell
            x: Theme.dp(16)
            y: column.rowsTop
            width: parent.width - Theme.dp(32)
            height: Math.max(Theme.dp(98), Math.min(rows.contentHeight + Theme.dp(8), nav.y - column.gap - column.loginRoom - y))
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

        SettingsLoginCard {
            id: qrCard

            x: rows.x
            y: rows.y + rows.height + column.gap
            width: rows.width
            source: page.form.stepId === "stores" ? page.source : ""
        }

        FocusScope {
            id: nav

            property int index: 1

            anchors.horizontalCenter: parent.horizontalCenter
            y: column.floor - height - Theme.dp(24)
            width: buttons.width
            height: buttons.height

            Row {
                id: buttons
                spacing: Theme.dp(28)

                Repeater {
                    model: [page.backLabel, page.nextLabel]

                    PillButton {
                        text: modelData
                        width: Theme.dp(320)
                        focused: nav.activeFocus && index === nav.index
                        direct: true
                        onPicked: {
                            nav.index = index;
                            nav.forceActiveFocus();
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
                if (event.isAutoRepeat || page.shell.modal)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    nav.index === 1 ? page.advance() : page.retreat();
                }
            }
        }
    }
}
