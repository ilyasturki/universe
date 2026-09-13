import QtQuick
import "../core"
import "../sound"
import "../ui"

// A game's journal: entries on the left, the picked one read on the right. Right or A hands
// the d-pad to the article; Down past its end lands on the screenshots.
FocusScope {
    id: page

    focus: true

    property var game: null
    // Set by the shell when the recordings jump here.
    property string session: ""
    readonly property var store: api.screens.journal
    readonly property var rows: store.rows
    property int index: 0
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null
    readonly property string currentSession: current ? current.session : ""
    readonly property var images: current ? current.images : []

    // 0 entries, 1 the text, 2 the screenshots
    property int mode: 0
    property int shotIndex: 0
    property bool lightbox: false
    readonly property bool reading: mode > 0

    signal closeRequested()
    signal jumpRequested(string source, string session)

    readonly property var hints: {
        if (lightbox)
            return [ { glyph: "dpad", label: "Previous / next" }, { glyph: "B", label: "Close" } ];
        var out = [];
        if (mode === 2) {
            out.push({ glyph: "A", label: "View" });
            out.push({ glyph: "dpad", label: "Navigate" });
        } else if (mode === 1) {
            out.push({ glyph: "dpad", label: "Scroll" });
        } else {
            out.push({ glyph: "A", label: "Read" });
            out.push({ glyph: "dpad", label: "Navigate" });
        }
        if (current && current.hasRecording)
            out.push({ glyph: "Y", label: "Recording" });
        out.push({ glyph: "B", label: reading ? "Back to entries" : "Back" });
        return out;
    }

    readonly property real sideMargin: Theme.dp(90)
    readonly property real listWidth: Theme.dp(560)

    onGameChanged: {
        index = 0;
        mode = 0;
        lightbox = false;
        if (game)
            store.load(game.id);
        landOnSession();
    }

    onSessionChanged: landOnSession()
    onRowsChanged: landOnSession()

    function landOnSession() {
        if (session === "")
            return;
        for (var i = 0; i < rows.length; i++) {
            if (rows[i].session === session) {
                index = i;
                return;
            }
        }
    }

    onCurrentChanged: {
        flick.contentY = 0;
        shotIndex = 0;
    }

    function step(d) {
        var next = Math.max(0, Math.min(rows.length - 1, index + d));
        next === index ? Sound.edge() : Sound.tick();
        index = next;
    }

    function maxScroll() {
        return Math.max(0, flick.contentHeight - flick.height);
    }

    function scroll(d) {
        var next = Math.max(0, Math.min(maxScroll(), flick.contentY + d * Theme.dp(260)));
        if (next === flick.contentY) {
            if (d > 0 && images.length > 0) {
                Sound.panel();
                mode = 2;
                flick.contentY = maxScroll();
            } else {
                Sound.edge();
            }
            return;
        }
        Sound.tick();
        flick.contentY = next;
    }

    function stepShot(d) {
        var next = Math.max(0, Math.min(images.length - 1, shotIndex + d));
        next === shotIndex ? Sound.edge() : Sound.tick();
        shotIndex = next;
    }

    function read() {
        if (!current) {
            Sound.edge();
            return;
        }
        Sound.panel();
        mode = 1;
    }

    function leave() {
        Sound.cancel();
        mode = 0;
    }

    Keys.onPressed: function(event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        if (event.isAutoRepeat && !(lightbox && arrow) && !(mode === 2 && arrow))
            return;

        if (lightbox) {
            event.accepted = true;
            if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                Sound.cancel();
                lightbox = false;
            } else if (arrow) {
                stepShot(event.key === Qt.Key_Left ? -1 : 1);
            }
            return;
        }

        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (mode === 2) {
                Sound.enter();
                lightbox = true;
            } else if (mode === 1) {
                if (images.length > 0) {
                    Sound.panel();
                    mode = 2;
                    flick.contentY = maxScroll();
                } else {
                    Sound.edge();
                }
            } else {
                read();
            }
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            reading ? leave() : page.closeRequested();
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            if (current && current.hasRecording)
                page.jumpRequested("pages/RecordingsPage.qml", current.session);
            else
                Sound.edge();
        } else if (event.key === Qt.Key_Up) {
            event.accepted = true;
            if (mode === 2) {
                Sound.panel();
                mode = 1;
            } else if (mode === 1) {
                scroll(-1);
            } else {
                step(-1);
            }
        } else if (event.key === Qt.Key_Down) {
            event.accepted = true;
            if (mode === 2)
                Sound.edge();
            else if (mode === 1)
                scroll(1);
            else
                step(1);
        } else if (event.key === Qt.Key_Right) {
            event.accepted = true;
            mode === 2 ? stepShot(1) : mode === 1 ? Sound.edge() : read();
        } else if (event.key === Qt.Key_Left) {
            event.accepted = true;
            mode === 2 ? stepShot(-1) : mode === 1 ? leave() : Sound.edge();
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
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
        detail: page.rows.length > 0 ? page.rows.length + (page.rows.length === 1 ? " entry" : " entries") : ""
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
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        delegate: Rectangle {
            readonly property bool focused: index === page.index
            readonly property bool lit: focused && !page.reading

            width: list.width
            height: Theme.dp(96)
            radius: Theme.dp(16)
            color: lit ? Theme.text : Theme.surface

            Behavior on color {
                ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
            }

            Column {
                anchors.left: parent.left
                anchors.right: recordingMark.visible ? recordingMark.left : parent.right
                anchors.margins: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(6)

                Text {
                    width: parent.width
                    text: modelData.title
                    color: lit ? Theme.onLight : Theme.text
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(24)
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: modelData.dateText
                    color: lit ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(20)
                    elide: Text.ElideRight
                }
            }

            MenuGlyph {
                id: recordingMark
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(24)
                height: width
                visible: modelData.hasRecording
                kind: "film"
                tint: lit ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Theme.textMuted
            }
        }
    }

    Flickable {
        id: flick

        anchors.top: list.top
        anchors.bottom: hintBar.top
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(60)
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        contentWidth: width
        contentHeight: article.height + Theme.dp(60)
        interactive: false
        clip: true
        visible: page.current !== null

        Behavior on contentY {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
        }

        Column {
            id: article

            width: flick.width
            spacing: Theme.dp(28)

            Text {
                width: parent.width
                text: page.current ? page.current.title : ""
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(38)
                wrapMode: Text.WordWrap
            }

            CapsLabel {
                text: page.current ? page.current.dateText : ""
                tracking: 0.11
            }

            Repeater {
                model: page.current ? page.current.blocks : []

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
                        ColorAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
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

            Column {
                id: shots

                width: parent.width
                spacing: Theme.dp(18)
                visible: page.images.length > 0

                readonly property bool focused: page.mode === 2 && !page.lightbox
                readonly property real shotWidth: Theme.dp(336)
                readonly property real shotHeight: Theme.dp(189)

                CapsLabel {
                    text: "SCREENSHOTS"
                    tracking: 0.11
                    color: shots.focused ? Theme.textSecondary : Theme.textMuted
                }

                ListView {
                    id: shotStrip

                    // Room for the focus ring's halo inside the clip on every side.
                    readonly property real inset: Theme.dp(24)

                    x: -inset
                    width: parent.width + page.sideMargin + inset
                    height: shots.shotHeight + inset * 2
                    leftMargin: inset
                    orientation: ListView.Horizontal
                    spacing: Theme.dp(20)
                    model: page.images
                    interactive: false
                    clip: true
                    currentIndex: page.shotIndex
                    boundsBehavior: Flickable.StopAtBounds
                    highlightFollowsCurrentItem: false

                    onCurrentIndexChanged: slide()
                    onWidthChanged: slide()

                    function slide() {
                        var pitch = shots.shotWidth + spacing;
                        var target = currentIndex * pitch - (width - page.sideMargin) * 0.5 + shots.shotWidth * 0.5;
                        contentX = Math.max(-inset, Math.min(target, Math.max(-inset, contentWidth - width + inset)));
                    }

                    Behavior on contentX {
                        NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutQuint }
                    }

                    delegate: Item {
                        width: shots.shotWidth
                        height: shotStrip.height

                        readonly property bool current: index === page.shotIndex && shots.focused

                        RoundedMask {
                            id: shotCard
                            width: shots.shotWidth
                            height: shots.shotHeight
                            anchors.verticalCenter: parent.verticalCenter
                            radius: Theme.dp(10)
                            opacity: shots.focused && !current ? 0.6 : 1.0
                            scale: current ? 1.03 : 1.0

                            Behavior on opacity {
                                NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                            }
                            Behavior on scale {
                                NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
                            }

                            Rectangle {
                                anchors.fill: parent
                                color: Theme.cardBase
                            }

                            Image {
                                anchors.fill: parent
                                source: modelData
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                mipmap: true
                            }
                        }

                        Loader {
                            anchors.fill: shotCard
                            active: current
                            sourceComponent: FocusRing {
                                cornerRadius: shotCard.radius
                                gapWidth: Theme.dp(4)
                            }
                        }
                    }
                }
            }
        }
    }

    // Scrolled text runs out under the hint bar rather than into it.
    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: flick.left
        anchors.right: parent.right
        height: hintBar.height + Theme.dp(50)
        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(0.055, 0.059, 0.075, 0.0) }
            GradientStop { position: 0.45; color: Qt.rgba(0.055, 0.059, 0.075, 0.92) }
            GradientStop { position: 1.0; color: Theme.ground }
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        showClock: true
        hints: page.hints
    }

    Lightbox {
        anchors.fill: parent
        images: page.images
        index: page.shotIndex
        open: page.lightbox
    }
}
