import QtQuick
import "../core"
import "../core/Format.js" as Format
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    property var args: ({})
    readonly property var game: args.game || null
    readonly property string session: args.session || ""
    readonly property var store: api.screens.journal
    readonly property var rows: store.rows
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
    readonly property string currentSession: current ? current.session : ""
    readonly property var images: current ? current.images : []
    readonly property bool currentPending: current !== null && current.state === "pending"
    // An entry nobody has written yet: never journaled, put off after a failure, or given up on.
    readonly property bool currentBlank: current !== null && (current.state === "none" || current.state === "deferred" || current.state === "failed")
    readonly property bool currentWritten: current !== null && current.state === "written"
    // The sessions still without an entry are rows too, but they are not what the header counts.
    readonly property int written: rows.filter(function (r) {
        return r.state !== "none";
    }).length
    readonly property bool anyPending: rows.some(function (r) {
        return r.state === "pending";
    })
    property double now: Date.now()

    readonly property var recordings: api.screens.recordings
    readonly property var recording: {
        if (!current || !current.hasRecording)
            return null;
        var all = recordings.rows;
        return all.find(function (r) {
            return r.session === current.session;
        }) || null;
    }

    // 0 entries, 1 the text, 2 the screenshots, 3 the recording card
    property int mode: 0
    property int shotIndex: 0
    property bool lightbox: false
    readonly property bool reading: mode > 0

    signal closeRequested
    signal jumpRequested(string source, string session)

    readonly property var hints: {
        if (lightbox)
            return [
                {
                    glyph: "dpad",
                    label: "Previous / next"
                },
                {
                    glyph: "B",
                    label: "Close"
                }
            ];
        if (menu.open)
            return menu.hints;
        var out = [];
        if (mode === 3)
            out.push({
                glyph: "A",
                label: "Watch"
            });
        else if (mode === 2)
            out.push({
                glyph: "A",
                label: "View"
            });
        else if (mode === 1)
            out.push({
                glyph: "A",
                label: recording ? "Recording" : "Screenshots",
                dim: !recording && images.length === 0
            });
        else
            out.push({
                glyph: "A",
                label: currentPending ? "Being written" : currentBlank ? "Write it" : "Read",
                dim: currentPending
            });
        if (mode !== 3)
            out.push({
                glyph: "Y",
                label: "Recording",
                dim: !(current && current.hasRecording)
            });
        out.push({
            glyph: "Start",
            label: "More",
            dim: current === null
        });
        out.push({
            glyph: "B",
            label: reading ? "Back to entries" : "Back"
        });
        return out;
    }

    readonly property real sideMargin: Theme.dp(90)
    readonly property real listWidth: Theme.dp(560)

    onGameChanged: {
        index = 0;
        mode = 0;
        lightbox = false;
        if (game) {
            store.load(game.id);
            if (recordings.gameId !== game.id)
                recordings.load(game.id);
        }
        landOnSession();
    }

    onSessionChanged: landOnSession()
    onRowsChanged: {
        if (index >= rows.length)
            index = Math.max(0, rows.length - 1);
        landOnSession();
    }

    function landOnSession() {
        var i = rows.findIndex(function (r) {
            return r.session === session;
        });
        if (i >= 0)
            index = i;
    }

    onCurrentChanged: {
        flick.contentY = 0;
        shotIndex = 0;
    }

    onRecordingChanged: {
        if (mode === 3 && !recording)
            mode = 1;
    }

    function step(d) {
        index = Sound.stepped(index, d, rows.length);
    }

    function stepScreen(d) {
        var pitch = list.currentItem ? list.currentItem.height + list.spacing : 0;
        index = Sound.paged(index, d, 1, pitch > 0 ? Math.floor(list.height / pitch) : 1, rows.length);
    }

    function maxScroll() {
        return Math.max(0, flick.contentHeight - flick.height);
    }

    function scroll(d) {
        var next = Math.max(0, Math.min(maxScroll(), flick.contentY + d * Theme.dp(260)));
        if (next === flick.contentY) {
            d > 0 ? (recording ? openRecordingCard() : openShots()) : Sound.edge();
            return;
        }
        Sound.tick();
        flick.contentY = next;
    }

    function openShots() {
        if (images.length === 0) {
            Sound.edge();
            return;
        }
        Sound.panel();
        mode = 2;
        flick.contentY = maxScroll();
    }

    function openRecordingCard() {
        Sound.panel();
        mode = 3;
        flick.contentY = Math.max(0, Math.min(maxScroll(), recordingCard.y + recordingCard.height + Theme.dp(60) - flick.height));
    }

    function stepShot(d) {
        shotIndex = Sound.stepped(shotIndex, d, images.length);
    }

    function read() {
        if (!current || currentPending) {
            Sound.edge();
            return;
        }
        if (currentBlank) {
            write(false);
            return;
        }
        Sound.panel();
        mode = 1;
    }

    // Hands the session to the journal module: a first entry, another try, or a fresh one over what is there.
    function write(again) {
        if (!current || currentPending) {
            Sound.edge();
            return;
        }
        Sound.enter();
        store.write(current.gameId, current.session, again);
    }

    function elapsedText(startedAt) {
        var s = Math.round((page.now - Date.parse(startedAt)) / 1000);
        if (isNaN(s))
            return "";
        s = Math.max(0, s);
        if (s < 60)
            return s + " s";
        if (s < 3600)
            return Math.floor(s / 60) + " min";
        return Math.floor(s / 3600) + " h " + ("0" + Math.floor((s % 3600) / 60)).slice(-2);
    }

    function whenText(entry) {
        return entry.dateText + (entry.durationText !== "" ? "  ·  " + entry.durationText : "");
    }

    Timer {
        interval: 1000
        running: page.anyPending
        repeat: true
        onTriggered: page.now = Date.now()
    }

    function leave() {
        Sound.cancel();
        mode = 0;
    }

    function openRecording() {
        if (current && current.hasRecording)
            page.jumpRequested("pages/RecordingsPage.qml", current.session);
        else
            Sound.edge();
    }

    function rowRect() {
        var item = list.currentItem;
        return Qt.rect(item.x, item.y - list.contentY, item.width, item.height);
    }

    function openMenu() {
        if (!current || !list.currentItem) {
            Sound.edge();
            return;
        }
        Sound.panel();
        var items = [];
        if (currentWritten)
            items.push({
                icon: "book",
                label: "Read",
                action: "read"
            });
        if (currentBlank)
            items.push({
                icon: current.state === "none" ? "plus" : "refresh",
                label: current.state === "none" ? "Write the entry" : "Try again now",
                action: "write"
            });
        if (currentWritten)
            items.push({
                icon: "refresh",
                label: "Write it again",
                action: "rewrite"
            });
        if (current.hasRecording)
            items.push({
                icon: "film",
                label: "Recording",
                action: "recording"
            });
        if (current.state !== "none" || current.hasRecording)
            items.push({
                icon: "trash",
                label: currentPending ? "Cancel the writing…" : current.state === "none" ? "Remove the recording…" : "Remove entry…",
                action: "remove",
                danger: true
            });
        menu.show(items, list, rowRect(), current.title !== "" ? current.title : whenText(current), menuAction);
    }

    function menuAction(action) {
        if (action === "read") {
            read();
        } else if (action === "write") {
            write(false);
        } else if (action === "rewrite") {
            write(true);
        } else if (action === "recording") {
            openRecording();
        } else if (action === "remove") {
            Sound.panel();
            var blank = current.state === "none";
            var items = [
                {
                    icon: "",
                    label: "Keep it",
                    action: ""
                }
            ];
            if (!blank)
                items.push({
                    icon: "trash",
                    label: currentPending ? "Stop the writing" : "Trash the entry",
                    action: "remove!",
                    danger: true
                });
            if (current.hasRecording)
                items.push({
                    icon: "trash",
                    label: blank ? "Trash the recording" : currentPending ? "Stop it and trash the recording" : "Trash it and its recording",
                    action: "remove-both!",
                    danger: true
                });
            menu.show(items, list, rowRect(), currentPending ? "Cancel this entry?" : blank ? "Remove this recording?" : "Remove this entry?", menuAction);
        } else if (action === "remove!" || action === "remove-both!") {
            Sound.enter();
            var gameId = current.gameId, session = current.session, written = current.state !== "none";
            if (action === "remove-both!")
                api.screens.recordings.remove(gameId, session);
            if (written)
                store.remove(gameId, session);
            mode = 0;
        }
        if (action !== "remove")
            page.forceActiveFocus();
    }

    Keys.onPressed: function (event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (event.isAutoRepeat && !(lightbox && arrow) && !(mode === 2 && arrow) && !screen)
            return;

        event.accepted = true;
        if (lightbox) {
            if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                Sound.cancel();
                lightbox = false;
            } else if (arrow) {
                stepShot(event.key === Qt.Key_Left ? -1 : 1);
            }
        } else if (api.keys.isAccept(event)) {
            if (mode === 3) {
                openRecording();
            } else if (mode === 2) {
                Sound.enter();
                lightbox = true;
            } else if (mode === 1) {
                recording ? openRecordingCard() : openShots();
            } else {
                read();
            }
        } else if (api.keys.isCancel(event)) {
            reading ? leave() : page.closeRequested();
        } else if (api.keys.isFilters(event)) {
            openRecording();
        } else if (api.keys.isMenu(event)) {
            openMenu();
        } else if (event.key === Qt.Key_Up) {
            if (mode === 2 && recording) {
                openRecordingCard();
            } else if (mode >= 2) {
                Sound.panel();
                mode = 1;
            } else if (mode === 1) {
                scroll(-1);
            } else {
                step(-1);
            }
        } else if (event.key === Qt.Key_Down) {
            if (mode === 3)
                openShots();
            else if (mode === 2)
                Sound.edge();
            else if (mode === 1)
                scroll(1);
            else
                step(1);
        } else if (event.key === Qt.Key_Right) {
            mode === 2 ? stepShot(1) : mode >= 1 ? Sound.edge() : read();
        } else if (event.key === Qt.Key_Left) {
            mode === 2 ? stepShot(-1) : mode === 1 ? leave() : Sound.edge();
        } else if (screen) {
            mode === 0 ? stepScreen(screen) : Sound.edge();
        } else if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            mode === 0 ? (index = Sound.stepped(index, api.keys.isFirst(event) ? -rows.length : rows.length, rows.length)) : Sound.edge();
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
        label: "JOURNAL"
        detail: page.written > 0 ? Format.plural(page.written, "entry", "entries") : ""
    }

    Text {
        anchors.centerIn: parent
        visible: page.rows.length === 0
        text: "No journal entries yet — one is written after each session."
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

            readonly property bool pending: modelData.state === "pending"
            readonly property bool blank: modelData.state === "none"

            width: list.width
            height: Theme.dp(96)
            lit: index === page.index && !page.reading
            muted: pending || blank
            title: pending ? "Writing the entry…" : blank ? "No entry yet" : modelData.title
            subtitle: {
                if (pending)
                    return page.elapsedText(modelData.started_at);
                if (modelData.state === "failed")
                    return modelData.reason;
                if (modelData.state === "deferred")
                    return modelData.reason + "  ·  another try " + modelData.retryText;
                return page.whenText(modelData);
            }
            mark: "film"
            showMark: modelData.hasRecording
            leadWidth: pending ? Theme.dp(12) : 0
            gap: Theme.dp(16)

            Pointer {
                current: lit
                radius: Theme.dp(14)
                onPicked: {
                    page.mode = 0;
                    page.index = index;
                }
            }

            PulseDot {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(12)
                visible: entry.pending
                running: entry.pending
                color: entry.lit ? Theme.onLight : Theme.text
            }
        }
    }

    Flickable {
        id: flick

        readonly property real room: shots.inset

        anchors.top: list.top
        anchors.bottom: hintBar.top
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(60) - room
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        contentWidth: width
        contentHeight: article.height + Theme.dp(60)
        interactive: false
        clip: true
        visible: page.current !== null

        Behavior on contentY {
            id: articleEase
            Ease {
                duration: Theme.durView
            }
        }

        // A click on the article reads it; the wheel scrolls the text. Its cards below take the mode of their own.
        Pointer {
            accept: false
            wash: 0
            onPicked: if (page.current && !page.currentPending && page.mode !== 1)
                page.mode = 1
        }

        Wheel {
            ease: articleEase
        }

        Column {
            id: article

            x: flick.room
            width: flick.width - flick.room
            spacing: Theme.dp(28)

            Text {
                width: parent.width
                text: page.current ? (page.currentPending ? "Writing the entry…" : page.current.state === "none" ? "No entry yet" : page.current.title) : ""
                color: page.currentPending || page.currentBlank ? Theme.textSecondary : Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(38)
                wrapMode: Text.WordWrap
            }

            CapsLabel {
                text: page.current ? page.whenText(page.current) : ""
                tracking: 0.11
            }

            Text {
                width: parent.width
                visible: page.currentPending || page.currentBlank
                text: {
                    if (!page.current)
                        return "";
                    if (page.currentPending)
                        return "The journal module is writing this entry — " + page.elapsedText(page.current.started_at) + " so far. It shows up here when it is done.";
                    if (page.current.state === "deferred")
                        return page.current.reason + ". Another try " + page.current.retryText + ", or ask for one now with Start.";
                    if (page.current.state === "failed")
                        return page.current.reason + ". Ask for another try with Start.";
                    return "This session was never journaled. Start writes its entry from the recording and the screenshots you took.";
                }
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(24)
                lineHeight: 1.5
                wrapMode: Text.WordWrap
            }

            Repeater {
                // A row with no entry says its piece above; its one paragraph is that same reason.
                model: page.current && !page.currentBlank ? page.current.blocks : []

                Text {
                    width: article.width
                    text: modelData
                    textFormat: Text.MarkdownText
                    color: page.mode === 1 ? Theme.text : Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(24)
                    lineHeight: 1.5
                    wrapMode: Text.WordWrap

                    Behavior on color {
                        ColorEase {
                            duration: Theme.durBase
                        }
                    }
                }
            }

            Column {
                width: parent.width
                spacing: Theme.dp(10)
                visible: page.current && page.current.next_up !== ""

                CapsLabel {
                    text: "NEXT UP"
                    tracking: 0.11
                }

                Text {
                    width: parent.width
                    text: page.current ? page.current.next_up : ""
                    textFormat: Text.MarkdownText
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Medium
                    font.pixelSize: Theme.dp(24)
                    lineHeight: 1.4
                    wrapMode: Text.WordWrap
                }
            }

            RecordingCard {
                id: recordingCard

                width: parent.width
                recording: page.recording
                focused: page.mode === 3
                dimmed: page.mode === 2 && !page.lightbox
                onPicked: page.mode = 3
            }

            ScreenshotStrip {
                id: shots

                width: parent.width
                images: page.images
                index: page.shotIndex
                focused: page.mode === 2 && !page.lightbox
                sideMargin: page.sideMargin
                onPointed: function (i) {
                    page.mode = 2;
                    page.shotIndex = i;
                }
            }
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: flick.left
        anchors.leftMargin: flick.room
        anchors.right: parent.right
        height: hintBar.height + Theme.dp(50)
        gradient: Gradient {
            GradientStop {
                position: 0.0
                color: Qt.rgba(0.055, 0.059, 0.075, 0.0)
            }
            GradientStop {
                position: 0.45
                color: Qt.rgba(0.055, 0.059, 0.075, 0.92)
            }
            GradientStop {
                position: 1.0
                color: Theme.ground
            }
        }
    }

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(52)
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        flickable: flick
        shown: page.mode === 1
        to: recordingCard.y
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        hints: page.hints
    }

    Lightbox {
        anchors.fill: parent
        images: page.images
        index: page.shotIndex
        open: page.lightbox
    }

    ActionMenu {
        id: menu

        anchors.fill: parent
        z: 5

        onDismissed: page.forceActiveFocus()
    }
}
