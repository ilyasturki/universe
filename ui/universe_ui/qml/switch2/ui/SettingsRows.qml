import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: rows

    property var model: []
    property int index: 0
    property var shell: null
    property var folder: null
    readonly property bool cursorShown: activeFocus
    readonly property var currentRow: index >= 0 && index < model.length ? model[index] : null

    signal activated(int index, var row)
    signal escapedLeft()
    signal escapedUp()

    readonly property real rowHeight: Theme.dp(113)
    readonly property real headingHeight: Theme.dp(96)
    readonly property real detailLine: Theme.dp(58)
    readonly property real inset: Theme.dp(24)

    function stops() {
        var out = [];
        for (var i = 0; i < model.length; i++)
            if (!model[i].heading)
                out.push(i);
        return out;
    }

    function heightOf(i) {
        var r = model[i];
        return !r ? 0 : r.heading ? headingHeight : rowHeight + (r.detail ? detailLine : 0);
    }

    function yOf(i) {
        var y = 0;
        for (var k = 0; k < i && k < model.length; k++)
            y += heightOf(k);
        return y;
    }

    readonly property real contentHeight: yOf(model.length)

    function edit(row, apply) {
        var choices = row.choices || [];
        if (row.type === "enum" || ((row.type === "int" || row.type === "string") && choices.length > 0)) {
            var opts = choices.map(function(c) { return String(c); });
            if (row.type !== "enum")
                opts.push("Type a value…");
            var current = opts.indexOf(String(row.value));
            shell.pick({ title: row.label, choices: opts, index: current >= 0 ? current : 0 }, function(i) {
                if (i < 0)
                    return;
                if (i < choices.length) {
                    Sound.select();
                    apply(choices[i]);
                } else {
                    shell.prompt({ title: row.label, value: row.value, numeric: row.type === "int" }, function(v) { if (v !== null) apply(v); });
                }
            });
            return;
        }
        if (row.type === "path") {
            folder.show({ title: row.label, path: row.value, files: /(_path|_file|file)$/.test(String(row.key || "")) }, function(path) {
                if (path !== null)
                    apply(path);
                rows.forceActiveFocus();
            });
            return;
        }
        shell.prompt({ title: row.label, value: row.value, numeric: row.type === "int" }, function(v) { if (v !== null) apply(v); });
    }

    function reset() {
        var s = stops();
        index = s.length > 0 ? s[0] : 0;
    }

    function step(d) {
        var s = stops();
        var pos = s.indexOf(index);
        if (pos < 0) {
            reset();
            return;
        }
        var next = pos + d;
        if (next < 0) {
            rows.escapedUp();
            return;
        }
        if (next >= s.length) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = s[next];
    }

    onModelChanged: {
        if (!currentRow || currentRow.heading)
            reset();
        view.scrollToCurrent();
    }
    onIndexChanged: view.scrollToCurrent()
    onHeightChanged: view.scrollToCurrent()

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)
    Keys.onLeftPressed: {
        Sound.tick();
        rows.escapedLeft();
    }
    Keys.onRightPressed: Sound.edge()

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (currentRow && !currentRow.heading && currentRow.type !== "info" && currentRow.type !== "static" && !currentRow.disabled)
                rows.activated(index, currentRow);
            else
                Sound.edge();
        }
    }

    Text {
        anchors.centerIn: parent
        visible: rows.model.length === 0
        text: "Nothing here yet."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(Theme.fontBody)
    }

    Flickable {
        id: view

        anchors.fill: parent
        contentWidth: width
        contentHeight: rows.contentHeight + Theme.dp(40)
        interactive: false
        clip: true

        function scrollToCurrent() {
            if (height <= 0)
                return;
            var top = rows.yOf(rows.index), bottom = top + rows.heightOf(rows.index);
            if (rows.index > 0 && rows.model[rows.index - 1] && rows.model[rows.index - 1].heading)
                top -= rows.headingHeight;
            Theme.reveal(view, top, bottom, height);
        }

        Behavior on contentY {
            NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic }
        }

        Repeater {
            model: rows.model

            Item {
                id: row

                readonly property var entry: modelData
                readonly property bool heading: entry.heading === true
                readonly property bool focused: rows.cursorShown && index === rows.index
                readonly property bool disabled: entry.disabled === true
                readonly property bool info: entry.type === "info"
                readonly property bool hasDetail: entry.detail !== undefined && entry.detail !== ""
                readonly property bool hasGlyph: entry.slot !== undefined && String(entry.slot) !== "" && entry.family !== undefined
                // An icon naming a file (a runner's logo) is drawn whole; a bare name is a Glyph kind.
                readonly property bool iconIsFile: entry.icon !== undefined && entry.icon !== null && String(entry.icon).indexOf("/") >= 0
                readonly property bool hasMark: iconIsFile && mark.status === Image.Ready
                readonly property bool hasIcon: !hasGlyph && !iconIsFile && entry.icon !== undefined && String(entry.icon) !== ""
                // A list where most rows carry a mark keeps the label aligned on the ones without.
                readonly property bool keepsMark: hasMark || entry.iconSlot === true
                // Dim reads as disabled but still opens: a runner whose program was not found.
                readonly property color ink: disabled || entry.dim === true ? Theme.textDisabled : Theme.text

                y: rows.yOf(index)
                width: view.width
                height: rows.heightOf(index)

                Item {
                    visible: row.heading
                    anchors.fill: parent

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: Theme.dp(8)
                        width: Theme.dp(5)
                        height: Theme.dp(34)
                        color: Theme.text
                    }

                    Text {
                        x: Theme.dp(20)
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: Theme.dp(8)
                        text: row.entry.label || ""
                        color: Theme.text
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontBody)
                    }

                    Text {
                        anchors.right: parent.right
                        anchors.rightMargin: rows.inset
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: Theme.dp(8)
                        text: row.entry.display || ""
                        color: Theme.textSecondary
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }

                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.left: parent.left
                        anchors.right: parent.right
                        height: 1
                        color: Theme.hairline
                    }
                }

                Item {
                    visible: !row.heading
                    anchors.fill: parent

                    Rectangle {
                        id: pill
                        width: parent.width
                        height: rows.rowHeight
                        radius: Theme.dp(Theme.radiusRow)
                        color: Theme.focusFill
                        visible: row.focused
                    }

                    FocusOutline {
                        target: pill
                        cornerRadius: pill.radius
                        gap: 0
                        shown: row.focused
                    }

                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.left: parent.left
                        anchors.right: parent.right
                        height: 1
                        visible: !row.focused
                        color: Theme.hairlineSoft
                    }

                    Rectangle {
                        id: swatch
                        x: rows.inset
                        y: (rows.rowHeight - height) / 2
                        width: visible ? Theme.dp(98) : 0
                        height: Theme.dp(66)
                        visible: row.entry.swatch !== undefined && row.entry.swatch !== ""
                        color: visible ? row.entry.swatch : "transparent"
                        border.width: 1
                        border.color: Theme.hairline
                    }

                    Item {
                        id: lead
                        x: rows.inset
                        height: rows.rowHeight
                        width: row.hasGlyph ? Theme.dp(64) : row.hasIcon || row.keepsMark ? Theme.dp(52) : 0
                        visible: row.hasGlyph || row.hasIcon || row.keepsMark

                        Loader {
                            anchors.verticalCenter: parent.verticalCenter
                            active: row.hasGlyph
                            source: "../../ui/PadGlyph.qml"
                            onLoaded: {
                                item.family = Qt.binding(function() { return String(row.entry.family); });
                                item.slot = Qt.binding(function() { return String(row.entry.slot); });
                                item.unit = Qt.binding(function() { return Theme.dp(44); });
                                item.ink = Qt.binding(function() { return row.ink; });
                            }
                        }

                        Glyph {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: row.hasIcon
                            width: Theme.dp(40)
                            height: width
                            kind: row.hasIcon ? String(row.entry.icon) : ""
                            tint: row.ink
                        }

                        Image {
                            id: mark
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(44)
                            height: width
                            source: row.iconIsFile ? Qt.resolvedUrl("../../" + row.entry.icon) : ""
                            asynchronous: true
                            fillMode: Image.PreserveAspectFit
                            sourceSize.height: 128
                            smooth: true
                            mipmap: true
                            visible: row.hasMark
                            opacity: row.disabled || row.entry.dim === true ? 0.4 : 1.0
                        }
                    }

                    Text {
                        id: label
                        x: rows.inset + (lead.visible ? lead.width + Theme.dp(8) : 0) + (swatch.visible ? swatch.width + Theme.dp(30) : 0)
                        height: rows.rowHeight
                        width: control.x - x - Theme.dp(24)
                        verticalAlignment: Text.AlignVCenter
                        text: row.entry.label || ""
                        color: row.ink
                        elide: Text.ElideRight
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontBody)
                    }

                    Item {
                        id: control
                        x: parent.width - width - rows.inset
                        height: rows.rowHeight
                        width: toggle.visible ? toggle.width : radio.visible ? radio.width : valueText.visible ? valueText.width : check.visible ? check.width : 0

                        Toggle {
                            id: toggle
                            visible: row.entry.type === "bool"
                            anchors.verticalCenter: parent.verticalCenter
                            on: row.entry.value === true
                            opacity: row.disabled ? 0.4 : 1.0
                        }

                        Rectangle {
                            id: radio
                            visible: row.entry.type === "radio"
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(44)
                            height: width
                            radius: width / 2
                            color: row.entry.value === true ? Theme.accentStrong : "transparent"
                            border.width: Theme.dp(2)
                            border.color: row.entry.value === true ? Theme.accentStrong : Theme.hairline

                            Rectangle {
                                anchors.centerIn: parent
                                width: Theme.dp(16)
                                height: width
                                radius: width / 2
                                color: "#ffffff"
                                visible: row.entry.value === true
                            }
                        }

                        Text {
                            id: valueText
                            visible: row.entry.type !== "bool" && row.entry.type !== "radio" && !row.info
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.entry.display || ""
                            color: row.disabled ? Theme.textDisabled : row.entry.inherited === true || row.entry.type === "static" ? Theme.textSecondary : Theme.accent
                            elide: Text.ElideMiddle
                            width: Math.min(implicitWidth, row.width * 0.5)
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(Theme.fontBody)
                        }

                        Row {
                            id: check
                            visible: row.info
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: Theme.dp(18)

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: text !== ""
                                text: row.entry.display || ""
                                color: Theme.textSecondary
                                font.family: Theme.sans
                                font.pixelSize: Theme.dp(Theme.fontSmall)
                            }

                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: Theme.dp(36)
                                height: width
                                radius: width / 2
                                color: row.entry.value ? Theme.okGreen : Theme.danger

                                Glyph {
                                    anchors.fill: parent
                                    anchors.margins: Theme.dp(8)
                                    kind: row.entry.value === true ? "check" : "cross"
                                    tint: "#ffffff"
                                    stroke: 4.3
                                }
                            }
                        }
                    }

                    Text {
                        visible: row.hasDetail
                        x: rows.inset
                        y: rows.rowHeight + Theme.dp(16)
                        width: parent.width - rows.inset * 2
                        height: rows.detailLine - Theme.dp(16)
                        verticalAlignment: Text.AlignTop
                        text: row.entry.detail || ""
                        color: Theme.textSecondary
                        elide: Text.ElideRight
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }
                }
            }
        }
    }

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: -Theme.dp(40)
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        flickable: view
    }
}
