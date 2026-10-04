import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../ui" as Base
import "Home.js" as Home
import "../ui/Removal.js" as Removal

// A journal entry to read over the game's world: its title, the text, what comes next,
// the session's recording and the pictures it was written from.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var hints: []
    readonly property bool strip: false
    signal closeRequested
    focus: true

    readonly property var store: api.screens.news
    readonly property var row: {
        var all = store.rows;
        return all.filter(function (r) {
            return r.session === args.session && (!args.gameId || r.gameId === args.gameId);
        })[0] || null;
    }
    readonly property var game: row ? api.allGames.byId(row.gameId) : args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var images: row ? row.images : []
    readonly property bool pending: row !== null && row.state === "pending"
    // An entry nobody has written yet: never asked for, put off after a failure, or given up on.
    readonly property bool blank: row !== null && (row.state === "none" || row.state === "deferred" || row.state === "failed")
    readonly property bool written: row !== null && row.state === "written"
    readonly property bool hasRecording: row !== null && row.hasRecording === true
    // The recording's picture, from the gallery's timeline when it holds it.
    readonly property string recordingImage: {
        if (!row)
            return "";
        var hit = api.screens.media.rows.filter(function (r) {
            return r.kind === "recording" && r.session === row.session && r.gameId === row.gameId;
        })[0];
        return hit && hit.image ? hit.image : "";
    }

    // "text", "recording" or "images"
    property string zone: "text"
    property int shotIndex: 0
    property bool lightbox: false
    property double now: Date.now()
    readonly property bool modal: lightbox

    // Pushed cold (from the hub, the player): the journal is asked for the game's rows once.
    property bool asked: false
    function ensure() {
        if (row || asked || !args.session)
            return;
        asked = true;
        if (args.gameId)
            store.load(args.gameId);
        else
            store.loadAll();
    }

    onArgsChanged: ensure()
    onRowChanged: {
        if (!row) {
            zone = "text";
            lightbox = false;
        }
    }

    Timer {
        interval: 1000
        running: page.pending
        repeat: true
        onTriggered: page.now = Date.now()
    }

    function elapsed(startedAt) {
        var s = Math.round((now - Date.parse(startedAt)) / 1000);
        if (isNaN(s))
            return "";
        s = Math.max(0, s);
        if (s < 60)
            return s + " s";
        if (s < 3600)
            return Math.floor(s / 60) + " min";
        return Math.floor(s / 3600) + " h " + ("0" + Math.floor((s % 3600) / 60)).slice(-2);
    }

    function maxScroll() {
        return Math.max(0, flick.contentHeight - flick.height);
    }

    function scroll(d) {
        var next = Math.max(0, Math.min(maxScroll(), flick.contentY + d * Theme.dp(260)));
        if (next === flick.contentY) {
            if (d > 0)
                below();
            else
                Sound.play("edge");
            return;
        }
        Sound.play("tick");
        flick.contentY = next;
    }

    // Down past the text: the recording's card, then the pictures.
    function below() {
        if (hasRecording && zone === "text") {
            Sound.play("tick");
            zone = "recording";
            reveal(recordingCard);
        } else if (images.length > 0 && zone !== "images") {
            Sound.play("tick");
            zone = "images";
            reveal(shots);
        } else {
            Sound.play("edge");
        }
    }

    function above() {
        Sound.play("tick");
        if (zone === "images" && hasRecording) {
            zone = "recording";
            reveal(recordingCard);
        } else {
            zone = "text";
        }
    }

    function reveal(item) {
        var top = item.y + article.y, bottom = top + item.height + Theme.dp(40);
        Theme.reveal(flick, top, bottom, flick.height);
    }

    function write(again) {
        if (!row || pending) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        store.write(row.gameId, row.session, again);
    }

    function playRecording() {
        if (!hasRecording) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        shell.push("pages/PlayerPage.qml", {
            gameId: row.gameId,
            session: row.session
        });
    }

    function options() {
        if (!row) {
            Sound.play("edge");
            return;
        }
        var items = [];
        if (hasRecording)
            items.push({
                label: "Play Recording",
                glyph: "film",
                act: "recording"
            });
        if (blank)
            items.push({
                label: row.state === "none" ? "Write the Entry" : "Try Again Now",
                glyph: row.state === "none" ? "plus" : "refresh",
                act: "write"
            });
        if (written)
            items.push({
                label: "Write It Again",
                glyph: "refresh",
                act: "rewrite"
            });
        if (row.state !== "none")
            items.push({
                label: pending ? "Cancel the Writing…" : "Delete Entry…",
                glyph: "trash",
                act: "remove",
                gap: true
            });
        if (items.length === 0) {
            Sound.play("edge");
            return;
        }
        shell.showMenu({
            title: row.gameTitle + " · " + row.dateText,
            items: items
        }, function (i) {
            if (i < 0)
                return;
            var act = items[i].act;
            if (act === "recording")
                playRecording();
            else if (act === "write")
                write(false);
            else if (act === "rewrite")
                write(true);
            else
                Removal.entry(shell, api.screens, row, function () {
                    page.closeRequested();
                });
        });
    }

    Keys.onPressed: function (event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        if (event.isAutoRepeat && !arrow && !(event.key === Qt.Key_Up || event.key === Qt.Key_Down))
            return;
        if (lightbox) {
            event.accepted = true;
            if (arrow)
                shotIndex = Sound.stepped(shotIndex, event.key === Qt.Key_Left ? -1 : 1, images.length);
            else if (!event.isAutoRepeat && (api.keys.isCancel(event) || api.keys.isAccept(event))) {
                Sound.play("back");
                lightbox = false;
            }
            return;
        }
        if (event.key === Qt.Key_Down) {
            event.accepted = true;
            zone === "text" ? scroll(1) : below();
        } else if (event.key === Qt.Key_Up) {
            event.accepted = true;
            zone === "text" ? scroll(-1) : above();
        } else if (arrow) {
            event.accepted = true;
            if (zone === "images")
                shotIndex = Sound.stepped(shotIndex, event.key === Qt.Key_Left ? -1 : 1, images.length);
            else
                Sound.play("edge");
        } else if (event.isAutoRepeat) {
            return;
        } else if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (zone === "images") {
                Sound.play("ok");
                lightbox = true;
            } else if (zone === "recording")
                playRecording();
            else if (blank)
                write(false);
            else if (written && (hasRecording || images.length > 0))
                below();
            else
                Sound.play("edge");
        } else if (api.keys.isCancel(event) && zone !== "text") {
            event.accepted = true;
            Sound.play("back");
            zone = "text";
            flick.contentY = 0;
        } else if (api.keys.isDetails(event) || api.keys.isFilters(event)) {
            event.accepted = true;
            playRecording();
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            options();
        }
    }

    ArtBackdrop {
        anchors.fill: parent
        target: page.game ? Home.art(page.game) : {
            scene: "system"
        }
        dim: 0.62
    }

    PageTitle {
        anchors.left: parent.left
        anchors.right: parent.right
        title: page.row ? page.row.gameTitle : page.game ? page.game.title : "Journal"
        game: page.game
        trailing: "Journal"
    }

    Label {
        anchors.centerIn: parent
        visible: page.row === null && page.asked
        text: "This entry is gone."
        color: Theme.textMuted
    }

    Flickable {
        id: flick

        readonly property real room: Theme.dp(12)

        x: Theme.dp(Theme.edge) - room
        y: Theme.dp(170)
        width: Math.min(Theme.dp(1260), parent.width - Theme.dp(Theme.edge) - Theme.dp(Theme.columnRight)) + room * 2
        height: parent.height - y
        contentWidth: width
        contentHeight: article.height + Theme.dp(120)
        interactive: false
        clip: true
        visible: page.row !== null

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        Column {
            id: article

            x: flick.room
            y: Theme.dp(20)
            width: flick.width - flick.room * 2
            spacing: Theme.dp(26)

            Label {
                width: parent.width
                text: !page.row ? "" : page.pending ? "Writing the entry…" : page.row.state === "none" ? "No entry yet" : page.row.title
                color: page.pending || page.blank ? Theme.textSecondary : Theme.text
                wrapMode: Text.WordWrap
                lineHeight: 1.05
                font.weight: Font.Light
                font.pixelSize: Theme.dp(56)
            }

            Label {
                text: page.row ? [page.row.dateText, page.row.durationText, page.row.provider].filter(Boolean).join("  ·  ") : ""
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Label {
                width: parent.width
                visible: text !== ""
                text: {
                    if (!page.row)
                        return "";
                    if (page.pending)
                        return "The journal module is writing this entry: " + page.elapsed(page.row.started_at) + " so far. It shows up here when it is done.";
                    if (page.row.state === "none")
                        return "Nobody wrote about this session yet. " + Theme.buttonName("A") + " asks the journal module for its entry.";
                    if (page.row.state === "deferred")
                        return page.row.reason + ". Another try " + page.row.retryText + ", or ask for one now with " + Theme.buttonName("A") + ".";
                    if (page.row.state === "failed")
                        return page.row.reason + ". Ask for another try with " + Theme.buttonName("A") + ".";
                    return "";
                }
                color: page.row && page.row.state === "failed" ? Theme.danger : Theme.textMuted
                wrapMode: Text.WordWrap
                lineHeight: 1.4
                font.pixelSize: Theme.dp(26)
            }

            Repeater {
                // A row with no entry says its piece above; its one paragraph is that same reason.
                model: page.row && !page.blank ? page.row.blocks : []

                Label {
                    width: article.width
                    text: modelData
                    textFormat: Text.MarkdownText
                    wrapMode: Text.WordWrap
                    lineHeight: 1.5
                    color: page.zone === "text" ? Theme.text : Theme.textSecondary
                    font.pixelSize: Theme.dp(Theme.fontBody)

                    Behavior on color {
                        ColorAnimation {
                            duration: Theme.durFocus
                        }
                    }
                }
            }

            Rectangle {
                width: parent.width
                height: nextColumn.height + Theme.dp(44)
                radius: Theme.dp(Theme.radiusCard)
                color: Theme.glass
                border.width: 1
                border.color: Theme.glassEdge
                visible: page.row !== null && page.row.next_up !== ""

                Column {
                    id: nextColumn
                    x: Theme.dp(28)
                    y: Theme.dp(22)
                    width: parent.width - Theme.dp(56)
                    spacing: Theme.dp(10)

                    Label {
                        text: "Next up"
                        color: Theme.textSecondary
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }

                    Label {
                        width: parent.width
                        text: page.row ? page.row.next_up : ""
                        textFormat: Text.MarkdownText
                        wrapMode: Text.WordWrap
                        lineHeight: 1.4
                        font.weight: Font.DemiBold
                        font.pixelSize: Theme.dp(Theme.fontBody)
                    }
                }
            }

            Item {
                width: parent.width
                height: Theme.dp(10)
            }

            HubCard {
                id: recordingCard
                width: Theme.dp(480)
                height: Theme.dp(270)
                visible: page.hasRecording
                image: page.recordingImage !== "" ? page.recordingImage : page.game ? Home.art(page.game).source : ""
                playIcon: true
                badge: "film"
                caption: "Recording"
                title: page.row && page.row.durationText ? page.row.durationText : "Play"
                focused: page.zone === "recording" && !page.lightbox && page.activeFocus
                onPicked: {
                    Sound.play("tick");
                    page.zone = "recording";
                    page.forceActiveFocus();
                }
            }

            Column {
                id: shots

                width: parent.width
                spacing: Theme.dp(16)
                visible: page.images.length > 0

                Label {
                    text: "Screenshots"
                    color: page.zone === "images" ? Theme.text : Theme.textSecondary
                    font.pixelSize: Theme.dp(26)
                }

                ListView {
                    id: strip

                    readonly property real cardWidth: Theme.dp(440)
                    readonly property real cardHeight: Math.round(cardWidth * 9 / 16)
                    readonly property real inset: Theme.dp(10)

                    x: -inset
                    width: parent.width + inset * 2
                    height: cardHeight + inset * 2
                    leftMargin: inset
                    rightMargin: inset
                    orientation: ListView.Horizontal
                    spacing: Theme.dp(24)
                    model: page.images
                    interactive: false
                    clip: true
                    currentIndex: page.shotIndex
                    boundsBehavior: Flickable.StopAtBounds
                    highlightFollowsCurrentItem: false

                    onCurrentIndexChanged: slide()
                    onWidthChanged: slide()

                    function slide() {
                        var pitch = cardWidth + spacing;
                        var left = currentIndex * pitch, right = left + cardWidth;
                        var target = contentX;
                        if (left < contentX + leftMargin - inset)
                            target = left - leftMargin + inset;
                        else if (right > contentX + width - inset)
                            target = right - width + inset;
                        contentX = Math.max(-leftMargin, Math.min(target, Math.max(-leftMargin, contentWidth - width + rightMargin)));
                    }

                    Behavior on contentX {
                        NumberAnimation {
                            duration: Theme.durMove
                            easing.type: Easing.OutCubic
                        }
                    }

                    delegate: Item {
                        id: shot

                        readonly property bool here: index === page.shotIndex && page.zone === "images" && !page.lightbox && page.activeFocus

                        width: strip.cardWidth
                        height: strip.height

                        Base.RoundedMask {
                            id: shotCard
                            y: strip.inset
                            width: strip.cardWidth
                            height: strip.cardHeight
                            radius: Theme.dp(6)

                            Rectangle {
                                anchors.fill: parent
                                color: Theme.artShade
                            }

                            Image {
                                anchors.fill: parent
                                source: modelData
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                sourceSize.width: 640
                            }
                        }

                        FocusFrame {
                            target: shotCard
                            shown: shot.here
                            radius: Theme.dp(6)
                            gap: Theme.dp(3)
                            line: Theme.dp(3)
                        }

                        Touch {
                            anchors.fill: shotCard
                            current: shot.here
                            onPicked: {
                                Sound.play("tick");
                                page.zone = "images";
                                page.shotIndex = index;
                                page.forceActiveFocus();
                            }
                        }
                    }
                }
            }
        }
    }

    Swipe {
        flickable: flick
        ease: scrollEase
    }

    Scrollbar {
        anchors.left: flick.right
        anchors.leftMargin: Theme.dp(24)
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        flickable: flick
    }

    GalleryViewer {
        anchors.fill: parent
        z: 5
        images: page.images
        index: page.shotIndex
        open: page.lightbox
        caption: page.row ? page.row.gameTitle + "  ·  " + page.row.dateText : ""
    }
}
