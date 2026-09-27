import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Artwork.js" as Artwork

// One slot's choices: what shows now, the default under a pick of yours, then SteamGridDB's candidates; A uses one.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property string slot: args && args.slot ? args.slot : ""
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var form: api.screens.artwork
    readonly property bool strip: true
    readonly property var current: {
        var slots = form.slots;
        return slots.find(function (s) {
            return s.slot === page.slot;
        }) || null;
    }
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

    readonly property int columns: aspect > 1.5 ? 4 : aspect > 0.9 ? 6 : 7
    readonly property real gap: Theme.dp(24)
    readonly property real captionHeight: Theme.dp(46)
    readonly property real cellWidth: Math.floor((width - Theme.dp(Theme.edge + Theme.columnRight) - gap * (columns - 1)) / columns)
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
        if (cell && cell.kind !== "now")
            items.push({
                label: cell.kind === "under" ? "Back to Default" : cell.kind === "more" ? "Load More" : "Use This",
                glyph: "check",
                act: "activate"
            });
        if (picked)
            items.push({
                label: "Back to Default",
                glyph: "refresh",
                act: "default"
            });
        items.push({
            label: "Fetch Missing Art",
            glyph: "download",
            act: "fetch"
        }, {
            label: "Wrong Game?",
            glyph: "search",
            act: "search"
        }, {
            label: "Use a File for the " + label + "…",
            glyph: "file",
            act: "file"
        });
        shell.menu(current.label, items, function (act) {
            if (act === "activate")
                page.activate();
            else if (act === "default")
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

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
        title: page.current ? page.current.label : "Artwork"
        trailing: page.form.candidatesBusy && page.candidates.length === 0 ? "Fetching…" : page.candidates.length === 0 ? "" : page.candidates.length + (page.form.more ? "+" : "") + " on SteamGridDB" + (page.form.entryDiffers ? " as " + page.form.entry : "")
    }

    Label {
        anchors.centerIn: grid
        visible: page.cells.length <= 1 && !page.form.candidatesBusy
        text: "SteamGridDB has nothing else for this slot."
        color: Theme.textMuted
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    GameCellGrid {
        id: grid

        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(20)
        height: parent.height - y - Theme.dp(90)
        focus: true
        model: page.cells
        columns: page.columns
        cellWidth: page.cellWidth
        cellHeight: page.artHeight + page.captionHeight
        gap: page.gap

        onEscapedLeft: Sound.play("edge")
        onEscapedUp: Sound.play("edge")
        onActivated: page.activate()
        onOptionsRequested: page.options()

        delegate: Item {
            Rectangle {
                id: body
                width: parent.width
                height: page.artHeight
                radius: Theme.dp(8)
                color: entry.kind === "more" ? Qt.rgba(1, 1, 1, 0.06) : page.slot === "logo" ? Qt.rgba(0.2, 0.22, 0.27, 0.9) : Qt.rgba(0.06, 0.07, 0.09, 0.9)
                border.width: entry.kind === "more" ? Theme.dp(2) : 1
                border.color: entry.kind === "more" ? Qt.rgba(1, 1, 1, 0.18) : Theme.glassEdge
                opacity: entry.kind === "under" ? 0.6 : 1.0
                clip: true

                Image {
                    anchors.fill: parent
                    anchors.margins: page.slot === "logo" ? Theme.dp(10) : 1
                    source: entry.url
                    fillMode: page.slot === "logo" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
                    asynchronous: true
                    sourceSize.width: 640
                }

                Glyph {
                    anchors.centerIn: parent
                    visible: entry.kind === "more"
                    width: Theme.dp(44)
                    height: width
                    kind: "plus"
                    tint: Theme.textSecondary
                }
            }

            Rectangle {
                anchors.right: body.right
                anchors.top: body.top
                anchors.margins: Theme.dp(10)
                width: nowText.implicitWidth + Theme.dp(18)
                height: nowText.implicitHeight + Theme.dp(8)
                radius: Theme.dp(4)
                visible: entry.kind === "now"
                color: page.picked ? Theme.accent : Qt.rgba(0, 0, 0, 0.6)

                Label {
                    id: nowText
                    anchors.centerIn: parent
                    text: page.picked ? "Your pick" : "Shown"
                    font.pixelSize: Theme.dp(Theme.fontTiny)
                }
            }

            FocusFrame {
                target: body
                shown: focused
                radius: body.radius
                gap: Theme.dp(3)
                line: Theme.dp(3)
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
