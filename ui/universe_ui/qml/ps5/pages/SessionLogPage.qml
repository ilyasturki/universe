import QtQuick
import "../core"
import "../sound"
import "../ui"

// What the game and its runner wrote during one session, the last lines in view.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property string session: args && args.session !== undefined ? String(args.session) : ""
    readonly property var store: api.screens.sessions
    readonly property var log: store.log
    readonly property bool strip: true

    readonly property var hints: [
        {
            glyph: "LT RT",
            label: "Page"
        },
        {
            glyph: "B",
            label: "Back"
        }
    ]

    signal closeRequested

    focus: true

    onArgsChanged: {
        if (args && args.gameId) {
            if (store.gameId !== args.gameId)
                store.load(args.gameId);
            store.openLog(args.session !== undefined ? String(args.session) : "");
        }
    }

    onLogChanged: Qt.callLater(function () {
        flick.contentY = maxScroll();
    })

    function maxScroll() {
        return Math.max(0, flick.contentHeight - flick.height);
    }

    function scroll(d) {
        var next = Math.max(0, Math.min(maxScroll(), flick.contentY + d * Theme.dp(260)));
        if (next === flick.contentY) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        flick.contentY = next;
    }

    Keys.onPressed: function (event) {
        var screen = api.keys.isScreenUp(event) || api.keys.isPageUp(event) ? -1 : api.keys.isScreenDown(event) || api.keys.isPageDown(event) ? 1 : 0;
        if (event.key === Qt.Key_Up) {
            event.accepted = true;
            scroll(-1);
        } else if (event.key === Qt.Key_Down) {
            event.accepted = true;
            scroll(1);
        } else if (screen) {
            event.accepted = true;
            scroll(screen * 3);
        } else if (!event.isAutoRepeat && (api.keys.isFirst(event) || api.keys.isLast(event))) {
            event.accepted = true;
            flick.contentY = api.keys.isFirst(event) ? 0 : maxScroll();
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
        title: page.args && page.args.label ? String(page.args.label) : "Log"
        trailing: page.session === "" ? "Playing now" : page.session
    }

    Label {
        anchors.centerIn: parent
        visible: page.store.logLoading || page.store.logError !== "" || page.log.length === 0
        text: page.store.logLoading ? "Reading…" : page.store.logError !== "" ? page.store.logError : "The system journal no longer holds this session."
        color: page.store.logError !== "" ? Theme.danger : Theme.textMuted
    }

    Rectangle {
        x: Theme.dp(Theme.edge) - Theme.dp(24)
        y: header.height + Theme.dp(4)
        width: parent.width - x - Theme.dp(Theme.columnRight) + Theme.dp(24)
        height: parent.height - y - Theme.dp(96)
        radius: Theme.dp(Theme.radiusCard)
        color: Qt.rgba(0.04, 0.045, 0.06, 0.72)
        border.width: 1
        border.color: Theme.glassEdge
        visible: flick.visible
    }

    Flickable {
        id: flick

        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(4)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: parent.height - y - Theme.dp(96)
        contentWidth: width
        contentHeight: lines.height + Theme.dp(48)
        interactive: false
        clip: true
        visible: page.log.length > 0 && !page.store.logLoading

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        Column {
            id: lines
            y: Theme.dp(24)
            width: flick.width - Theme.dp(24)
            spacing: Theme.dp(6)

            Repeater {
                model: page.log

                Row {
                    width: lines.width
                    spacing: Theme.dp(16)

                    Label {
                        width: Theme.dp(110)
                        text: modelData.time
                        color: Theme.textMuted
                        font.family: "monospace"
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                    }

                    Label {
                        width: Theme.dp(190)
                        text: modelData.source
                        color: modelData.error ? Theme.danger : modelData.warning ? "#f0a64a" : Theme.textSecondary
                        font.family: "monospace"
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                        elide: Text.ElideRight
                    }

                    Label {
                        width: parent.width - Theme.dp(110 + 190 + 32)
                        text: modelData.message
                        color: modelData.error ? Theme.danger : Theme.text
                        font.family: "monospace"
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                        wrapMode: Text.WrapAnywhere
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
        anchors.leftMargin: Theme.dp(20)
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        flickable: flick
    }
}
