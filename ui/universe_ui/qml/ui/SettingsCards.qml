import QtQuick
import "../core"
import "../sound"

// groups: { title, meta, warning, caps, control, off, rows, icon }; `rows` and `control` index the flat list.
FocusScope {
    id: cards

    property var rows: []
    property var groups: []
    property int columns: 2
    property int index: 0
    property bool compact: false
    property bool dimmed: false

    signal activated(int index, var row)
    signal escapedUp()
    signal escapedLeft()

    readonly property var currentRow: index >= 0 && index < rows.length ? rows[index] : null
    readonly property bool cursorShown: activeFocus || dimmed

    readonly property real gap: Theme.dp(compact ? 24 : 32)
    readonly property real pad: Theme.dp(compact ? 6 : 8)
    readonly property real rowHeight: Theme.dp(compact ? 60 : 66)
    readonly property real columnWidth: (width - gap * (columns - 1)) / columns
    readonly property rect focusRect: {
        var s = stopOf(index);
        if (!s)
            return Qt.rect(0, 0, 0, 0);
        return Qt.rect(columnX(s.col) + 1 + pad, s.y0 - view.contentY, columnWidth - 2 - pad * 2, s.y1 - s.y0);
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
        return pad * 2 + headerHeight(g) + g.rows.length * rowHeight + 2;
    }

    // Each card joins the shortest column; a stop's `top` is the card's top for its first stop, so the header comes into view with it.
    readonly property var layout: {
        var cardsOut = [], stops = [], tops = [], c, i, r;
        for (c = 0; c < columns; c++) {
            stops.push([]);
            tops.push(0);
        }
        for (i = 0; i < groups.length; i++) {
            var group = groups[i];
            c = 0;
            for (var k = 1; k < columns; k++)
                if (tops[k] < tops[c])
                    c = k;
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
        var height = Math.max.apply(null, tops);
        return { cards: cardsOut, stops: stops, height: height > 0 ? height - gap : 0 };
    }

    function stopOf(row) {
        for (var c = 0; c < layout.stops.length; c++)
            for (var i = 0; i < layout.stops[c].length; i++)
                if (layout.stops[c][i].row === row)
                    return layout.stops[c][i];
        return null;
    }

    // The first row that is not a search field; the field is reached by going up from it.
    function firstStop() {
        var fallback = null;
        for (var c = 0; c < layout.stops.length; c++)
            for (var i = 0; i < layout.stops[c].length; i++) {
                var s = layout.stops[c][i];
                if (!fallback)
                    fallback = s;
                var row = rows[s.row];
                if (!row || row.type !== "search")
                    return s;
            }
        return fallback;
    }

    function reset() {
        var s = firstStop();
        index = s ? s.row : 0;
    }

    function go(stop) {
        index = stop.row;
        Sound.tick();
    }

    function step(d) {
        var s = stopOf(index);
        var list = s ? layout.stops[s.col] : [];
        var pos = s ? list.indexOf(s) + d : -1;
        if (pos >= 0 && pos < list.length)
            go(list[pos]);
        else if (d < 0)
            cards.escapedUp();
        else
            Sound.edge();
    }

    function cross(d) {
        var s = stopOf(index);
        var col = s ? s.col + d : -1;
        if (col < 0) {
            cards.escapedLeft();
            return;
        }
        if (col >= columns || layout.stops[col].length === 0) {
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
        }
    }

    component SettingsCard: Item {
        id: card

        property var group: ({})
        readonly property bool hasIcon: group.icon != null && String(group.icon) !== "" && logo.status === Image.Ready
        readonly property real headerHeight: cards.headerHeight(group)

        readonly property bool hasControl: group.control >= 0
        readonly property bool headerFocused: hasControl && cards.index === group.control && cards.cursorShown
        readonly property color onFocus: Qt.rgba(0.063, 0.067, 0.086, 0.6)

        height: cards.cardHeight(group)
        opacity: group.off && !headerFocused ? 0.55 : 1.0

        Behavior on opacity { Ease { duration: Theme.durQuick } }

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(24)
            color: Qt.rgba(1, 1, 1, 0.04)
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.10)
        }

        Rectangle {
            id: head

            x: 1 + cards.pad
            y: 1 + cards.pad
            width: parent.width - 2 - cards.pad * 2
            height: card.headerHeight
            radius: Theme.dp(14)
            visible: card.headerHeight > 0
            color: card.headerFocused ? Theme.text : "transparent"

            Behavior on color { ColorEase {} }

            Image {
                id: logo
                anchors.left: parent.left
                anchors.leftMargin: Theme.dp(16)
                anchors.verticalCenter: parent.verticalCenter
                height: parent.height - Theme.dp(24)
                width: height
                source: card.group.icon ? Qt.resolvedUrl("../" + card.group.icon) : ""
                asynchronous: true
                fillMode: Image.PreserveAspectFit
                sourceSize.height: 128
                smooth: true
                mipmap: true
                visible: card.hasIcon
            }

            Column {
                anchors.left: parent.left
                anchors.leftMargin: card.hasIcon ? logo.width + Theme.dp(30) : Theme.dp(18)
                anchors.right: toggle.visible ? toggle.left : parent.right
                anchors.rightMargin: Theme.dp(20)
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(2)

                CapsLabel {
                    visible: card.group.caps === true
                    text: (card.group.title || "").toUpperCase()
                    color: card.headerFocused ? card.onFocus : Theme.textMuted
                }

                Text {
                    visible: card.group.caps !== true
                    width: parent.width
                    text: card.group.title || ""
                    color: card.headerFocused ? Theme.onLight : Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Bold
                    font.pixelSize: Theme.dp(cards.compact ? 24 : 27)
                    elide: Text.ElideRight
                }

                Text {
                    visible: text !== ""
                    width: parent.width
                    text: (card.group.meta || "")
                          + (card.group.warning
                             ? (card.group.meta ? " · " : "") + "<font color=\"#e0655a\">" + card.group.warning + "</font>"
                             : "")
                    textFormat: Text.StyledText
                    color: card.headerFocused ? card.onFocus : Theme.textMuted
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(cards.compact ? 18 : 19)
                    elide: Text.ElideRight
                }
            }

            SettingsToggle {
                id: toggle
                visible: card.hasControl
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(16)
                anchors.verticalCenter: parent.verticalCenter
                on: card.hasControl && cards.rows[card.group.control] ? cards.rows[card.group.control].value === true : false
                focused: card.headerFocused
            }
        }

        Column {
            x: 1 + cards.pad
            y: 1 + cards.pad + card.headerHeight
            width: parent.width - 2 - cards.pad * 2

            Repeater {
                model: card.group.rows

                SettingsRow {
                    readonly property bool prevFocused: index > 0 && card.group.rows[index - 1] === cards.index && cards.cursorShown

                    width: parent.width
                    height: cards.rowHeight
                    entry: cards.rows[modelData] || ({})
                    focused: modelData === cards.index && cards.cursorShown
                    compact: cards.compact
                    separator: index > 0 && !focused && !prevFocused
                }
            }
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

        Behavior on opacity { Ease { duration: Theme.durQuick } }

        function scrollToCurrent() {
            var s = cards.stopOf(cards.index);
            if (s && height > 0)
                Theme.reveal(view, s.top, s.y1, height);
        }

        Behavior on contentY { Ease { duration: Theme.durView; easing.type: Easing.OutQuint } }

        Repeater {
            model: cards.layout.cards

            SettingsCard {
                x: cards.columnX(modelData.col)
                y: modelData.y
                width: cards.columnWidth
                group: cards.groups[modelData.group]
            }
        }
    }
}
