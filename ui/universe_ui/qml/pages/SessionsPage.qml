import QtQuick
import "../core"
import "../core/Format.js" as Format
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "sessionsPage"
    focus: true

    property var args: ({})
    readonly property var game: args.game || null
    readonly property string landing: args.session || ""
    readonly property var store: api.screens.sessions
    readonly property var rows: store.rows
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
    readonly property var log: store.log
    // 0 the sessions, 1 the log
    property int mode: 0
    readonly property bool reading: mode === 1

    signal closeRequested

    readonly property var hints: [
        {
            glyph: "A",
            label: "Read the log",
            dim: current === null || reading
        },
        {
            glyph: "B",
            label: reading ? "Back to sessions" : "Back"
        }
    ]

    readonly property real sideMargin: Theme.dp(90)
    readonly property real listWidth: Theme.dp(560)

    Component.onDestruction: store.unload()

    onGameChanged: {
        index = 0;
        mode = 0;
        opened = "";
        if (game)
            store.load(game.id);
        landOnSession();
    }

    onRowsChanged: {
        if (index >= rows.length)
            index = Math.max(0, rows.length - 1);
        landOnSession();
    }

    function landOnSession() {
        if (landing === "")
            return;
        var i = rows.findIndex(function (r) {
            return r.session === landing;
        });
        if (i >= 0)
            index = i;
    }

    // The highlighted row's log follows the highlight; the running session's key is "" on the store's side, "live" here.
    readonly property string wanted: current ? (current.live ? "live" : current.session) : ""
    property string opened: ""

    onWantedChanged: open()

    function open() {
        if (wanted === "" || wanted === opened)
            return;
        opened = wanted;
        store.openLog(current.session);
    }

    function step(d) {
        if (rows.length === 0) {
            Sound.edge();
            return;
        }
        var next = Math.max(0, Math.min(rows.length - 1, index + d));
        if (next === index) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
    }

    function maxScroll() {
        return Math.max(0, flick.contentHeight - flick.height);
    }

    function scroll(d) {
        var next = Math.max(0, Math.min(maxScroll(), flick.contentY + d * Theme.dp(300)));
        if (next === flick.contentY) {
            Sound.edge();
            return;
        }
        Sound.tick();
        flick.contentY = next;
    }

    Keys.onPressed: function (event) {
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (event.isAutoRepeat && !vertical && !screen)
            return;
        event.accepted = true;
        if (api.keys.isCancel(event)) {
            if (reading) {
                Sound.cancel();
                mode = 0;
            } else {
                page.closeRequested();
            }
        } else if (api.keys.isAccept(event)) {
            if (current && !reading) {
                Sound.enter();
                mode = 1;
                flick.contentY = maxScroll();
            } else {
                Sound.edge();
            }
        } else if (vertical) {
            reading ? scroll(event.key === Qt.Key_Up ? -1 : 1) : step(event.key === Qt.Key_Up ? -1 : 1);
        } else if (screen) {
            reading ? scroll(screen * 3) : step(screen * 5);
        } else if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            var far = api.keys.isFirst(event) ? -1 : 1;
            reading ? scroll(far * 1000000) : step(far * rows.length);
        } else {
            event.accepted = false;
        }
    }

    GameBackdrop {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
    }

    GameHeader {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        game: page.game
        label: "SESSIONS"
        detail: page.rows.length > 0 ? Format.plural(page.rows.length, "session", "sessions") : ""
    }

    Text {
        anchors.centerIn: parent
        visible: page.rows.length === 0
        text: "Not played yet — every session and what the game wrote will show here."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    ListView {
        id: list

        Wheel {
            step: Theme.dp(96) + list.spacing
        }

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(32)
        anchors.bottom: hintBar.top
        anchors.left: parent.left
        anchors.leftMargin: page.sideMargin
        width: page.listWidth
        model: page.rows
        currentIndex: page.index
        interactive: false
        clip: true
        spacing: Theme.dp(12)
        opacity: page.reading ? 0.55 : 1.0
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: 0
        preferredHighlightEnd: height
        highlightRangeMode: ListView.ApplyRange

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }

        delegate: SessionRow {
            id: entry

            width: list.width
            height: Theme.dp(96)
            lit: index === page.index && !page.reading
            title: modelData.live ? "Playing now" : modelData.dateText
            subtitle: modelData.live ? Format.lastPlayed(modelData.started_at) : modelData.durationText + "  ·  " + modelData.endText
            mark: "film"
            showMark: modelData.hasRecording
            leadWidth: modelData.live || modelData.bad ? Theme.dp(12) : 0
            gap: Theme.dp(16)

            Pointer {
                current: entry.lit
                radius: Theme.dp(14)
                onPicked: {
                    page.mode = 0;
                    page.index = index;
                }
            }

            PulseDot {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(12)
                visible: modelData.live
                running: modelData.live
                color: entry.lit ? Theme.onLight : Theme.text
            }

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(12)
                height: width
                radius: width / 2
                visible: modelData.bad
                color: "#e5484d"
            }
        }
    }

    Rectangle {
        id: pane

        anchors.top: list.top
        anchors.bottom: hintBar.top
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(48)
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        radius: Theme.dp(16)
        color: Qt.rgba(0, 0, 0, 0.35)
        border.color: page.reading ? Theme.text : Theme.surfaceBorder
        border.width: page.reading ? Theme.dp(2) : 1
        visible: page.current !== null

        Behavior on border.color {
            ColorEase {}
        }

        // A click on the log reads it; the wheel scrolls the lines.
        Pointer {
            accept: false
            wash: 0
            onPicked: page.mode = 1
        }

        Text {
            anchors.centerIn: parent
            visible: page.store.logLoading || page.store.logError !== "" || (page.log.length === 0 && !page.store.logLoading)
            text: page.store.logLoading ? "Reading…" : page.store.logError !== "" ? page.store.logError : "The journal no longer holds this session."
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(24)
        }

        Flickable {
            id: flick

            anchors.fill: parent
            anchors.margins: Theme.dp(24)
            contentWidth: width
            contentHeight: lines.height
            interactive: false
            clip: true
            visible: page.log.length > 0 && !page.store.logLoading

            Behavior on contentY {
                id: logEase
                Ease {
                    duration: Theme.durView
                }
            }

            Wheel {
                ease: logEase
            }

            Column {
                id: lines
                width: flick.width
                spacing: Theme.dp(4)

                Repeater {
                    model: page.log

                    Row {
                        width: lines.width
                        spacing: Theme.dp(14)

                        Text {
                            width: Theme.dp(92)
                            text: modelData.time
                            color: Theme.textFaint
                            font.family: "monospace"
                            font.pixelSize: Theme.dp(19)
                        }

                        Text {
                            width: Theme.dp(150)
                            text: modelData.source
                            color: modelData.error ? "#f0757a" : modelData.warning ? "#f0c674" : Theme.textSecondary
                            font.family: "monospace"
                            font.pixelSize: Theme.dp(19)
                            elide: Text.ElideRight
                        }

                        Text {
                            width: parent.width - Theme.dp(92 + 150 + 28)
                            text: modelData.message
                            color: modelData.error ? "#f0757a" : Theme.text
                            font.family: "monospace"
                            font.pixelSize: Theme.dp(19)
                            wrapMode: Text.WrapAnywhere
                        }
                    }
                }
            }
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        hints: page.hints
    }
}
