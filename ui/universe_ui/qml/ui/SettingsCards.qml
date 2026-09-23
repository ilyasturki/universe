import QtQuick
import "../core"
import "../sound"

// groups: { title, meta, warning, caps, control, off, rows, icon, wide, divider, dividers }; `rows` and `control` index the flat
// list; a `wide` card spans the columns; the rows from `divider` on are the card's advanced ones, each folded card ruled off
// under its label ([{ at, label }]).
FocusScope {
    id: cards

    property var rows: []
    property var groups: []
    property int columns: 2
    property int index: 0
    property bool compact: false
    property bool dimmed: false

    signal activated(int index, var row)
    signal escapedUp
    signal escapedDown
    signal escapedLeft

    readonly property var currentRow: index >= 0 && index < rows.length ? rows[index] : null
    readonly property bool cursorShown: activeFocus || dimmed
    // The focused row's detail, up to two lines under the cards; an info row prints its own inline.
    readonly property string caption: currentRow && currentRow.type !== "info" ? currentRow.detail || "" : ""
    readonly property bool hasCaptions: rows.some(function (r) {
        return r.type !== "info" && r.detail;
    })
    readonly property real captionHeight: hasCaptions ? Theme.dp(compact ? 78 : 86) : 0

    readonly property real gap: Theme.dp(compact ? 24 : 32)
    readonly property real pad: Theme.dp(compact ? 6 : 8)
    readonly property real rowHeight: Theme.dp(compact ? 60 : 66)
    // Row index → height, for the rows that grow past rowHeight; each row reports its own once laid out.
    property var tallRows: ({})
    readonly property real dividerHeight: Theme.dp(compact ? 34 : 38)
    readonly property real columnWidth: (width - gap * (columns - 1)) / columns
    readonly property rect focusRect: {
        var s = stopOf(index);
        if (!s)
            return Qt.rect(0, 0, 0, 0);
        return Qt.rect(columnX(s.col) + 1 + pad, s.y0 - view.contentY, (s.wide ? width : columnWidth) - 2 - pad * 2, s.y1 - s.y0);
    }

    function heightOf(row) {
        var h = tallRows[row];
        return h !== undefined ? h : rowHeight;
    }

    // A row reports while `layout` builds it: applied on the next tick, else the layout would depend on itself.
    property var pendingHeights: ({})

    function measured(row, h) {
        pendingHeights[row] = h;
        Qt.callLater(applyHeights);
    }

    function applyHeights() {
        var next = Object.assign({}, tallRows), changed = false;
        for (var row in pendingHeights) {
            var h = pendingHeights[row];
            if (h > rowHeight ? next[row] === h : next[row] === undefined)
                continue;
            if (h > rowHeight)
                next[row] = h;
            else
                delete next[row];
            changed = true;
        }
        pendingHeights = {};
        if (changed)
            tallRows = next;
    }

    function columnX(c) {
        return c * (columnWidth + gap);
    }

    function headerHeight(g) {
        if (!g.title)
            return 0;
        if (g.meta || g.warning)
            return Theme.dp(compact ? 66 : 74);
        return Theme.dp(compact ? 56 : 66);
    }

    function hasDivider(g) {
        return g.divider !== undefined && g.divider >= 0 && g.divider < g.rows.length;
    }

    // The rule over the card's r-th row, "" for none: the labelled ones, else the one plain Advanced rule at `divider`.
    function ruleAt(g, r) {
        if (g.dividers !== undefined && g.dividers.length > 0) {
            var d = g.dividers.find(function (d) {
                return d.at === r;
            });
            return d ? d.label : "";
        }
        return hasDivider(g) && r === g.divider ? "Advanced" : "";
    }

    function ruleCount(g) {
        if (g.dividers !== undefined && g.dividers.length > 0)
            return g.dividers.filter(function (d) {
                return d.at < g.rows.length;
            }).length;
        return hasDivider(g) ? 1 : 0;
    }

    function cardHeight(g) {
        var rowsHeight = g.rows.reduce(function (sum, r) {
            return sum + heightOf(r);
        }, 0);
        return pad * 2 + headerHeight(g) + rowsHeight + ruleCount(g) * dividerHeight + 2;
    }

    // Each card joins the shortest column; a stop's `top` is the card's top for its first stop, so the header comes into view with it.
    readonly property var layout: {
        var cardsOut = [], stops = [], tops = [], c, i, r;
        for (c = 0; c < columns; c++) {
            stops.push([]);
            tops.push(0);
        }
        for (i = 0; i < groups.length; i++) {
            var group = groups[i], wide = group.wide === true, k;
            c = 0;
            var top = tops[0];
            if (wide) {
                for (k = 1; k < columns; k++)
                    top = Math.max(top, tops[k]);
            } else {
                for (k = 1; k < columns; k++)
                    if (tops[k] < tops[c])
                        c = k;
                top = tops[c];
            }
            cardsOut.push({
                group: i,
                col: c,
                y: top,
                wide: wide
            });
            var cy = top + 1 + pad;
            if (group.control >= 0)
                stops[c].push({
                    row: group.control,
                    col: c,
                    top: top,
                    y0: cy,
                    y1: cy + headerHeight(group),
                    bottom: cy + headerHeight(group) + (group.rows.length ? 0 : pad + 1),
                    wide: wide
                });
            cy += headerHeight(group);
            for (r = 0; r < group.rows.length; r++) {
                var first = r === 0 && !(group.control >= 0), last = r === group.rows.length - 1;
                if (ruleAt(group, r) !== "")
                    cy += dividerHeight;
                var h = heightOf(group.rows[r]);
                stops[c].push({
                    row: group.rows[r],
                    col: c,
                    top: first ? top : cy,
                    y0: cy,
                    y1: cy + h,
                    bottom: cy + h + (last ? pad + 1 : 0),
                    wide: wide
                });
                cy += h;
            }
            for (k = 0; k < columns; k++)
                if (wide || k === c)
                    tops[k] = top + cardHeight(group) + gap;
        }
        var height = Math.max.apply(null, tops);
        return {
            cards: cardsOut,
            stops: stops,
            height: height > 0 ? height - gap : 0
        };
    }

    function stopOf(row) {
        for (var c = 0; c < layout.stops.length; c++)
            for (var i = 0; i < layout.stops[c].length; i++)
                if (layout.stops[c][i].row === row)
                    return layout.stops[c][i];
        return null;
    }

    function firstStop() {
        for (var c = 0; c < layout.stops.length; c++)
            if (layout.stops[c].length > 0)
                return layout.stops[c][0];
        return null;
    }

    function reset() {
        var s = firstStop();
        index = s ? s.row : 0;
    }

    function go(stop) {
        index = stop.row;
        Sound.tick();
    }

    // The mouse on a row: the cursor lands there in silence, and the focus comes to the cards.
    signal pointed

    function pointTo(row) {
        index = row;
        forceActiveFocus();
        pointed();
    }

    // The Advanced row just opened: the cursor moves onto the first row it revealed, the first card of its own after it,
    // else the first row folded into a card above.
    function stepInto() {
        var k = groups.findIndex(function (g) {
            return g.rows.indexOf(index) >= 0;
        });
        if (k >= 0 && k + 1 < groups.length && groups[k + 1].rows.length > 0) {
            index = groups[k + 1].rows[0];
            return;
        }
        var folded = groups.find(hasDivider);
        if (folded)
            index = folded.rows[folded.divider];
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
            cards.escapedDown();
    }

    function stepScreen(d) {
        var s = stopOf(index);
        if (!s)
            return;
        var list = layout.stops[s.col];
        var centre = (s.y0 + s.y1) / 2 + d * view.height, best = s, dist = Infinity;
        for (var i = 0; i < list.length; i++) {
            var dd = Math.abs((list[i].y0 + list[i].y1) / 2 - centre);
            if (dd < dist) {
                best = list[i];
                dist = dd;
            }
        }
        if (best === s)
            best = list[d < 0 ? 0 : list.length - 1];
        best === s ? Sound.edge() : go(best);
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

    Keys.onPressed: function (event) {
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (screen) {
            event.accepted = true;
            stepScreen(screen);
            return;
        }
        if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            event.accepted = true;
            var s = stopOf(index), list = s ? layout.stops[s.col] : [];
            var end = list.length ? list[api.keys.isFirst(event) ? 0 : list.length - 1] : null;
            !end || end === s ? Sound.edge() : go(end);
            return;
        }
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

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }

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

            Behavior on color {
                ColorEase {}
            }

            Pointer {
                enabled: card.hasControl
                direct: true
                radius: Theme.dp(14)
                onPicked: cards.pointTo(card.group.control)
            }

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
                    text: (card.group.meta || "") + (card.group.warning ? (card.group.meta ? " · " : "") + "<font color=\"#e0655a\">" + card.group.warning + "</font>" : "")
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

                Item {
                    id: slot

                    readonly property string rule: cards.ruleAt(card.group, index)
                    readonly property bool divided: rule !== ""
                    readonly property bool prevFocused: index > 0 && card.group.rows[index - 1] === cards.index && cards.cursorShown

                    width: parent.width
                    height: cards.heightOf(modelData) + (divided ? cards.dividerHeight : 0)

                    Item {
                        visible: parent.divided
                        width: parent.width
                        height: cards.dividerHeight

                        CapsLabel {
                            id: divLabel
                            anchors.left: parent.left
                            anchors.leftMargin: Theme.dp(18)
                            anchors.verticalCenter: parent.verticalCenter
                            text: slot.rule.toUpperCase()
                            size: Theme.dp(cards.compact ? 14 : 15)
                            color: Theme.textFaint
                        }

                        Rectangle {
                            anchors.left: divLabel.right
                            anchors.right: parent.right
                            anchors.leftMargin: Theme.dp(14)
                            anchors.rightMargin: Theme.dp(16)
                            anchors.verticalCenter: parent.verticalCenter
                            height: 1
                            color: Qt.rgba(1, 1, 1, 0.10)
                        }
                    }

                    SettingsRow {
                        anchors.bottom: parent.bottom
                        width: parent.width
                        height: cards.heightOf(modelData)
                        baseHeight: cards.rowHeight
                        entry: cards.rows[modelData] || ({})
                        focused: modelData === cards.index && cards.cursorShown
                        compact: cards.compact
                        separator: index > 0 && !parent.divided && !focused && !parent.prevFocused

                        onNaturalHeightChanged: cards.measured(modelData, naturalHeight)
                        Component.onCompleted: cards.measured(modelData, naturalHeight)

                        Pointer {
                            direct: true
                            radius: Theme.dp(14)
                            onPicked: cards.pointTo(modelData)
                        }
                    }
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

    Text {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: Theme.dp(18)
        anchors.rightMargin: Theme.dp(18)
        height: cards.captionHeight
        visible: cards.hasCaptions
        verticalAlignment: Text.AlignTop
        topPadding: Theme.dp(18)
        wrapMode: Text.WordWrap
        maximumLineCount: 2
        text: cards.caption
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(cards.compact ? 19 : 21)
        elide: Text.ElideRight
        opacity: cards.cursorShown && text !== "" ? 1.0 : 0.0

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }
    }

    Flickable {
        id: view

        anchors.fill: parent
        anchors.bottomMargin: cards.captionHeight
        contentWidth: width
        contentHeight: cards.layout.height
        interactive: false
        clip: true
        opacity: cards.dimmed ? 0.55 : 1.0

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }

        function scrollToCurrent() {
            var s = cards.stopOf(cards.index);
            if (s && height > 0)
                Theme.reveal(view, s.top, s.bottom, height);
        }

        Behavior on contentY {
            id: slideEase
            Ease {
                duration: Theme.durView
                easing.type: Easing.OutQuint
            }
        }

        Wheel {
            ease: slideEase
        }

        Repeater {
            model: cards.layout.cards

            SettingsCard {
                x: cards.columnX(modelData.col)
                y: modelData.y
                width: modelData.wide ? cards.width : cards.columnWidth
                group: cards.groups[modelData.group]
            }
        }
    }

    Scrollbar {
        anchors.left: parent.right
        anchors.leftMargin: Theme.dp(24)
        anchors.top: view.top
        anchors.bottom: view.bottom
        flickable: view
        dimmed: cards.dimmed
    }
}
