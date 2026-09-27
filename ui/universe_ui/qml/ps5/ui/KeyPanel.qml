import QtQuick
import "../core"
import "../sound"

// The console's on-screen keyboard: eleven columns of keys, a row of wide keys under them. Emits what it types; the owner keeps the text.
Rectangle {
    id: panel

    property bool numeric: false
    property bool symbols: false
    property bool shift: false
    property bool built: false
    property bool cursorShown: true

    property int rowIndex: 1
    property int colIndex: 0

    signal typed(string value)
    signal backspaced
    signal accepted
    // Up from the top row: the owner may move on to what sits above the panel.
    signal escapedUp

    readonly property int columns: 11
    readonly property real unitW: Theme.dp(79)
    readonly property real keyH: Theme.dp(64)

    width: unitW * columns
    height: keyH * rows.length + Theme.dp(8)
    radius: Theme.dp(6)
    color: Qt.rgba(0.08, 0.09, 0.12, 0.96)
    border.width: 1
    border.color: Theme.glassEdge

    function reset() {
        rowIndex = numeric ? 0 : 1;
        colIndex = 0;
        shift = false;
    }

    function chars(s, span) {
        return s.split("").map(function (c) {
            return {
                value: c,
                span: span || 1
            };
        });
    }

    // The physical keyboard's rows (api.keys.layout), eleven keys each; a key's `shift` is what ⇧ types on it.
    function keys(row) {
        return (api.keys.layout.rows[row] || []).slice(0, 11).map(function (k) {
            return Object.assign({
                span: 1
            }, k);
        });
    }

    readonly property var letterRows: [keys("AE"), keys("AD"), keys("AC"), keys("AB")]
    readonly property var symbolRows: [chars("~`!@#$%^&*("), chars(")_+={}[]|\\;"), chars("\"<>/?,.-:'"), chars("¿¡€£¥•…—–«»")]
    readonly property var numericRows: [chars("123", 11 / 3), chars("456", 11 / 3), chars("789", 11 / 3), chars("-0.", 11 / 3)]
    readonly property var bottomRow: numeric ? [
        {
            action: "backspace",
            label: "⌫",
            span: 11 / 3
        },
        {
            action: "ok",
            label: "Done",
            span: 22 / 3
        }
    ] : [
        {
            action: "shift",
            label: "⇧",
            span: 2
        },
        {
            action: "symbols",
            label: symbols ? "ABC" : "@#:",
            span: 2
        },
        {
            action: "space",
            label: "Space",
            value: " ",
            span: 3
        },
        {
            action: "backspace",
            label: "⌫",
            span: 2
        },
        {
            action: "ok",
            label: "Done",
            span: 2
        }
    ]
    readonly property var rows: (numeric ? numericRows : symbols ? symbolRows : letterRows).concat([bottomRow])

    function keyAt(r, c) {
        var row = rows[r];
        return row ? row[c] : undefined;
    }

    function startOf(r, c) {
        var u = 0;
        for (var k = 0; k < c; k++)
            u += rows[r][k].span;
        return u;
    }

    // What a key types: its shift level while ⇧ is on, for a key that has one.
    function valueOf(key) {
        return shift && key.shift !== undefined ? key.shift : key.value;
    }

    function put(ch) {
        Sound.play("type");
        panel.typed(ch);
        shift = false;
    }

    function press() {
        var key = keyAt(rowIndex, colIndex);
        if (!key)
            return;
        if (key.action === "ok") {
            Sound.play("ok");
            panel.accepted();
        } else if (key.action === "backspace") {
            Sound.play("type");
            panel.backspaced();
        } else if (key.value !== undefined) {
            put(valueOf(key));
        } else if (key.action === "shift") {
            Sound.play("type");
            shift = !shift;
        } else if (key.action === "symbols") {
            Sound.play("type");
            symbols = !symbols;
        }
    }

    function move(dr, dc) {
        if (dr !== 0) {
            var nr = rowIndex + dr;
            if (nr < 0) {
                panel.escapedUp();
                return;
            }
            if (nr >= rows.length) {
                Sound.play("edge");
                return;
            }
            var centre = startOf(rowIndex, colIndex) + rows[rowIndex][colIndex].span / 2;
            var u = 0, pick = rows[nr].length - 1;
            for (var k = 0; k < rows[nr].length; k++) {
                if (centre < u + rows[nr][k].span) {
                    pick = k;
                    break;
                }
                u += rows[nr][k].span;
            }
            rowIndex = nr;
            colIndex = pick;
            Sound.play("tick");
            return;
        }
        colIndex = Sound.stepped(colIndex, dc, rows[rowIndex].length);
    }

    Repeater {
        model: panel.built ? panel.rows.length : 0

        Item {
            id: line

            readonly property int r: index
            readonly property bool lastRow: r === panel.rows.length - 1

            y: Theme.dp(4) + r * panel.keyH
            width: panel.width
            height: panel.keyH

            Rectangle {
                anchors.fill: parent
                visible: line.lastRow
                color: Qt.rgba(1, 1, 1, 0.05)
            }

            Repeater {
                model: panel.rows[line.r].length

                Item {
                    id: key

                    readonly property var spec: panel.keyAt(line.r, index)
                    readonly property bool focused: panel.cursorShown && panel.rowIndex === line.r && panel.colIndex === index
                    readonly property bool latched: (spec.action === "shift" && panel.shift) || (spec.action === "symbols" && panel.symbols)

                    x: panel.startOf(line.r, index) * panel.unitW
                    width: spec.span * panel.unitW
                    height: panel.keyH

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: Theme.dp(5)
                        radius: Theme.dp(3)
                        color: key.focused ? Qt.rgba(1, 1, 1, 0.14) : "transparent"
                        border.width: key.focused ? Theme.dp(2) : 0
                        border.color: Theme.ring
                    }

                    Label {
                        anchors.centerIn: parent
                        text: key.spec.label !== undefined ? key.spec.label : panel.valueOf(key.spec)
                        color: key.latched ? Theme.accent : Theme.text
                        font.weight: key.spec.action === "ok" || key.focused ? Font.DemiBold : Font.Normal
                        font.pixelSize: Theme.dp(key.spec.label !== undefined && key.spec.label.length > 1 ? 26 : 32)
                    }

                    Rectangle {
                        visible: key.latched
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: Theme.dp(9)
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: parent.width * 0.5
                        height: Theme.dp(3)
                        color: Theme.accent
                    }

                    Touch {
                        direct: true
                        onPicked: {
                            panel.rowIndex = line.r;
                            panel.colIndex = index;
                        }
                    }
                }
            }
        }
    }
}
