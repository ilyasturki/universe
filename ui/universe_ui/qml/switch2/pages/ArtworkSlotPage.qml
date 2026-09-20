import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Artwork.js" as Artwork

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property string slot: args && args.slot ? args.slot : ""
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var form: api.screens.artwork
    readonly property var current: form.slots.find(function (s) {
        return s.slot === page.slot;
    }) || null
    readonly property real aspect: current ? current.aspect : 1
    readonly property bool picked: current !== null && current.hasOverride
    readonly property bool underShown: picked && current.hasDefault
    readonly property var candidates: form.candidatesSlot === slot ? form.candidates : []

    readonly property var cells: {
        if (!current)
            return [];
        var out = [
            {
                kind: "now",
                url: current.url,
                caption: "Currently shown"
            }
        ];
        if (underShown)
            out.push({
                kind: "under",
                url: current.defaultUrl,
                caption: "Default under it"
            });
        for (var i = 0; i < candidates.length; i++)
            out.push({
                kind: "candidate",
                url: candidates[i].thumb,
                pick: candidates[i].url,
                caption: candidates[i].votes > 0 ? "▲ " + candidates[i].votes : ""
            });
        if (form.more)
            out.push({
                kind: "more",
                url: "",
                caption: form.candidatesBusy ? "…" : "More"
            });
        return out;
    }
    property alias cellIndex: grid.index
    readonly property var cell: cellIndex < cells.length ? cells[cellIndex] : null

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
            label: cell === null ? "OK" : cell.kind === "now" ? "Shown" : cell.kind === "under" ? "Back to default" : cell.kind === "more" ? "Load more" : "Use this",
            dim: cell === null || cell.kind === "now"
        }
    ]

    signal closeRequested

    focus: true

    readonly property int columns: aspect > 1.5 ? 4 : 6
    readonly property real gap: Theme.dp(22)
    readonly property real captionHeight: Theme.dp(44)
    readonly property real cellWidth: Math.floor((Theme.dp(1600) - gap * (columns - 1)) / columns)
    readonly property real artHeight: Math.round(cellWidth / aspect)

    // The "under" cell comes and goes at index 1: the ring stays on the same candidate.
    onUnderShownChanged: cellIndex = Math.max(0, cellIndex + (underShown ? 1 : -1))

    // args.slot itself: the derived `slot` may not have caught up when the handler runs.
    onArgsChanged: {
        if (args && args.slot)
            form.loadCandidates(args.slot);
    }

    function activate() {
        if (!cell || cell.kind === "now") {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        if (cell.kind === "under")
            form.removeOverride(slot);
        else if (cell.kind === "more")
            form.moreCandidates();
        else
            form.apply(slot, cell.pick);
    }

    function options() {
        if (!current) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        var label = current.label.toLowerCase();
        var items = [];
        if (picked)
            items.push({
                label: "Back to default",
                act: "default"
            });
        items.push({
            label: "Fetch missing art",
            act: "fetch"
        }, {
            label: "Wrong game?",
            act: "search"
        }, {
            label: "Use a file for the " + label + "…",
            act: "file"
        });
        shell.menu(current.label, items, function (act) {
            if (act === "default")
                form.removeOverride(page.slot);
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
                        form.useFile(page.slot, path);
                });
        });
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "album"
        title: page.current ? page.current.label : ""
        subtitle: "Artwork · " + (page.game ? page.game.title : "")
        trailing: page.form.candidatesBusy && page.candidates.length === 0 ? "Fetching…" : page.candidates.length === 0 ? "" : page.candidates.length + (page.form.more ? "+" : "") + " on SteamGridDB" + (page.form.entryDiffers ? " as " + page.form.entry : "")
    }

    CellGrid {
        id: grid

        x: Theme.dp(160)
        y: Theme.dp(200)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        focus: true
        model: page.cells
        columns: page.columns
        cellWidth: page.cellWidth
        cellHeight: page.artHeight + page.captionHeight
        gap: page.gap

        onEscapedLeft: Sound.play("edge")
        onActivated: page.activate()
        onOptionsRequested: page.options()

        delegate: Item {
            Rectangle {
                id: body
                width: parent.width
                height: page.artHeight
                radius: Theme.dp(4)
                color: entry.kind === "more" ? Theme.slot : page.slot === "logo" ? Theme.artShade : Theme.thumb
                border.width: entry.kind === "more" ? 2 : 0
                border.color: Theme.hairline
                opacity: entry.kind === "under" ? 0.6 : 1.0

                Image {
                    anchors.fill: parent
                    source: entry.url
                    fillMode: page.slot === "logo" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
                    asynchronous: true
                    sourceSize.width: 640
                }

                Label {
                    anchors.centerIn: parent
                    visible: entry.kind === "more"
                    text: "+"
                    font.pixelSize: Theme.dp(Theme.fontTitle)
                    color: Theme.textSecondary
                }
            }

            FocusOutline {
                target: body
                cornerRadius: body.radius
                shown: focused
            }

            Label {
                anchors.top: body.bottom
                anchors.topMargin: Theme.dp(10)
                width: parent.width
                horizontalAlignment: Text.AlignHCenter
                text: entry.caption
                color: entry.kind === "now" && page.picked ? Theme.accent : Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontTiny)
                elide: Text.ElideRight
            }
        }
    }
}
