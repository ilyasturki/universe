import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Artwork.js" as Artwork

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var form: api.screens.artwork
    readonly property var slots: form.slots
    property int index: 0
    readonly property var current: index >= 0 && index < slots.length ? slots[index] : null

    readonly property var hints: [ { glyph: "Start", label: "Options" }, { glyph: "B", label: "Back" }, { glyph: "A", label: "Open", dim: current === null } ]

    signal closeRequested()

    focus: true

    readonly property real rowHeight: Theme.dp(132)
    readonly property real thumbHeight: Theme.dp(96)

    onArgsChanged: {
        if (args && args.gameId)
            form.load(args.gameId);
    }
    Component.onDestruction: form.unload()
    onSlotsChanged: if (index >= slots.length) index = Math.max(0, slots.length - 1)

    Connections {
        target: page.form
        function onMessage(text) { page.shell.showToast(text); }
    }

    function open() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        shell.push("pages/ArtworkSlotPage.qml", { gameId: args.gameId, slot: current.slot });
    }

    function options() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        var slot = current.slot, label = current.label.toLowerCase();
        var items = [ { label: "Fetch missing art", act: "fetch" }, { label: "Wrong game?", act: "search" }, { label: "Use a file for the " + label + "…", act: "file" } ];
        shell.menu(current.label, items, function(act) {
            if (act === "fetch")
                form.refresh();
            else if (act === "search")
                Artwork.search(shell, form);
            else if (act === "file")
                shell.browse({ title: "Use a file for the " + label, path: "", files: true }, function(path) { if (path) form.useFile(slot, path); });
        });
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat && (api.keys.isAccept(event) || api.keys.isCancel(event)))
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            open();
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            options();
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            event.accepted = true;
            index = Sound.stepped(index, event.key === Qt.Key_Up ? -1 : 1, slots.length);
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "album"
        title: "Artwork"
        subtitle: page.game ? page.game.title : ""
    }

    Column {
        x: Theme.dp(300)
        y: Theme.dp(190)
        width: Theme.dp(1320)

        Repeater {
            model: page.slots

            Item {
                id: row

                readonly property var entry: modelData
                readonly property bool focused: page.activeFocus && index === page.index
                readonly property bool empty: entry.kind === "missing"
                readonly property color ink: entry.kind === "picked" ? Theme.accent : entry.kind === "missing" ? Theme.danger : Theme.textSecondary

                width: parent.width
                height: page.rowHeight

                FocusPill {
                    anchors.fill: parent
                    focused: row.focused
                }

                Hairline {
                    visible: !row.focused && index < page.slots.length - 1
                }

                Item {
                    id: thumb
                    x: Theme.dp(28)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(Theme.dp(300), Math.round(page.thumbHeight * row.entry.aspect))
                    height: page.thumbHeight

                    Rectangle {
                        anchors.fill: parent
                        radius: Theme.dp(4)
                        color: row.empty ? Theme.slot : row.entry.slot === "logo" ? Theme.artShade : Theme.thumb
                        border.width: row.empty ? 2 : 0
                        border.color: Theme.hairline
                    }

                    Image {
                        anchors.fill: parent
                        source: row.entry.url
                        fillMode: row.entry.slot === "logo" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
                        asynchronous: true
                        sourceSize.width: 600
                    }

                    Label {
                        anchors.centerIn: parent
                        visible: row.empty
                        text: "Nothing yet"
                        color: Theme.textMuted
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                    }
                }

                Column {
                    anchors.left: parent.left
                    anchors.leftMargin: Theme.dp(28) + Theme.dp(300) + Theme.dp(28)
                    anchors.right: stateLabel.left
                    anchors.rightMargin: Theme.dp(24)
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.dp(2)

                    Label {
                        width: parent.width
                        text: row.entry.label
                        elide: Text.ElideRight
                    }

                    Label {
                        width: parent.width
                        text: row.entry.use
                        color: Theme.textSecondary
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                        elide: Text.ElideRight
                    }
                }

                Label {
                    id: stateLabel
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.dp(28)
                    anchors.verticalCenter: parent.verticalCenter
                    text: row.entry.kindLabel
                    color: row.ink
                    font.pixelSize: Theme.dp(30)
                }
            }
        }
    }
}
