import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    // { game } | { runner } | { module } | { source }: one game's, one runner's, one module's or one source's settings.
    property var args: ({})
    readonly property var game: args.game || null
    readonly property string runner: args.runner || ""
    readonly property string module: args.module || ""
    readonly property string source: args.source || ""
    readonly property var form: formOf(args)
    readonly property var info: game ? null : form.info
    readonly property var login: api.screens.login

    function formOf(a) {
        return a.game ? api.screens.gameSettings : a.runner ? api.screens.runner : a.source ? api.screens.source : api.screens.module;
    }

    signal closeRequested()
    signal message(string text)

    readonly property var hints: editor.open ? editor.hints
        : [ { glyph: "A", label: cards.currentRow && cards.currentRow.type === "bool" ? "Toggle"
                                : cards.currentRow && cards.currentRow.key === "add_file" ? "Pick a file"
                                : cards.currentRow && cards.currentRow.type === "action" ? cards.currentRow.action || "Select" : "Change",
              dim: !cards.currentRow || cards.currentRow.disabled === true || cards.currentRow.type === "info" },
            { glyph: "dpad", label: "Navigate" },
            { glyph: "B", label: "Back" } ]

    readonly property real sideMargin: Theme.dp(90)
    readonly property bool hasLogo: info !== null && info.icon !== undefined && String(info.icon) !== "" && logo.status === Image.Ready

    // The derived game/runner/module/source are still stale here: read the args themselves.
    onArgsChanged: {
        var id = args.game ? args.game.id : args.runner || args.module || args.source || "";
        if (id !== "")
            formOf(args).load(id);
    }

    function activate(index, row) {
        if (row.disabled === true || row.type === "info") {
            Sound.edge();
        } else if (row.type === "bool") {
            form.toggle(index);
            Sound.favourite(!row.value);
        } else if (row.key === "link" && source !== "") {
            Sound.enter();
            login.begin(source);
        } else if (row.key === "code" && source !== "") {
            Sound.panel();
            editor.prompt("Code from " + (info ? info.name : source), "", function(code) { login.submit(code); });
        } else if (row.key === "add_file") {
            Sound.panel();
            editor.edit({ type: "path", key: "add_file", label: "Game file for " + info.name, value: "" }, function(path) {
                if (form.setValue(index, path))
                    editor.prompt("Title of the game", form.pendingTitle(), function(title) {
                        form.addGame(title) !== "" ? Sound.enter() : Sound.edge();
                    });
            });
        } else {
            Sound.panel();
            editor.edit(row, function(value) { form.setValue(index, value); });
        }
    }

    Connections {
        target: page.form
        ignoreUnknownSignals: true
        function onMessage(text) { page.message(text); }
    }

    Connections {
        target: page.login
        function onFinished(ok, text) { page.message(text); }
    }

    GameBackdrop {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        visible: page.game !== null
        game: page.game
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

        GameHeader {
            anchors.fill: parent
            visible: page.game !== null
            game: page.game
            label: "GAME SETTINGS"
        }

        Image {
            id: logo
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            height: parent.height
            width: height
            source: page.info && page.info.icon ? Qt.resolvedUrl("../" + page.info.icon) : ""
            asynchronous: true
            fillMode: Image.PreserveAspectFit
            sourceSize.height: 256
            smooth: true
            mipmap: true
            visible: page.hasLogo
        }

        Column {
            anchors.left: parent.left
            anchors.leftMargin: page.hasLogo ? logo.width + Theme.dp(28) : 0
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(4)
            visible: page.game === null

            CapsLabel {
                text: page.runner !== "" ? "RUNNER" : page.source !== "" ? "SOURCE" : "MODULE"
            }

            Text {
                width: parent.width
                text: page.info ? page.info.name || "" : ""
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(42)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                visible: text !== ""
                text: page.info ? (page.info.meta || "")
                      + (page.info.warning
                         ? (page.info.meta ? " · " : "") + "<font color=\"#e0655a\">" + page.info.warning + "</font>"
                         : "") : ""
                textFormat: Text.StyledText
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
        anchors.topMargin: Theme.dp(page.game ? 32 : 40)
        anchors.bottom: loginCard.visible ? loginCard.top : hintBar.top
        anchors.bottomMargin: loginCard.visible ? Theme.dp(24) : 0
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        focus: true
        compact: true
        rows: page.form.rows
        groups: page.form.groups
        dimmed: editor.open

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedUp: Sound.edge()
        onEscapedLeft: Sound.edge()

        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                page.closeRequested();
            }
        }
    }

    // The sign-in link of a source, as a QR code and the URL, under the cards once asked for.
    Item {
        id: loginCard

        anchors.bottom: hintBar.top
        anchors.bottomMargin: Theme.dp(24)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: Math.max(Theme.dp(74), loginHead.height) + loginBody.height + Theme.dp(8) * 2 + 2
        visible: page.source !== "" && page.login.source === page.source && (page.login.url !== "" || page.login.status !== "")

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(24)
            color: Qt.rgba(1, 1, 1, 0.04)
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.10)
        }

        Text {
            id: loginHead
            x: Theme.dp(8) + 1 + Theme.dp(18)
            y: Theme.dp(8) + 1
            height: Theme.dp(74)
            verticalAlignment: Text.AlignVCenter
            text: "Sign-in link"
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(27)
        }

        Row {
            id: loginBody

            x: Theme.dp(8) + 1 + Theme.dp(16)
            y: loginHead.y + loginHead.height
            width: parent.width - x * 2
            height: Math.max(qr.height, loginText.height) + Theme.dp(36)
            spacing: Theme.dp(28)

            QrCode {
                id: qr
                y: Theme.dp(18)
                width: Theme.dp(300)
                height: width
                matrix: page.login.matrix
                visible: page.login.url !== ""
            }

            Column {
                id: loginText
                y: Theme.dp(18)
                width: parent.width - (qr.visible ? qr.width + parent.spacing : 0)
                spacing: Theme.dp(14)

                Text {
                    width: parent.width
                    text: "Scan to sign in on your phone"
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Medium
                    font.pixelSize: Theme.dp(22)
                }

                Text {
                    width: parent.width
                    text: page.login.url
                    color: Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(18)
                    lineHeight: 1.3
                    wrapMode: Text.WrapAnywhere
                    maximumLineCount: 5
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: page.login.status
                    color: Theme.textMuted
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(20)
                    wrapMode: Text.WordWrap
                }
            }
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        z: 3
        sideMargin: page.sideMargin
        showClock: true
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
