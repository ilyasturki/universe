import QtQuick
import "../core"
import "../sound"

// The Switch's keyboard panel: rows of keys with a side column (⌫, Done). Emits what it types; the owner keeps the text.
Item {
    id: panel

    property bool numeric: false
    property bool symbols: false
    property bool shift: false
    property bool built: false
    property bool cursorShown: true
    property real scale: 1.0
    property string doneLabel: "Done"

    property string zone: "keys"
    property int rowIndex: 1
    property int colIndex: 0
    property int sideIndex: 1

    signal typed(string value)
    signal backspaced
    signal accepted
    // Up from the top row: the owner may move on to what sits above the panel.
    signal escapedUp

    readonly property real keyGap: Theme.dp(6)
    readonly property real keysLeft: Theme.dp(134)
    readonly property real sideW: Theme.dp(170)
    // Eleven keys and the side column keep a 125dp right margin: a narrower screen narrows the keys.
    readonly property real keyW: Math.min(Theme.dp(128 * scale), width > 0 ? Math.floor((width - keysLeft - Theme.dp(12 + 125) - sideW) / 11 - keyGap) : Theme.dp(128 * scale))
    readonly property real keyH: Theme.dp(70 * scale)
    readonly property real keysTop: Theme.dp(40 * scale)
    readonly property real sideX: keysLeft + 11 * (keyW + keyGap) + Theme.dp(12)
    readonly property real radius: Theme.dp(40)

    height: keysTop + keyH * 5 + keyGap * 4 + Theme.dp(20) * scale

    function reset() {
        zone = "keys";
        rowIndex = numeric ? 0 : 1;
        colIndex = 0;
        sideIndex = 1;
        shift = false;
    }

    function chars(s) {
        return s.split("").map(function (c) {
            return {
                value: c
            };
        });
    }

    // The physical keyboard's rows (api.keys.layout), eleven keys each; a key's `shift` is what ⇧ types on it.
    function keys(row) {
        return (api.keys.layout.rows[row] || []).slice(0, 11);
    }

    readonly property var rows: numeric ? [chars("123"), chars("456"), chars("789"), chars("-0.")] : [keys("AE"), keys("AD"), keys("AC"), keys("AB")]
    readonly property var bottomRow: numeric ? [] : [
        {
            label: "⇧",
            action: "shift"
        },
        {
            label: "ABC",
            action: "abc"
        },
        {
            label: "#+=",
            action: "symbols"
        },
        {
            label: "Space",
            action: "space",
            value: " ",
            badge: "Y"
        }
    ]
    readonly property var side: [
        {
            label: "⌫",
            action: "backspace",
            badge: "B"
        },
        {
            label: panel.doneLabel,
            action: "ok",
            badge: "Start"
        }
    ]
    readonly property var symbolRows: [chars("~`!@#$%^&*("), chars(")_+={}[]|\\;"), chars("\"<>/?,.-:'"), chars("¿¡€£¥•…—–")]
    readonly property var shownRows: symbols && !numeric ? symbolRows : rows

    function keyAt(r, c) {
        return r < shownRows.length ? shownRows[r][c] : bottomRow[c];
    }

    // What a key types: its shift level while ⇧ is on, for a key that has one.
    function valueOf(key) {
        return shift && key.shift !== undefined ? key.shift : key.value;
    }

    function rowLength(r) {
        return r < shownRows.length ? shownRows[r].length : bottomRow.length;
    }
    readonly property int rowCount: shownRows.length + (bottomRow.length > 0 ? 1 : 0)

    function put(ch) {
        Sound.play("type");
        panel.typed(ch);
        shift = false;
    }

    function press() {
        if (zone === "side") {
            if (side[sideIndex].action === "backspace") {
                Sound.play("type");
                panel.backspaced();
            } else {
                Sound.play("ok");
                panel.accepted();
            }
            return;
        }
        var key = keyAt(rowIndex, colIndex);
        if (!key)
            return;
        if (key.value !== undefined) {
            put(valueOf(key));
            return;
        }
        Sound.play("type");
        if (key.action === "shift")
            shift = !shift;
        else if (key.action === "abc")
            symbols = false;
        else
            symbols = !symbols;
    }

    function move(dr, dc) {
        if (zone === "side") {
            if (dc < 0) {
                zone = "keys";
                colIndex = rowLength(rowIndex) - 1;
                Sound.play("tick");
                return;
            }
            if (dr < 0 && sideIndex === 0) {
                panel.escapedUp();
                return;
            }
            sideIndex = Sound.stepped(sideIndex, dr, side.length);
            return;
        }
        if (dr !== 0) {
            var nr = rowIndex + dr;
            if (nr < 0) {
                panel.escapedUp();
                return;
            }
            if (nr >= rowCount) {
                Sound.play("edge");
                return;
            }
            var ratio = colIndex / Math.max(1, rowLength(rowIndex) - 1);
            rowIndex = nr;
            colIndex = Math.round(ratio * (rowLength(nr) - 1));
            Sound.play("tick");
            return;
        }
        var nc = colIndex + dc;
        if (nc >= rowLength(rowIndex)) {
            zone = "side";
            // ⌫ beside the top row, Done beside the rest: Return between them takes no cursor.
            sideIndex = rowIndex === 0 ? 0 : 1;
            Sound.play("tick");
            return;
        }
        if (nc < 0) {
            Sound.play("edge");
            return;
        }
        colIndex = nc;
        Sound.play("tick");
    }

    component Badge: HintGlyph {
        unit: Theme.dp(26 * panel.scale)
    }

    Rectangle {
        width: parent.width
        // The top corners round; the bottom ones run on under the hint bar.
        height: parent.height + panel.radius
        radius: panel.radius
        color: Theme.ground
    }

    Column {
        x: panel.keysLeft
        y: panel.keysTop
        spacing: panel.keyGap

        Repeater {
            model: panel.built ? panel.rowCount : 0

            Row {
                readonly property int r: index
                spacing: panel.keyGap

                Repeater {
                    model: panel.rowLength(r)

                    Item {
                        id: key

                        readonly property var spec: panel.keyAt(r, index)
                        readonly property bool wide: spec.action === "space"
                        readonly property bool mode: spec.action === "abc" || spec.action === "symbols"
                        readonly property bool focused: panel.cursorShown && panel.zone === "keys" && panel.rowIndex === r && panel.colIndex === index
                        readonly property bool latched: (spec.action === "shift" && panel.shift) || (spec.action === "abc" && !panel.symbols && !panel.numeric) || (spec.action === "symbols" && panel.symbols)

                        width: wide ? panel.keyW * 8 + panel.keyGap * 7 : panel.keyW
                        height: panel.keyH

                        // ABC and #+= sit on one strip, a rule between them, as the console's mode switch.
                        Rectangle {
                            anchors.fill: parent
                            anchors.rightMargin: key.spec.action === "abc" ? -panel.keyGap : 0
                            radius: Theme.dp(4)
                            color: Theme.card
                            visible: key.mode
                        }

                        Rectangle {
                            visible: key.spec.action === "abc"
                            x: parent.width + panel.keyGap / 2
                            anchors.verticalCenter: parent.verticalCenter
                            width: 1
                            height: parent.height * 0.6
                            color: Theme.hairline
                        }

                        FocusPill {
                            anchors.fill: parent
                            radius: Theme.dp(4)
                            color: key.focused ? Theme.focusFill : key.mode ? "transparent" : Theme.card
                            visible: true
                            focused: key.focused
                        }

                        Row {
                            anchors.centerIn: parent
                            spacing: Theme.dp(2)

                            Canvas {
                                id: arrow

                                readonly property color ink: key.latched ? Theme.accent : Theme.text

                                visible: key.spec.action === "shift"
                                width: Theme.dp(44 * panel.scale)
                                height: width
                                onInkChanged: requestPaint()
                                onWidthChanged: requestPaint()
                                onPaint: {
                                    var ctx = getContext("2d");
                                    ctx.reset();
                                    var s = width / 24;
                                    ctx.strokeStyle = ink;
                                    ctx.fillStyle = ink;
                                    ctx.lineWidth = 1.8 * s;
                                    ctx.lineJoin = "round";
                                    ctx.beginPath();
                                    ctx.moveTo(12 * s, 2.5 * s);
                                    ctx.lineTo(22 * s, 12.5 * s);
                                    ctx.lineTo(16.5 * s, 12.5 * s);
                                    ctx.lineTo(16.5 * s, 21 * s);
                                    ctx.lineTo(7.5 * s, 21 * s);
                                    ctx.lineTo(7.5 * s, 12.5 * s);
                                    ctx.lineTo(2 * s, 12.5 * s);
                                    ctx.closePath();
                                    if (key.latched)
                                        ctx.fill();
                                    ctx.stroke();
                                }
                            }

                            Label {
                                visible: !arrow.visible
                                text: key.spec.label !== undefined ? key.spec.label : panel.valueOf(key.spec)
                                color: key.latched ? Theme.accent : Theme.text
                                font.pixelSize: Theme.dp((key.spec.label !== undefined && key.spec.label.length > 1 ? 32 : 40) * panel.scale)
                            }

                            Badge {
                                visible: key.spec.badge !== undefined
                                glyph: key.spec.badge || "Y"
                            }
                        }

                        Rectangle {
                            visible: key.latched && key.mode
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: Theme.dp(10 * panel.scale)
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: Theme.dp(64 * panel.scale)
                            height: Theme.dp(3)
                            color: Theme.accent
                        }
                    }
                }
            }
        }
    }

    Item {
        x: panel.sideX
        y: panel.keysTop
        visible: panel.built

        Repeater {
            model: panel.built ? panel.side : []

            Item {
                id: sideKey

                readonly property bool focused: panel.cursorShown && panel.zone === "side" && panel.sideIndex === index
                readonly property bool ok: modelData.action === "ok"
                readonly property int slot: ok ? (panel.numeric ? 1 : 3) : 0

                y: slot * (panel.keyH + panel.keyGap)
                width: panel.sideW
                height: ok ? panel.keyH * 2 + panel.keyGap : panel.keyH

                FocusPill {
                    anchors.fill: parent
                    radius: Theme.dp(4)
                    color: sideKey.focused ? Theme.focusFill : Theme.card
                    visible: true
                    focused: sideKey.focused
                }

                Label {
                    anchors.centerIn: parent
                    text: modelData.label
                    font.pixelSize: Theme.dp((modelData.label.length > 1 ? 34 : 40) * panel.scale)
                }

                Badge {
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: Theme.dp(8 * panel.scale)
                    glyph: modelData.badge
                }
            }
        }

        Rectangle {
            y: panel.keyH + panel.keyGap
            width: panel.sideW
            height: panel.keyH * 2 + panel.keyGap
            radius: Theme.dp(4)
            visible: !panel.numeric
            color: "#e3e3e3"

            Label {
                anchors.centerIn: parent
                text: "Return"
                color: Theme.textDisabled
                font.pixelSize: Theme.dp(34 * panel.scale)
            }
        }
    }
}
