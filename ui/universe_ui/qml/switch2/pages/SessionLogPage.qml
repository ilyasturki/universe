import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property string session: args && args.session !== undefined ? String(args.session) : ""
    readonly property var store: api.screens.sessions
    readonly property var log: store.log

    readonly property var hints: [
        {
            glyph: "B",
            label: "Back"
        }
    ]

    signal closeRequested

    readonly property real columnX: Theme.dp(253)
    readonly property real columnWidth: Theme.dp(1667 - 253)

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
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (event.key === Qt.Key_Up) {
            event.accepted = true;
            scroll(-1);
        } else if (event.key === Qt.Key_Down) {
            event.accepted = true;
            scroll(1);
        } else if (screen) {
            event.accepted = true;
            scroll(screen * 3);
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        title: args && args.label ? String(args.label) : "Log"
        subtitle: page.game ? page.game.title : ""
        trailing: page.session === "" ? "Playing now" : page.session
    }

    Label {
        anchors.centerIn: parent
        visible: page.store.logLoading || page.store.logError !== "" || page.log.length === 0
        text: page.store.logLoading ? "Reading…" : page.store.logError !== "" ? page.store.logError : "The system journal no longer holds this session."
        color: Theme.textMuted
    }

    Flickable {
        id: flick

        x: page.columnX
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight)
        width: page.columnWidth
        contentWidth: width
        contentHeight: lines.height + Theme.dp(96)
        interactive: false
        clip: true
        visible: page.log.length > 0 && !page.store.logLoading

        Behavior on contentY {
            Ease {}
        }

        Column {
            id: lines
            y: Theme.dp(48)
            width: flick.width
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
                        color: modelData.error ? Theme.danger : modelData.warning ? Theme.barOrange : Theme.textSecondary
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

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(60)
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        flickable: flick
    }
}
