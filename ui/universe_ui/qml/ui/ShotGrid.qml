import QtQuick
import "../core"
import "../sound"

// The player's shots, `columns` to a line; a hairline parts those taken since `since` (a session's started_at; "" for one run) from the rest.
Item {
    id: grid

    property var rows: []
    property string since: ""
    property int index: 0
    property bool active: true
    property int columns: 4
    property real gap: Theme.dp(24)
    property real sideMargin: Theme.dp(90)
    property real topPadding: Theme.dp(40)

    readonly property real cellWidth: (width - sideMargin * 2 + gap) / columns
    readonly property real cellHeight: (cellWidth - gap) * 9 / 16 + gap
    readonly property real sectionHeight: Theme.dp(40)

    readonly property var split: {
        var start = since === "" ? NaN : Date.parse(since);
        var mine = [], earlier = [];
        rows.forEach(function(r) {
            var t = Date.parse(r.taken_at);
            (!isNaN(start) && !isNaN(t) && t >= start ? mine : earlier).push(r);
        });
        return { mine: mine, earlier: earlier };
    }
    readonly property var ordered: split.mine.concat(split.earlier)
    readonly property int mine: split.mine.length
    readonly property var lines: {
        var out = [];
        function chunk(items, group, first) {
            for (var i = 0; i < items.length; i += columns)
                out.push({ group: group, first: first + i, items: items.slice(i, i + columns) });
        }
        chunk(split.mine, "", 0);
        chunk(split.earlier, split.mine.length > 0 ? "earlier" : "", split.mine.length);
        return out;
    }
    readonly property int line: lineOf(index)
    readonly property var current: index >= 0 && index < ordered.length ? ordered[index] : null

    function lineOf(i) {
        for (var l = 0; l < lines.length; l++)
            if (lines[l].items.length > 0 && i >= lines[l].first && i < lines[l].first + lines[l].items.length)
                return l;
        return -1;
    }

    function step(d) {
        index = Sound.stepped(index, d, ordered.length);
    }

    // False at the edge: the caller decides what lies past it.
    function stepLine(d) {
        var l = lineOf(index);
        if (l < 0)
            return false;
        var col = index - lines[l].first;
        var n = l + d;
        while (n >= 0 && n < lines.length && lines[n].items.length === 0)
            n += d;
        if (n < 0 || n >= lines.length)
            return false;
        Sound.tick();
        index = Math.min(lines[n].first + col, lines[n].first + lines[n].items.length - 1);
        return true;
    }

    function currentCard() {
        var item = list.currentItem;
        return line >= 0 && item && item.cards ? (item.cards.itemAt(index - lines[line].first) || grid) : grid;
    }

    ListView {
        id: list

        Wheel {}

        anchors.fill: parent
        anchors.leftMargin: grid.sideMargin - grid.gap / 2
        anchors.rightMargin: grid.sideMargin - grid.gap / 2
        clip: true
        model: grid.lines
        currentIndex: grid.line
        interactive: false
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: grid.topPadding
        preferredHighlightEnd: height - Theme.dp(40)
        highlightRangeMode: ListView.ApplyRange
        highlightMoveDuration: Theme.durView
        header: Item { height: grid.topPadding }

        section.property: "group"
        section.criteria: ViewSection.FullString
        section.delegate: Item {
            width: list.width
            height: section === "" ? 0 : grid.sectionHeight
            visible: section !== ""

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.leftMargin: grid.gap / 2
                anchors.rightMargin: grid.gap / 2
                anchors.verticalCenter: parent.verticalCenter
                height: 1
                color: Theme.surfaceBorder
            }
        }

        delegate: Item {
            id: lineItem

            readonly property var line: modelData
            readonly property var cards: repeater

            width: list.width
            height: grid.cellHeight

            Repeater {
                id: repeater
                model: modelData.items

                ShotCard {
                    readonly property int flat: lineItem.line.first + index

                    x: index * grid.cellWidth + grid.gap / 2
                    y: grid.gap / 2
                    width: grid.cellWidth - grid.gap
                    height: grid.cellHeight - grid.gap
                    // `version` is read so the card repaints when its thumbnail lands.
                    source: (api.screens.thumbs.version, api.screens.thumbs.url(modelData.thumb))
                    caption: modelData.dateText
                    focused: grid.active && flat === grid.index
                    dimmed: grid.active && flat !== grid.index
                    journaled: modelData.hasJournal

                    Pointer {
                        current: grid.active && flat === grid.index
                        radius: Theme.dp(12)
                        onPicked: grid.index = flat
                    }
                }
            }
        }
    }
}
