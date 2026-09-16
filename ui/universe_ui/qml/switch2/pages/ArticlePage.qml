import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../ui/Removal.js" as Removal

FocusScope {
    id: page

    property var shell: null
    property var args: ({})

    readonly property var store: api.screens.news
    readonly property var row: store.rows.filter(function(r) { return r.session === args.session; })[0] || null
    readonly property var images: row ? row.images : []
    readonly property var game: row ? api.allGames.byId(row.gameId) : null

    property string mode: "text"
    property int shotIndex: 0
    property bool lightbox: false

    readonly property var hints: lightbox
        ? [ { glyph: "dpad", label: "Previous / next" }, { glyph: "B", label: "Close" } ]
        : mode === "shots"
        ? [ { glyph: "Y", label: "Recording", dim: !(row && row.hasRecording) }, { glyph: "B", label: "Back" }, { glyph: "A", label: "View" } ]
        : [ { glyph: "Start", label: "Options" }, { glyph: "Y", label: "Recording", dim: !(row && row.hasRecording) }, { glyph: "B", label: "Back" } ]

    signal closeRequested()

    readonly property real columnX: Theme.dp(253)
    readonly property real columnWidth: Theme.dp(1667 - 253)
    readonly property real shotWidth: Theme.dp(440)
    readonly property real shotHeight: Math.round(shotWidth * 9 / 16)

    focus: true

    Component.onCompleted: {
        if (store.count === 0)
            store.loadAll();
    }

    function maxScroll() {
        return Math.max(0, flick.contentHeight - flick.height);
    }

    function scroll(d) {
        var next = Math.max(0, Math.min(maxScroll(), flick.contentY + d * Theme.dp(260)));
        if (next === flick.contentY) {
            d > 0 ? openShots() : Sound.play("edge");
            return;
        }
        Sound.play("tick");
        flick.contentY = next;
    }

    function openShots() {
        if (images.length === 0) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        mode = "shots";
        flick.contentY = maxScroll();
    }

    function stepShot(d) { shotIndex = Sound.stepped(shotIndex, d, images.length); }

    Keys.onPressed: function(event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        if (event.isAutoRepeat && !arrow)
            return;

        if (lightbox) {
            event.accepted = true;
            if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                Sound.play("back");
                lightbox = false;
            } else if (arrow) {
                stepShot(event.key === Qt.Key_Left ? -1 : 1);
            }
            return;
        }

        if (api.keys.isFilters(event)) {
            event.accepted = true;
            if (row && row.hasRecording) {
                Sound.play("ok");
                shell.push("pages/PlayerPage.qml", { session: row.session, gameId: row.gameId });
            } else {
                Sound.play("edge");
            }
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            if (!row) {
                Sound.play("edge");
                return;
            }
            Sound.play("ok");
            Removal.entry(shell, api.screens, row, function() { page.closeRequested(); });
        } else if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (mode === "shots") {
                Sound.play("ok");
                lightbox = true;
            } else {
                openShots();
            }
        } else if (api.keys.isCancel(event)) {
            if (mode === "shots") {
                event.accepted = true;
                Sound.play("back");
                mode = "text";
            }
        } else if (event.key === Qt.Key_Up) {
            event.accepted = true;
            if (mode === "shots") {
                Sound.play("tick");
                mode = "text";
            } else {
                scroll(-1);
            }
        } else if (event.key === Qt.Key_Down) {
            event.accepted = true;
            mode === "shots" ? Sound.play("edge") : scroll(1);
        } else if (arrow) {
            event.accepted = true;
            mode === "shots" ? stepShot(event.key === Qt.Key_Left ? -1 : 1) : Sound.play("edge");
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "news"
        title: "News"
        trailing: page.row ? page.row.gameTitle : ""
    }

    Label {
        anchors.centerIn: parent
        visible: page.row === null
        text: "This entry is gone."
        color: Theme.textMuted
    }

    Flickable {
        id: flick

        readonly property real room: Theme.dp(Theme.ringRoom)

        x: page.columnX - room
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight)
        width: page.columnWidth + room * 2
        contentWidth: width
        contentHeight: article.height + Theme.dp(120)
        interactive: false
        clip: true
        visible: page.row !== null

        Behavior on contentY { Ease {} }

        Column {
            id: article

            x: flick.room
            y: Theme.dp(48)
            width: page.columnWidth
            spacing: Theme.dp(24)

            Row {
                spacing: Theme.dp(18)

                Tile {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(52)
                    height: Theme.dp(52)
                    cornerRadius: Theme.dp(4)
                    game: page.game
                    outlineShown: false
                }

                Label {
                    anchors.verticalCenter: parent.verticalCenter
                    text: page.row ? page.row.gameTitle : ""
                    color: Theme.accent
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }
            }

            Label {
                width: parent.width
                text: page.row ? page.row.title : ""
                wrapMode: Text.WordWrap
                font.pixelSize: Theme.dp(Theme.fontTitle)
            }

            Label {
                text: page.row ? page.row.dateText : ""
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Rectangle {
                width: parent.width
                height: 1
                color: Theme.hairline
            }

            Repeater {
                model: page.row ? page.row.blocks : []

                Label {
                    width: article.width
                    text: modelData
                    textFormat: Text.MarkdownText
                    wrapMode: Text.WordWrap
                    lineHeight: 1.5
                }
            }

            Column {
                width: parent.width
                spacing: Theme.dp(10)
                visible: page.row && page.row.next_up !== ""

                Label {
                    text: "Next up"
                    color: Theme.accent
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }

                Label {
                    width: parent.width
                    text: page.row ? page.row.next_up : ""
                    textFormat: Text.MarkdownText
                    wrapMode: Text.WordWrap
                    lineHeight: 1.4
                }
            }

            Column {
                id: shots

                width: parent.width
                spacing: Theme.dp(16)
                visible: page.images.length > 0

                readonly property bool focused: page.mode === "shots" && !page.lightbox

                Label {
                    text: "Screenshots"
                    color: shots.focused ? Theme.accent : Theme.textSecondary
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }

                ListView {
                    id: strip

                    readonly property real inset: flick.room

                    x: -inset
                    width: parent.width + inset * 2
                    height: page.shotHeight + inset * 2
                    leftMargin: inset
                    rightMargin: inset
                    orientation: ListView.Horizontal
                    spacing: Theme.dp(18)
                    model: page.images
                    interactive: false
                    clip: true
                    currentIndex: page.shotIndex
                    boundsBehavior: Flickable.StopAtBounds
                    highlightFollowsCurrentItem: false

                    onCurrentIndexChanged: slide()
                    onWidthChanged: slide()

                    function slide() {
                        var pitch = page.shotWidth + spacing;
                        var left = currentIndex * pitch, right = left + page.shotWidth;
                        var target = contentX;
                        if (left < contentX + leftMargin - inset)
                            target = left - leftMargin + inset;
                        else if (right > contentX + width - inset)
                            target = right - width + inset;
                        contentX = Math.max(-leftMargin, Math.min(target, Math.max(-leftMargin, contentWidth - width + rightMargin)));
                    }

                    Behavior on contentX { Ease {} }

                    delegate: Item {
                        width: page.shotWidth
                        height: strip.height

                        readonly property bool current: index === page.shotIndex && shots.focused

                        Rectangle {
                            id: shotCard
                            width: page.shotWidth
                            height: page.shotHeight
                            anchors.verticalCenter: parent.verticalCenter
                            radius: Theme.dp(4)
                            color: Theme.artShade

                            Image {
                                anchors.fill: parent
                                source: modelData
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                sourceSize.width: 640
                            }
                        }

                        FocusOutline {
                            target: shotCard
                            cornerRadius: shotCard.radius
                            shown: current
                        }
                    }
                }
            }
        }
    }

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(60)
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        flickable: flick
    }

    Rectangle {
        anchors.fill: parent
        color: "#000000"
        opacity: page.lightbox ? 1.0 : 0.0
        visible: opacity > 0.01
        z: 5

        Behavior on opacity { Ease { duration: Theme.durQuick } }

        Image {
            anchors.fill: parent
            source: page.lightbox && page.shotIndex < page.images.length ? page.images[page.shotIndex] : ""
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            sourceSize.width: page.width
        }

        Label {
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: Theme.dp(40)
            text: (page.shotIndex + 1) + " / " + page.images.length
            color: "#ffffff"
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }
}
