import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Artwork.js" as Artwork

// A game's artwork: one card per slot with what shows there now and where it came from; A opens the slot's choices.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var form: api.screens.artwork
    readonly property var slots: form.slots
    property alias index: grid.index
    readonly property var current: index >= 0 && index < slots.length ? slots[index] : null
    readonly property bool strip: true

    readonly property var hints: [
        {
            glyph: "Start",
            label: "Options"
        },
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: "Open",
            dim: current === null
        }
    ]

    signal closeRequested

    focus: true

    readonly property real gap: Theme.dp(32)
    readonly property real cellWidth: Math.floor((width - Theme.dp(Theme.edge + Theme.columnRight) - gap * 2) / 3)
    readonly property real artHeight: Math.round(cellWidth * 0.56)

    onArgsChanged: {
        if (args && args.gameId)
            form.load(args.gameId);
    }
    Component.onDestruction: form.unload()

    Connections {
        target: page.form
        function onMessage(text) {
            page.shell.showToast(text);
        }
    }

    function open() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        shell.push("pages/ArtworkSlotPage.qml", {
            gameId: args.gameId,
            slot: current.slot
        });
    }

    function options() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        var slot = current.slot, label = current.label.toLowerCase();
        var items = [
            {
                label: "Open " + current.label,
                glyph: "image",
                act: "open"
            },
            {
                label: "Fetch Missing Art",
                glyph: "download",
                act: "fetch"
            },
            {
                label: "Wrong Game?",
                glyph: "search",
                act: "search"
            },
            {
                label: "Use a File for the " + label + "…",
                glyph: "file",
                act: "file"
            }
        ];
        shell.menu(current.label, items, function (act) {
            if (act === "open")
                page.open();
            else if (act === "fetch")
                form.refresh();
            else if (act === "search")
                Artwork.search(shell, form);
            else if (act === "file")
                shell.browse({
                    title: "Use a file for the " + label,
                    path: "",
                    files: true
                }, function (path) {
                    if (path)
                        form.useFile(slot, path);
                });
        });
    }

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
        title: "Artwork"
        trailing: page.form.entryDiffers ? "On " + page.form.catalogue + " as " + page.form.entry : ""
    }

    GameCellGrid {
        id: grid

        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(20)
        height: parent.height - y - Theme.dp(90)
        focus: true
        model: page.slots
        columns: 3
        cellWidth: page.cellWidth
        cellHeight: page.artHeight + Theme.dp(100)
        gap: page.gap

        onEscapedLeft: Sound.play("edge")
        onEscapedUp: Sound.play("edge")
        onActivated: page.open()
        onOptionsRequested: page.options()

        delegate: Item {
            readonly property bool empty: entry.kind === "missing"
            readonly property bool fit: entry.slot === "box_front" || entry.slot === "square" || entry.slot === "logo"

            Rectangle {
                id: art
                width: parent.width
                height: page.artHeight
                radius: Theme.dp(Theme.radiusCard)
                color: entry.slot === "logo" ? Qt.rgba(0.2, 0.22, 0.27, 0.9) : Qt.rgba(0.06, 0.07, 0.09, 0.9)
                border.width: parent.empty ? Theme.dp(2) : 1
                border.color: parent.empty ? Qt.rgba(1, 1, 1, 0.18) : Theme.glassEdge
                clip: true

                Image {
                    anchors.fill: parent
                    anchors.margins: parent.parent.fit ? Theme.dp(14) : 1
                    source: entry.url
                    fillMode: parent.parent.fit ? Image.PreserveAspectFit : Image.PreserveAspectCrop
                    asynchronous: true
                    mipmap: true
                    sourceSize.width: 720
                }

                Column {
                    anchors.centerIn: parent
                    visible: parent.parent.empty
                    spacing: Theme.dp(10)

                    Glyph {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: Theme.dp(44)
                        height: width
                        kind: "image"
                        tint: Theme.textMuted
                    }

                    Label {
                        text: "Nothing yet"
                        color: Theme.textMuted
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }
                }
            }

            FocusFrame {
                target: art
                shown: focused
                radius: Theme.dp(Theme.radiusCard)
                gap: Theme.dp(3)
                line: Theme.dp(3)
            }

            Label {
                id: slotName
                anchors.top: art.bottom
                anchors.topMargin: Theme.dp(16)
                width: parent.width - pill.width - Theme.dp(16)
                text: entry.label
                elide: Text.ElideRight
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(28)
            }

            Rectangle {
                id: pill
                anchors.right: parent.right
                anchors.verticalCenter: slotName.verticalCenter
                width: kindText.implicitWidth + Theme.dp(20)
                height: kindText.implicitHeight + Theme.dp(8)
                radius: Theme.dp(4)
                color: "transparent"
                border.width: Theme.dp(1.5)
                border.color: entry.kind === "picked" ? Theme.accent : entry.kind === "missing" ? Theme.danger : Qt.rgba(1, 1, 1, 0.5)

                Label {
                    id: kindText
                    anchors.centerIn: parent
                    text: entry.kindLabel
                    color: entry.kind === "picked" ? Theme.accent : entry.kind === "missing" ? Theme.danger : Theme.textSecondary
                    font.pixelSize: Theme.dp(Theme.fontTiny)
                }
            }

            Label {
                anchors.top: slotName.bottom
                anchors.topMargin: Theme.dp(4)
                width: parent.width
                text: entry.use
                color: Theme.textMuted
                elide: Text.ElideRight
                font.pixelSize: Theme.dp(Theme.fontTiny)
            }
        }
    }
}
