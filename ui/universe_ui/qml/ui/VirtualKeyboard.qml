import QtQuick
import "../core"

Item {
    id: keyboard

    signal charEntered(string value)
    signal backspaced
    signal cleared
    signal done
    // The mouse landed on a key (it is the selected one by then).
    signal pointed

    property real keyHeight: Theme.dp(58)
    property real keyGap: Theme.dp(10)
    property bool showDone: false
    property bool symbols: false
    property bool shift: false
    property bool numeric: false

    readonly property real keyUnit: (width - keyGap * 9) / 10
    readonly property real indent: (keyUnit + keyGap) / 2

    property int rowIndex: 1
    property int colIndex: 0

    implicitHeight: keyHeight * 5 + keyGap * 4

    function chars(s) {
        return s.split("").map(function (c) {
            return {
                label: c,
                value: c
            };
        });
    }

    function wide(key) {
        key.wide = true;
        return key;
    }

    readonly property var rows: {
        if (numeric)
            return [
                {
                    indent: 4,
                    keys: chars("123").map(wide)
                },
                {
                    indent: 4,
                    keys: chars("456").map(wide)
                },
                {
                    indent: 4,
                    keys: chars("789").map(wide)
                },
                {
                    indent: 4,
                    keys: [wide({
                            label: "-",
                            value: "-"
                        }), wide({
                            label: "0",
                            value: "0"
                        }), wide({
                            label: "⌫",
                            value: "",
                            action: "backspace"
                        })]
                },
                {
                    indent: 6,
                    keys: [
                        {
                            label: "clear",
                            value: "",
                            action: "clear"
                        },
                        {
                            label: "done",
                            value: "",
                            action: "done"
                        }
                    ]
                }
            ];
        var bottom = [];
        if (showDone)
            bottom.push({
                label: "⇧",
                value: "",
                action: "shift"
            });
        if (symbols)
            bottom = bottom.concat(chars("-_./:"));
        bottom.push({
            label: "space",
            value: " ",
            action: "space"
        });
        bottom.push({
            label: "clear",
            value: "",
            action: "clear"
        });
        if (showDone)
            bottom.push({
                label: "done",
                value: "",
                action: "done"
            });
        return [
            {
                indent: 0,
                keys: chars("1234567890")
            },
            {
                indent: 0,
                keys: chars("qwertyuiop")
            },
            {
                indent: 1,
                keys: chars("asdfghjkl")
            },
            {
                indent: 1,
                keys: chars("zxcvbnm").concat([
                    {
                        label: "⌫",
                        value: "",
                        action: "backspace"
                    }
                ])
            },
            {
                indent: 0,
                keys: bottom
            }
        ];
    }

    // With the path symbols in, the bottom row only fits with single-width shift, clear and done.
    readonly property real clearWidth: symbols ? keyUnit : keyUnit * 2 + keyGap
    readonly property real bottomFixed: rows[rows.length - 1].keys.reduce(function (w, k, i) {
        return w + (k.action === "space" ? 0 : keyWidth(k)) + (i > 0 ? keyGap : 0);
    }, 0)

    function keyWidth(key) {
        if (key.wide)
            return keyUnit * 2 + keyGap;
        if (key.action === "backspace")
            return width - indent - keyUnit * 7 - keyGap * 7;
        if (key.action === "clear" || key.action === "done" || key.action === "shift")
            return clearWidth;
        if (key.action === "space")
            return width - bottomFixed;
        return keyUnit;
    }

    onNumericChanged: {
        rowIndex = numeric ? 0 : 1;
        colIndex = 0;
    }

    // A physical keystroke, as the key on the sheet it stands for: typed, backspaced, done on Enter under a keyboard, Home and End
    // the ends of the row. False when it is none.
    function typed(event) {
        if (event.key === Qt.Key_Backspace) {
            backspaced();
            return true;
        }
        if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            colIndex = api.keys.isFirst(event) ? 0 : rows[rowIndex].keys.length - 1;
            return true;
        }
        if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && api.keys.mode === "keyboard") {
            done();
            return true;
        }
        var ch = event.text;
        if (ch.length !== 1 || ch < " " || ch.charCodeAt(0) === 127 || (event.modifiers & (Qt.ControlModifier | Qt.AltModifier)))
            return false;
        if (numeric && "0123456789-".indexOf(ch) < 0)
            return false;
        charEntered(ch);
        return true;
    }

    function press() {
        var key = rows[rowIndex].keys[colIndex];
        if (!key)
            return;
        if (key.action === "backspace")
            backspaced();
        else if (key.action === "clear")
            cleared();
        else if (key.action === "done")
            done();
        else if (key.action === "shift")
            shift = !shift;
        else
            charEntered(shift && key.value.length === 1 ? key.value.toUpperCase() : key.value);
    }

    function move(dRow, dCol) {
        if (dRow !== 0) {
            var nextRow = rowIndex + dRow;
            if (nextRow < 0 || nextRow >= rows.length)
                return false;
            var ratio = colIndex / Math.max(1, rows[rowIndex].keys.length - 1);
            rowIndex = nextRow;
            colIndex = Math.round(ratio * (rows[nextRow].keys.length - 1));
            return true;
        }
        var nextCol = Math.max(0, Math.min(rows[rowIndex].keys.length - 1, colIndex + dCol));
        if (nextCol === colIndex)
            return false;
        colIndex = nextCol;
        return true;
    }

    Column {
        anchors.fill: parent
        spacing: keyboard.keyGap

        Repeater {
            model: keyboard.rows

            Row {
                readonly property int rowNo: index

                x: modelData.indent * keyboard.indent
                spacing: keyboard.keyGap

                Repeater {
                    model: modelData.keys

                    Rectangle {
                        id: key

                        readonly property bool selected: rowNo === keyboard.rowIndex && index === keyboard.colIndex
                        readonly property bool latched: modelData.action === "shift" && keyboard.shift

                        width: keyboard.keyWidth(modelData)
                        height: keyboard.keyHeight
                        radius: Theme.dp(12)
                        color: selected ? Theme.text : latched ? Qt.rgba(1, 1, 1, 0.22) : Theme.surface

                        Behavior on color {
                            ColorEase {}
                        }

                        Pointer {
                            onHovered: {
                                keyboard.rowIndex = rowNo;
                                keyboard.colIndex = index;
                                keyboard.pointed();
                            }
                        }

                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: -Theme.dp(4)
                            radius: parent.radius + Theme.dp(4)
                            visible: key.selected
                            color: "transparent"
                            border.width: Theme.dp(4)
                            border.color: "#ffffff"
                            antialiasing: true
                        }

                        Text {
                            anchors.centerIn: parent
                            visible: modelData.action !== "backspace"
                            text: modelData.label.length === 1 ? modelData.label.toUpperCase() : modelData.label
                            color: key.selected ? Theme.onLight : Theme.text
                            font.family: Theme.sans
                            font.weight: key.selected ? Font.DemiBold : Font.Medium
                            font.pixelSize: modelData.label.length > 1 ? Theme.dp(22) : Theme.dp(27)
                        }

                        Loader {
                            anchors.centerIn: parent
                            active: modelData.action === "backspace"
                            width: Theme.dp(30)
                            height: Theme.dp(24)

                            sourceComponent: Canvas {
                                readonly property color stroke: key.selected ? Theme.onLight : Theme.text
                                onStrokeChanged: requestPaint()

                                onPaint: {
                                    var ctx = getContext("2d");
                                    ctx.reset();
                                    var s = width / 30;
                                    ctx.strokeStyle = stroke;
                                    ctx.lineWidth = 2 * s;
                                    ctx.lineJoin = "round";
                                    ctx.lineCap = "round";
                                    ctx.beginPath();
                                    ctx.moveTo(10 * s, 3 * s);
                                    ctx.lineTo(27 * s, 3 * s);
                                    ctx.lineTo(27 * s, 21 * s);
                                    ctx.lineTo(10 * s, 21 * s);
                                    ctx.lineTo(2 * s, 12 * s);
                                    ctx.closePath();
                                    ctx.stroke();
                                    ctx.beginPath();
                                    ctx.moveTo(15 * s, 9 * s);
                                    ctx.lineTo(22 * s, 15 * s);
                                    ctx.moveTo(22 * s, 9 * s);
                                    ctx.lineTo(15 * s, 15 * s);
                                    ctx.stroke();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
