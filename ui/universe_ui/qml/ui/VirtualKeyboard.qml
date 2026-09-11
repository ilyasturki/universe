import QtQuick
import "../core"

// Keys only: the sheet around it owns the panel, the field and the padding.
Item {
    id: keyboard

    signal charEntered(string value)
    signal backspaced()
    signal cleared()
    signal done()

    property real keyHeight: Theme.dp(58)
    property real keyGap: Theme.dp(10)
    // Sheets that take a value add shift, path symbols and a done key; search keeps the bare set.
    property bool showDone: false
    property bool symbols: false
    property bool shift: false

    readonly property real keyUnit: (width - keyGap * 9) / 10
    readonly property real indent: (keyUnit + keyGap) / 2

    property int rowIndex: 1
    property int colIndex: 0

    implicitHeight: keyHeight * 5 + keyGap * 4

    function chars(s) {
        return s.split("").map(function(c) { return { label: c, value: c }; });
    }

    readonly property var rows: {
        var bottom = [];
        if (showDone)
            bottom.push({ label: "⇧", value: "", action: "shift" });
        if (symbols)
            bottom = bottom.concat(chars("-_./:"));
        bottom.push({ label: "space", value: " ", action: "space" });
        bottom.push({ label: "clear", value: "", action: "clear" });
        if (showDone)
            bottom.push({ label: "done", value: "", action: "done" });
        return [
            { indent: 0, keys: chars("1234567890") },
            { indent: 0, keys: chars("qwertyuiop") },
            { indent: 1, keys: chars("asdfghjkl") },
            { indent: 1, keys: chars("zxcvbnm").concat([{ label: "⌫", value: "", action: "backspace" }]) },
            { indent: 0, keys: bottom }
        ];
    }

    readonly property real clearWidth: keyUnit * 2 + keyGap
    readonly property real bottomFixed: clearWidth + keyGap
                                        + (showDone ? (clearWidth + keyGap) * 2 : 0)
                                        + (symbols ? (keyUnit + keyGap) * 5 : 0)

    function keyWidth(key) {
        if (key.action === "backspace")
            return width - indent - keyUnit * 7 - keyGap * 7;
        if (key.action === "clear" || key.action === "done" || key.action === "shift")
            return clearWidth;
        if (key.action === "space")
            return width - bottomFixed;
        return keyUnit;
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

                        readonly property bool selected: rowNo === keyboard.rowIndex
                                                         && index === keyboard.colIndex
                        readonly property bool latched: modelData.action === "shift" && keyboard.shift

                        width: keyboard.keyWidth(modelData)
                        height: keyboard.keyHeight
                        radius: Theme.dp(12)
                        color: selected ? Theme.text : latched ? Qt.rgba(1, 1, 1, 0.22) : Theme.surface

                        Behavior on color {
                            ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
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
