import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../ui/Wifi.js" as Wifi

// First-run setup as a dialog over the launcher: one card list per step (found, stores, preferences, done) over a Back / Continue button pair.
FocusScope {
    id: page

    objectName: "onboardingPage"
    focus: true

    property var args: ({})
    readonly property var form: api.screens.onboarding
    readonly property var login: api.screens.login
    readonly property bool last: form.step >= form.steps.length - 1
    property string source: ""
    // A store's sign-in card is up: the list keeps that store's rows alone, so they fit beside it.
    readonly property bool signing: loginCard.visible

    signal closeRequested
    signal message(string text)

    readonly property string backLabel: form.step > 0 ? "Back" : form.added ? "Close" : "Skip setup"
    readonly property string nextLabel: last ? "Finish" : "Continue"
    readonly property string acceptLabel: nav.activeFocus ? (nav.index === 1 ? nextLabel : backLabel) : selectLabel(cards.currentRow)
    readonly property var hints: prompts.open ? prompts.hints : editor.open ? editor.hints : confirm.open ? confirm.hints : [acceptLabel !== "" && {
            glyph: "A",
            label: acceptLabel,
            dim: !nav.activeFocus && !cards.currentRow
        },
        {
            glyph: "B",
            label: backLabel
        },
        {
            glyph: "X",
            label: nextLabel
        }
    ].filter(Boolean)

    Component.onCompleted: form.load()

    function selectLabel(row) {
        if (!row)
            return "Select";
        if (row.type === "action")
            return row.action || "Select";
        return row.type === "bool" ? "Toggle" : row.type === "info" || row.type === "static" ? "" : "Change";
    }

    function activate(index, row) {
        if (row.type === "info" || row.type === "static") {
            Sound.edge();
        } else if (row.key === "link") {
            Sound.enter();
            page.source = row.module;
            login.begin(row.module);
        } else if (row.key === "code") {
            Sound.panel();
            page.source = row.module;
            editor.prompt(row.prompt, "", function (code) {
                login.submit(code);
            });
        } else if (row.key === "network") {
            joinNetwork(row);
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

    // The network this step asked to join: its outcome is told here.
    property string joining: ""

    function joinNetwork(row) {
        var wifi = api.screens.network;
        var step = Wifi.step(wifi, row);
        if (step === "refuse") {
            Sound.edge();
            page.message(Wifi.refusal(row));
        } else if (step === "forget") {
            Sound.edge();
        } else if (step === "password") {
            askPassword(row.ssid);
        } else if (wifi.join(row.ssid, "")) {
            Sound.enter();
            joining = row.ssid;
        } else {
            Sound.edge();
        }
    }

    function askPassword(ssid) {
        Sound.panel();
        prompts.prompt(Wifi.passwordTitle(ssid), "", function (password) {
            if (password === "")
                return;
            page.joining = ssid;
            api.screens.network.join(ssid, password);
        }, "secret");
    }

    Connections {
        target: api.screens.network
        function onJoined(ssid, connectivity) {
            if (ssid !== page.joining)
                return;
            page.joining = "";
            page.message(Wifi.joined(ssid, connectivity));
        }
        function onFailed(ssid, reason, message) {
            if (ssid !== page.joining)
                return;
            page.joining = "";
            if (reason !== "password") {
                page.message(message);
                return;
            }
            confirm.ask({
                message: ssid + " did not take that password",
                detail: "",
                no: "Cancel",
                yes: "Try Again"
            }, function (yes) {
                if (yes)
                    page.askPassword(ssid);
                else
                    cards.forceActiveFocus();
            });
        }
    }

    function advance() {
        Sound.enter();
        form.next();
    }

    // B on the first step asks before the setup is skipped, unless games came in: then it only closes.
    function retreat() {
        if (form.step > 0 || form.added) {
            Sound.cancel();
            form.step > 0 ? form.back() : form.finish();
            return;
        }
        var back = nav.activeFocus ? nav : cards;
        confirm.ask({
            message: "Skip setup?",
            detail: "It runs again from Settings › About whenever you like.",
            no: "Keep going",
            yes: "Skip",
            index: 0
        }, function (yes) {
            back.forceActiveFocus();
            if (yes)
                form.finish();
        });
    }

    function settle() {
        cards.reset();
        nav.index = 1;
        form.idle ? nav.forceActiveFocus() : cards.forceActiveFocus();
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat || editor.open || prompts.open || confirm.open)
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
            page.message(text);
        }
        function onFinished() {
            page.closeRequested();
        }
        function onStepChanged() {
            Qt.callLater(page.settle);
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

        // Keeps the mouse off the tabs beneath.
        HoverHandler {}
        TapHandler {}
    }

    Rectangle {
        id: panel

        readonly property real pad: Theme.dp(48)
        readonly property real gap: Theme.dp(24)
        readonly property real room: hintBar.y - Theme.dp(96)
        readonly property real loginRoom: loginCard.visible ? loginCard.height + gap : 0
        readonly property real navRoom: nav.height + gap
        // A store's heading and its three sign-in rows: what the list keeps while the card is up.
        readonly property real signRows: Theme.dp(300)
        readonly property real cardsRoom: room - pad * 2 - head.height - gap - loginRoom - navRoom

        anchors.horizontalCenter: parent.horizontalCenter
        y: (hintBar.y - height) / 2
        width: Math.min(Theme.dp(1100), parent.width - Theme.dp(180))
        height: pad * 2 + head.height + gap + cards.height + loginRoom + navRoom
        radius: Theme.dp(28)
        color: "#1b1d24"
        border.width: 1
        border.color: Theme.surfaceBorder

        HoverHandler {}
        TapHandler {}

        Column {
            id: head

            x: panel.pad
            y: panel.pad
            width: parent.width - panel.pad * 2
            spacing: Theme.dp(4)

            Text {
                width: parent.width
                visible: !page.form.loading && page.form.steps.length > 1
                text: (page.form.step + 1) + " / " + page.form.steps.length
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(19)
            }

            Text {
                width: parent.width
                text: page.form.title
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(36)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: page.form.loading ? "Looking at this machine…" : page.form.subtitle
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                elide: Text.ElideRight
            }
        }

        SettingsCards {
            id: cards

            objectName: "setupRows"
            x: panel.pad
            y: head.y + head.height + panel.gap
            width: parent.width - panel.pad * 2
            height: Math.min(cards.layout.height + cards.captionHeight, Math.max(page.signing ? panel.signRows : 0, panel.cardsRoom))
            focus: true
            columns: 1
            compact: true
            rows: page.form.rows
            groups: page.signing ? page.form.groups.filter(function (g) {
                return g.rows.some(function (i) {
                    return page.form.rows[i].module === page.source;
                });
            }) : page.form.groups
            dimmed: editor.open || prompts.open || confirm.open

            onActivated: function (index, row) {
                page.activate(index, row);
            }
            onEscapedUp: Sound.edge()
            onEscapedLeft: Sound.edge()
            onEscapedDown: {
                Sound.tick();
                nav.forceActiveFocus();
            }
        }

        LoginCard {
            id: loginCard

            x: panel.pad
            y: cards.y + cards.height + panel.gap
            width: parent.width - panel.pad * 2
            source: page.form.stepId === "stores" ? page.source : ""
            // The code shrinks before the rows of the store it signs in to are cut.
            qrSize: Math.max(Theme.dp(180), Math.min(Theme.dp(300), panel.room - panel.pad * 2 - head.height - panel.gap - panel.signRows - panel.navRoom - panel.gap - Theme.dp(150)))
        }

        FocusScope {
            id: nav

            property int index: 1

            objectName: "setupNav"
            x: panel.pad
            y: panel.height - panel.pad - height
            width: parent.width - panel.pad * 2
            height: buttons.height

            Row {
                id: buttons

                anchors.right: parent.right
                spacing: Theme.dp(24)

                PillButton {
                    ghost: true
                    icon: ""
                    label: page.backLabel
                    focused: nav.activeFocus && nav.index === 0
                    dimmed: nav.activeFocus && nav.index !== 0
                    onPicked: nav.pointTo(0)
                }

                PillButton {
                    icon: ""
                    label: page.nextLabel
                    focused: nav.activeFocus && nav.index === 1
                    dimmed: nav.activeFocus && nav.index !== 1
                    onPicked: nav.pointTo(1)
                }
            }

            function pointTo(i) {
                index = i;
                forceActiveFocus();
            }

            Keys.onLeftPressed: index = Sound.stepped(index, -1, 2)
            Keys.onRightPressed: index = Sound.stepped(index, 1, 2)
            Keys.onUpPressed: {
                Sound.tick();
                cards.forceActiveFocus();
            }
            Keys.onDownPressed: Sound.edge()
            Keys.onPressed: function (event) {
                if (event.isAutoRepeat)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    nav.index === 1 ? page.advance() : page.retreat();
                }
            }
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

    ConfirmDialog {
        id: confirm

        anchors.fill: parent
        z: 4
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
