import QtQuick
import QtMultimedia
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
    readonly property var store: api.screens.recordings
    readonly property var rows: store.rows
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
    readonly property string currentSession: current ? current.session : ""
    readonly property var frames: current && store.frameMap[current.session] ? store.frameMap[current.session] : null

    readonly property var journal: api.screens.journal
    readonly property var entry: {
        if (!current || !current.hasJournal)
            return null;
        var all = journal.rows;
        return all.find(function (r) {
            return r.session === current.session;
        }) || null;
    }
    readonly property bool entryPending: entry !== null && entry.state === "pending"

    property bool videoFocused: false
    property bool journalFocused: false
    property bool fullscreen: false
    // The row's cached thumbnail, shown large in the pane once the cursor has rested on it.
    property string posterSource: ""

    Timer {
        id: rest
        interval: 200
        onTriggered: page.posterSource = page.frames && page.frames.thumbnail ? page.frames.thumbnail : ""
    }
    readonly property bool playing: player.playbackState === MediaPlayer.PlayingState
    readonly property bool stopped: player.playbackState === MediaPlayer.StoppedState
    readonly property real duration: player.duration > 0 ? player.duration : frames && frames.duration > 0 ? frames.duration * 1000 : current ? current.duration_s * 1000 : 0

    readonly property bool scrubbing: scrub.scrubbing
    readonly property real shownPos: scrub.shownPos

    signal closeRequested
    signal jumpRequested(string source, string session)

    readonly property var hints: {
        if (menu.open)
            return menu.hints;
        var out = [];
        if (journalFocused) {
            out.push({
                glyph: "A",
                label: entryPending ? "Being written" : "Read",
                dim: entryPending
            });
            out.push({
                glyph: "B",
                label: "Back to list"
            });
            return out;
        }
        out.push({
            glyph: "A",
            label: videoFocused && playing ? "Pause" : "Play"
        });
        if (videoFocused)
            out.push({
                glyph: "dpad",
                label: "Seek 10 s"
            });
        out.push({
            glyph: "Start",
            label: "More",
            dim: current === null
        });
        out.push({
            glyph: "B",
            label: videoFocused && !fullscreen ? "Back to list" : "Back"
        });
        return out;
    }

    readonly property real sideMargin: Theme.dp(90)
    readonly property real listWidth: Theme.dp(620)

    onGameChanged: {
        player.stop();
        player.source = "";
        index = 0;
        videoFocused = false;
        journalFocused = false;
        fullscreen = false;
        if (game) {
            store.load(game.id);
            if (journal.gameId !== game.id)
                journal.load(game.id);
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
        player.stop();
        player.source = "";
        scrub.scrubbing = false;
        journalFocused = false;
        posterSource = "";
        rest.restart();
    }

    onFramesChanged: {
        if (!rest.running)
            posterSource = frames && frames.thumbnail ? frames.thumbnail : "";
    }

    function step(d) {
        index = Sound.stepped(index, d, rows.length);
    }

    function stepScreen(d) {
        var pitch = list.currentItem ? list.currentItem.height + list.spacing : 0;
        index = Sound.paged(index, d, 1, pitch > 0 ? Math.floor(list.height / pitch) : 1, rows.length);
    }

    function focusVideo(play, silent) {
        if (!current || !current.url) {
            if (!silent)
                Sound.edge();
            return;
        }
        if (!silent)
            Sound.panel();
        journalFocused = false;
        videoFocused = true;
        if (String(player.source) !== current.url) {
            player.source = current.url;
            store.select(current.session);
        }
        if (play && !playing)
            player.play();
        wake();
    }

    function toggleFullscreen() {
        if (!current || !current.url) {
            Sound.edge();
            return;
        }
        if (fullscreen) {
            Sound.cancel();
            fullscreen = false;
        } else {
            Sound.enter();
            fullscreen = true;
            if (!videoFocused)
                focusVideo(true);
        }
        wake();
    }

    function openJournal() {
        if (current && current.hasJournal && !entryPending)
            page.jumpRequested("pages/JournalPage.qml", current.session);
        else
            Sound.edge();
    }

    function focusJournal() {
        Sound.panel();
        videoFocused = false;
        journalFocused = true;
    }

    function openMenu() {
        if (!current || !list.currentItem) {
            Sound.edge();
            return;
        }
        Sound.panel();
        var items = [
            {
                icon: "play",
                label: "Play",
                action: "play"
            },
            {
                icon: "screen",
                label: fullscreen ? "Exit fullscreen" : "Fullscreen",
                action: "fullscreen"
            }
        ];
        if (current.hasJournal)
            items.push({
                icon: "book",
                label: "Journal entry",
                action: "journal"
            });
        items.push({
            icon: "trash",
            label: "Remove recording…",
            action: "remove",
            danger: true,
            gap: true
        });
        menu.show(items, list, rowRect(), "", menuAction);
    }

    function rowRect() {
        var item = list.currentItem;
        return Qt.rect(item.x, item.y - list.contentY, item.width, item.height);
    }

    function menuAction(action) {
        if (action === "play") {
            focusVideo(true);
        } else if (action === "fullscreen") {
            toggleFullscreen();
        } else if (action === "journal") {
            openJournal();
        } else if (action === "remove") {
            Sound.panel();
            var items = [
                {
                    icon: "",
                    label: "Keep it",
                    action: ""
                },
                {
                    icon: "trash",
                    label: "Trash the recording",
                    action: "remove!",
                    danger: true
                }
            ];
            if (current.hasJournal)
                items.push({
                    icon: "trash",
                    label: "Trash it and its journal entry",
                    action: "remove-both!",
                    danger: true
                });
            menu.show(items, list, rowRect(), "Remove this recording?", menuAction);
        } else if (action === "remove!" || action === "remove-both!") {
            Sound.enter();
            player.stop();
            fullscreen = false;
            videoFocused = false;
            var gameId = current.gameId, session = current.session;
            if (action === "remove-both!")
                api.screens.journal.remove(gameId, session);
            store.remove(gameId, session);
        }
        if (action !== "remove")
            page.forceActiveFocus();
    }

    function togglePlay() {
        if (!current || !current.url) {
            Sound.edge();
            return;
        }
        Sound.enter();
        playing ? player.pause() : player.play();
        wake();
    }

    function wake() {
        controls.awake = true;
        idleTimer.restart();
    }

    Timer {
        id: idleTimer
        interval: 2600
        onTriggered: controls.awake = false
    }

    Scrubber {
        id: scrub
        player: player
        duration: page.duration
        active: page.videoFocused
        onWoke: page.wake()
    }

    Keys.onPressed: function (event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (event.isAutoRepeat && !(page.videoFocused && arrow) && !screen)
            return;
        if (page.journalFocused) {
            event.accepted = true;
            if (api.keys.isAccept(event) || api.keys.isFilters(event)) {
                openJournal();
            } else if (api.keys.isCancel(event)) {
                Sound.cancel();
                page.journalFocused = false;
            } else if (event.key === Qt.Key_Up) {
                Sound.panel();
                page.journalFocused = false;
                page.videoFocused = true;
            } else if (api.keys.isMenu(event)) {
                openMenu();
            } else {
                Sound.edge();
            }
            return;
        }
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            page.videoFocused ? togglePlay() : focusVideo(true);
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            if (page.fullscreen) {
                Sound.cancel();
                page.fullscreen = false;
            } else if (page.videoFocused) {
                Sound.cancel();
                page.videoFocused = false;
            } else {
                player.stop();
                page.closeRequested();
            }
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            toggleFullscreen();
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            openMenu();
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            openJournal();
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            event.accepted = true;
            if (!page.videoFocused)
                step(event.key === Qt.Key_Up ? -1 : 1);
            else if (event.key === Qt.Key_Down && page.entry && !page.fullscreen)
                focusJournal();
            else
                Sound.edge();
        } else if (arrow) {
            event.accepted = true;
            if (page.videoFocused) {
                if (!event.isAutoRepeat)
                    Sound.tick();
                scrub.seekBy(event.key === Qt.Key_Left ? -scrub.step : scrub.step);
            } else if (event.key === Qt.Key_Right) {
                focusVideo(false);
            } else {
                Sound.edge();
            }
        } else if (screen) {
            event.accepted = true;
            page.videoFocused ? Sound.edge() : stepScreen(screen);
        } else if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            event.accepted = true;
            page.videoFocused ? Sound.edge() : (index = Sound.stepped(index, api.keys.isFirst(event) ? -rows.length : rows.length, rows.length));
        }
    }

    Keys.onReleased: function (event) {
        if (!event.isAutoRepeat && (event.key === Qt.Key_Left || event.key === Qt.Key_Right))
            scrub.release();
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
        label: "RECORDINGS"
    }

    Text {
        anchors.centerIn: parent
        visible: page.rows.length === 0
        text: "No recordings for this game yet."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    ListView {
        id: list

        Wheel {
            step: Theme.dp(120) + list.spacing
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
        opacity: page.videoFocused || page.journalFocused ? 0.55 : 1.0
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
            readonly property var frames: page.store.frameMap[modelData.session] || null

            width: list.width
            height: Theme.dp(120)
            lit: index === page.index && !page.videoFocused && !page.journalFocused
            title: modelData.dateText
            subtitle: modelData.durationText + " · " + modelData.sizeText
            mark: "book"
            showMark: modelData.hasJournal
            leadMargin: Theme.dp(10)
            leadWidth: (height - Theme.dp(20)) * 16 / 9

            Pointer {
                current: entry.lit
                radius: Theme.dp(14)
                onPicked: {
                    page.videoFocused = false;
                    page.journalFocused = false;
                    page.index = index;
                }
            }

            RoundedMask {
                anchors.fill: parent
                anchors.topMargin: Theme.dp(10)
                anchors.bottomMargin: Theme.dp(10)
                radius: Theme.dp(10)

                Rectangle {
                    anchors.fill: parent
                    color: Theme.cardBase
                }

                Image {
                    anchors.fill: parent
                    source: frames ? frames.thumbnail : ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    opacity: status === Image.Ready ? 1.0 : 0.0

                    Behavior on opacity {
                        Ease {
                            duration: Theme.durView
                        }
                    }
                }
            }
        }
    }

    RoundedMask {
        id: pane

        anchors.top: page.fullscreen ? parent.top : list.top
        anchors.left: page.fullscreen ? parent.left : list.right
        anchors.leftMargin: page.fullscreen ? 0 : Theme.dp(40)
        anchors.right: parent.right
        anchors.rightMargin: page.fullscreen ? 0 : page.sideMargin
        height: page.fullscreen ? parent.height : width * 9 / 16
        radius: page.fullscreen ? 0 : Theme.dp(16)
        visible: page.rows.length > 0
        z: page.fullscreen ? 3 : 0

        Rectangle {
            anchors.fill: parent
            color: Theme.cardBase
        }

        // A click on the player takes it as Right does; another is A, play or pause.
        Pointer {
            current: page.videoFocused
            radius: parent.radius
            onPicked: page.focusVideo(false, true)
        }

        Image {
            anchors.fill: parent
            source: page.posterSource
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            visible: page.stopped && !mosaic.complete
            opacity: status === Image.Ready ? 1.0 : 0.0

            Behavior on opacity {
                Ease {
                    duration: Theme.durView
                }
            }
        }

        Grid {
            id: mosaic

            readonly property bool complete: page.frames !== null && page.frames.complete

            anchors.fill: parent
            columns: 4
            visible: page.stopped && complete
            opacity: page.stopped ? 1.0 : 0.0

            Behavior on opacity {
                Ease {
                    duration: Theme.durView
                }
            }

            Repeater {
                model: 16

                Item {
                    width: mosaic.width / 4
                    height: mosaic.height / 4

                    Image {
                        anchors.fill: parent
                        source: page.frames ? page.frames.frames[index] : ""
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        opacity: status === Image.Ready ? 1.0 : 0.0

                        Behavior on opacity {
                            Ease {
                                duration: Theme.durScene
                            }
                        }
                    }
                }
            }
        }

        VideoOutput {
            id: video
            anchors.fill: parent
            visible: !page.stopped
        }

        MediaPlayer {
            id: player
            videoOutput: video
            audioOutput: AudioOutput {}
            onErrorOccurred: function (error, message) {
                page.wake();
            }
        }

        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0.02, 0.02, 0.03, page.stopped ? 0.42 : 0.30)
            opacity: page.playing ? 0.0 : 1.0

            Behavior on opacity {
                Ease {}
            }
        }

        Rectangle {
            anchors.centerIn: parent
            width: Theme.dp(96)
            height: width
            radius: width / 2
            color: Qt.rgba(1, 1, 1, 0.92)
            opacity: page.playing ? 0.0 : 1.0
            scale: page.playing ? 0.8 : 1.0

            Behavior on opacity {
                Ease {}
            }
            Behavior on scale {
                Ease {
                    easing.type: Easing.OutBack
                }
            }

            MenuGlyph {
                anchors.centerIn: parent
                anchors.horizontalCenterOffset: Theme.dp(3)
                width: Theme.dp(50)
                height: width
                kind: "play"
                tint: "#101116"
            }
        }

        Text {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: Theme.dp(84)
            visible: player.error !== MediaPlayer.NoError
            text: player.errorString
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(20)
        }

        Item {
            id: controls

            property bool awake: false
            readonly property bool shown: page.videoFocused && (!page.playing || awake || page.scrubbing)
            readonly property real inset: Theme.dp(36)
            readonly property real fraction: page.duration > 0 ? page.shownPos / page.duration : 0

            anchors.fill: parent
            visible: !page.stopped || page.scrubbing
            opacity: shown ? 1.0 : 0.0

            Behavior on opacity {
                Ease {}
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: Theme.dp(170) + (page.fullscreen ? hintBar.height : 0)
                gradient: Gradient {
                    GradientStop {
                        position: 0.0
                        color: Qt.rgba(0.02, 0.02, 0.03, 0.0)
                    }
                    GradientStop {
                        position: 1.0
                        color: Qt.rgba(0.02, 0.02, 0.03, 0.85)
                    }
                }
            }

            Item {
                id: bar

                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.leftMargin: page.fullscreen ? page.sideMargin : controls.inset
                anchors.rightMargin: page.fullscreen ? page.sideMargin : controls.inset
                anchors.bottomMargin: page.fullscreen ? hintBar.height + Theme.dp(64) : Theme.dp(64)
                height: page.scrubbing ? Theme.dp(10) : Theme.dp(6)

                Behavior on height {
                    Ease {}
                }

                // A click on the bar seeks there; the strip is taller than the bar so it can be hit.
                Item {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    height: Theme.dp(40)

                    HoverHandler {
                        onHoveredChanged: if (hovered)
                            page.wake()
                    }
                    TapHandler {
                        onTapped: function (point) {
                            scrub.seekTo(point.position.x / parent.width * page.duration);
                        }
                    }
                }

                Rectangle {
                    anchors.fill: parent
                    radius: height / 2
                    color: Qt.rgba(1, 1, 1, 0.22)
                }

                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: parent.width * controls.fraction
                    radius: height / 2
                    color: Theme.text
                }

                Rectangle {
                    id: knob
                    x: parent.width * controls.fraction - width / 2
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: page.scrubbing ? -Theme.dp(4) : 0
                    width: Theme.dp(18)
                    height: width
                    radius: width / 2
                    color: Theme.text
                    scale: page.scrubbing ? 1.7 : 1.0

                    Behavior on scale {
                        NumberAnimation {
                            duration: page.scrubbing ? Theme.durBase : Theme.durNudge
                            easing.type: page.scrubbing ? Easing.OutCubic : Easing.OutBack
                        }
                    }
                    Behavior on anchors.verticalCenterOffset {
                        Ease {
                            duration: Theme.durNudge
                            easing.type: Easing.OutBack
                        }
                    }

                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width * 1.9
                        height: width
                        radius: width / 2
                        color: Qt.rgba(1, 1, 1, 0.16)
                        opacity: page.scrubbing ? 1.0 : 0.0
                        z: -1

                        Behavior on opacity {
                            NumberAnimation {
                                duration: Theme.durBase
                            }
                        }
                    }
                }

                RoundedMask {
                    id: peek

                    readonly property int frame: Math.max(0, Math.min(15, Math.floor(controls.fraction * 16)))
                    readonly property string source: page.frames ? page.frames.frames[frame] : ""

                    width: Theme.dp(300)
                    height: width * 9 / 16
                    radius: Theme.dp(10)
                    x: Math.max(0, Math.min(parent.width - width, knob.x + knob.width / 2 - width / 2))
                    anchors.bottom: parent.top
                    anchors.bottomMargin: page.scrubbing ? Theme.dp(34) : Theme.dp(18)
                    opacity: page.scrubbing ? 1.0 : 0.0
                    scale: page.scrubbing ? 1.0 : 0.9
                    transformOrigin: Item.Bottom

                    Behavior on opacity {
                        Ease {}
                    }
                    Behavior on scale {
                        Ease {
                            duration: Theme.durNudge
                            easing.type: Easing.OutBack
                        }
                    }
                    Behavior on anchors.bottomMargin {
                        Ease {
                            duration: Theme.durNudge
                            easing.type: Easing.OutBack
                        }
                    }

                    Rectangle {
                        anchors.fill: parent
                        color: Theme.cardBase
                    }

                    Image {
                        anchors.fill: parent
                        source: peek.source
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                    }

                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: Theme.dp(10)
                        width: peekTime.width + Theme.dp(20)
                        height: peekTime.height + Theme.dp(8)
                        radius: height / 2
                        color: Qt.rgba(0.02, 0.02, 0.03, 0.75)

                        Text {
                            id: peekTime
                            anchors.centerIn: parent
                            text: Format.clockTime(page.shownPos / 1000)
                            color: Theme.text
                            font.family: Theme.sans
                            font.weight: Font.DemiBold
                            font.pixelSize: Theme.dp(20)
                        }
                    }
                }
            }

            Text {
                anchors.left: bar.left
                anchors.top: bar.bottom
                anchors.topMargin: Theme.dp(14)
                text: Format.clockTime(page.shownPos / 1000) + "  /  " + Format.clockTime(page.duration / 1000)
                color: Theme.textHint
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(21)
            }

            Text {
                anchors.right: bar.right
                anchors.top: bar.bottom
                anchors.topMargin: Theme.dp(14)
                text: page.current ? page.current.dateText : ""
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: Theme.dp(5)
            color: Qt.rgba(1, 1, 1, 0.15)
            visible: !page.stopped
            opacity: controls.shown ? 0.0 : 1.0

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.durBase
                }
            }

            Rectangle {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.width * controls.fraction
                color: Theme.text
            }
        }
    }

    Loader {
        anchors.fill: pane
        active: page.videoFocused
        sourceComponent: FocusRing {
            cornerRadius: pane.radius
            gapWidth: Theme.dp(Theme.ringGap)
        }
    }

    Text {
        anchors.top: pane.bottom
        anchors.topMargin: Theme.dp(22)
        anchors.left: pane.left
        visible: page.current !== null
        text: page.current ? page.current.dateText + "  ·  " + page.current.durationText + "  ·  " + page.current.sizeText : ""
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(22)
    }

    Text {
        anchors.top: pane.bottom
        anchors.topMargin: Theme.dp(22)
        anchors.right: pane.right
        visible: page.current !== null
        width: pane.width * 0.45
        horizontalAlignment: Text.AlignRight
        text: page.current ? page.current.path.split("/").pop() : ""
        color: Theme.textFaint
        font.family: Theme.sans
        font.pixelSize: Theme.dp(20)
        elide: Text.ElideMiddle
    }

    Item {
        anchors.top: pane.bottom
        anchors.topMargin: Theme.dp(72)
        anchors.left: pane.left
        anchors.right: pane.right
        visible: page.entry !== null && !page.fullscreen
        height: journalText.height

        Pointer {
            current: page.journalFocused
            wash: 0
            onPicked: {
                page.videoFocused = false;
                page.journalFocused = true;
            }
        }

        Column {
            id: journalText

            anchors.left: parent.left
            anchors.right: parent.right
            spacing: Theme.dp(8)

            Row {
                spacing: Theme.dp(12)

                MenuGlyph {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(20)
                    height: width
                    kind: "book"
                    tint: page.journalFocused ? Theme.textSecondary : Theme.textMuted
                }

                CapsLabel {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "JOURNAL"
                    tracking: 0.11
                    color: page.journalFocused ? Theme.textSecondary : Theme.textMuted
                }
            }

            Text {
                width: parent.width
                text: page.entry ? (page.entryPending ? "Writing the entry…" : page.entry.title) : ""
                color: page.entryPending ? Theme.textSecondary : Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(26)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: page.entry ? (page.entryPending ? "The journal module is writing this entry." : page.entry.paragraphs[0] || "") : ""
                textFormat: Text.MarkdownText
                color: page.journalFocused ? Theme.text : Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(22)
                lineHeight: 1.4
                wrapMode: Text.WordWrap

                Behavior on color {
                    ColorEase {
                        duration: Theme.durBase
                    }
                }
            }
        }

        Loader {
            anchors.fill: journalText
            anchors.margins: -Theme.dp(14)
            active: page.journalFocused
            sourceComponent: FocusRing {
                cornerRadius: Theme.dp(14)
                gapWidth: 0
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
        z: 4
        opacity: page.fullscreen && !controls.shown ? 0.0 : 1.0

        Behavior on opacity {
            Ease {}
        }
    }

    ActionMenu {
        id: menu

        anchors.fill: parent
        z: 5

        onDismissed: page.forceActiveFocus()
    }
}
