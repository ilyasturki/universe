import QtQuick
import "../core"
import "../sound"

// Settings as cards in two columns, driven by the flat rows the host builds and the groups
// that arrange them: { title, meta, warning, caps, control, off, span, rows }, `rows` and
// `control` indexing the flat list. A spanning group is a bar across both columns (search).
// The cursor walks a column and crosses to the nearest row; the page decides what A opens.
FocusScope {
    id: cards

    property var rows: []
    property var groups: []
    property int index: 0
    property bool compact: false
    property bool dimmed: false

    signal activated(int index, var row)
    signal escapedUp()

    readonly property var currentRow: index >= 0 && index < rows.length ? rows[index] : null
    // The white row stays while a picker or the keyboard opened from it holds the focus.
    readonly property bool cursorShown: activeFocus || dimmed

    readonly property real gap: Theme.dp(compact ? 24 : 32)
    readonly property real pad: Theme.dp(compact ? 6 : 8)
    readonly property real rowHeight: Theme.dp(compact ? 60 : 66)
    readonly property real barHeight: Theme.dp(66)
    readonly property real columnWidth: (width - gap) / 2
    // The focused stop, in the cards' own coordinates, for whatever the page drops from it.
    readonly property rect focusRect: {
        var s = stopOf(index);
        if (!s)
            return Qt.rect(0, 0, 0, 0);
        var x = s.col < 0 ? 0 : columnX(s.col) + 1 + pad;
        var w = s.col < 0 ? width : columnWidth - 2 - pad * 2;
        return Qt.rect(x, s.y0 - view.contentY, w, s.y1 - s.y0);
    }

    function columnX(c) { return c * (columnWidth + gap); }

    function headerHeight(g) {
        if (!g.title)
            return 0;
        if (g.meta || g.warning)
            return Theme.dp(compact ? 66 : 74);
        return Theme.dp(compact ? 56 : 66);
    }

    function cardHeight(g) {
        return g.span ? barHeight : pad * 2 + headerHeight(g) + g.rows.length * rowHeight + 2;
    }

    // Spanning groups stack at the top; each card then joins the shorter column, so the
    // columns end close together. Every focus stop gets its y range and the y to reveal
    // (the card's top for a card's first stop, so its header comes into view with it).
    readonly property var layout: {
        var bars = [], cardsOut = [], lead = [], stops = [[], []], y = 0, i, r;
        for (i = 0; i < groups.length; i++) {
            var g = groups[i];
            if (!g.span)
                continue;
            bars.push({ group: i, y: y });
            lead.push({ row: g.rows[0], col: -1, top: y, y0: y, y1: y + barHeight, lead: true });
            y += barHeight + gap;
        }
        var tops = [y, y];
        for (i = 0; i < groups.length; i++) {
            var group = groups[i];
            if (group.span)
                continue;
            var c = tops[1] < tops[0] ? 1 : 0;
            var top = tops[c];
            cardsOut.push({ group: i, col: c, y: top });
            var cy = top + 1 + pad;
            if (group.control >= 0)
                stops[c].push({ row: group.control, col: c, top: top, y0: cy, y1: cy + headerHeight(group) });
            cy += headerHeight(group);
            for (r = 0; r < group.rows.length; r++) {
                var first = r === 0 && !(group.control >= 0);
                stops[c].push({ row: group.rows[r], col: c, top: first ? top : cy, y0: cy, y1: cy + rowHeight });
                cy += rowHeight;
            }
            tops[c] = top + cardHeight(group) + gap;
        }
        var height = Math.max(tops[0], tops[1], y);
        return { bars: bars, cards: cardsOut, lead: lead, stops: stops, height: height > 0 ? height - gap : 0 };
    }

    function stopOf(row) {
        var lists = [layout.lead, layout.stops[0], layout.stops[1]];
        for (var l = 0; l < lists.length; l++)
            for (var i = 0; i < lists[l].length; i++)
                if (lists[l][i].row === row)
                    return lists[l][i];
        return null;
    }

    function firstStop() {
        if (layout.lead.length > 0)
            return layout.lead[0];
        for (var c = 0; c < 2; c++)
            if (layout.stops[c].length > 0)
                return layout.stops[c][0];
        return null;
    }

    function reset() {
        var s = firstStop();
        index = s ? s.row : 0;
    }

    // The column the cursor last stood in, so leaving the search bar comes back to it.
    property int lastColumn: 0

    function go(stop) {
        index = stop.row;
        if (stop.col >= 0)
            lastColumn = stop.col;
        Sound.tick();
    }

    function step(d) {
        var s = stopOf(index);
        if (!s) {
            if (d < 0)
                cards.escapedUp();
            else
                Sound.edge();
            return;
        }
        var list = s.lead ? layout.lead : layout.stops[s.col];
        var pos = list.indexOf(s) + d;
        if (pos >= 0 && pos < list.length) {
            go(list[pos]);
            return;
        }
        if (s.lead) {
            if (d < 0) {
                cards.escapedUp();
                return;
            }
            var col = layout.stops[lastColumn].length > 0 ? lastColumn : 1 - lastColumn;
            if (layout.stops[col].length > 0)
                go(layout.stops[col][0]);
            else
                Sound.edge();
            return;
        }
        if (d < 0) {
            if (layout.lead.length > 0)
                go(layout.lead[layout.lead.length - 1]);
            else
                cards.escapedUp();
            return;
        }
        Sound.edge();
    }

    function cross(d) {
        var s = stopOf(index);
        var col = s && !s.lead ? s.col + d : -1;
        if (col < 0 || col > 1 || layout.stops[col].length === 0) {
            Sound.edge();
            return;
        }
        var centre = (s.y0 + s.y1) / 2, best = null, dist = 0;
        for (var i = 0; i < layout.stops[col].length; i++) {
            var t = layout.stops[col][i];
            var dd = Math.abs((t.y0 + t.y1) / 2 - centre);
            if (!best || dd < dist) {
                best = t;
                dist = dd;
            }
        }
        go(best);
    }

    onLayoutChanged: {
        if (!stopOf(index))
            reset();
        view.scrollToCurrent();
    }
    onIndexChanged: view.scrollToCurrent()
    onHeightChanged: view.scrollToCurrent()

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)
    Keys.onLeftPressed: cross(-1)
    Keys.onRightPressed: cross(1)

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (currentRow && currentRow.type !== "info")
                cards.activated(index, currentRow);
            else
                Sound.edge();
            return;
        }
    }

    Text {
        anchors.centerIn: parent
        visible: cards.rows.length === 0
        text: "Nothing here yet."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    Flickable {
        id: view

        anchors.fill: parent
        contentWidth: width
        contentHeight: cards.layout.height
        interactive: false
        clip: true
        opacity: cards.dimmed ? 0.55 : 1.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        function scrollToCurrent() {
            var s = cards.stopOf(cards.index);
            if (!s || height <= 0)
                return;
            var target = contentY;
            if (s.top < contentY)
                target = s.top;
            else if (s.y1 > contentY + height)
                target = s.y1 - height;
            contentY = Math.max(0, Math.min(target, Math.max(0, contentHeight - height)));
        }

        Behavior on contentY {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutQuint }
        }

        Repeater {
            model: cards.layout.cards

            SettingsCard {
                x: cards.columnX(modelData.col)
                y: modelData.y
                width: cards.columnWidth
                group: cards.groups[modelData.group]
                rows: cards.rows
                cursor: cards.index
                active: cards.cursorShown
                compact: cards.compact
                pad: cards.pad
                headerHeight: cards.headerHeight(group)
                rowHeight: cards.rowHeight
            }
        }

        Repeater {
            model: cards.layout.bars

            SettingsSearchBar {
                y: modelData.y
                width: cards.width
                height: cards.barHeight
                entry: cards.rows[cards.groups[modelData.group].rows[0]] || ({})
                focused: cards.index === cards.groups[modelData.group].rows[0] && cards.cursorShown
            }
        }
    }
}
