import QtQuick
import "../core"
import "../sound"
import "../../ui" as Base

FocusScope {
    id: rows

    property var model: []
    property int index: 0
    property var shell: null
    readonly property bool cursorShown: activeFocus
    readonly property var currentRow: index >= 0 && index < model.length ? model[index] : null

    signal activated(int index, var row)
    signal escapedLeft
    signal escapedDown

    readonly property real rowHeight: Theme.dp(113)
    readonly property real headingHeight: Theme.dp(96)
    readonly property real detailLine: Theme.dp(58)
    readonly property real inset: Theme.dp(24)
    readonly property real room: Theme.dp(Theme.ringRoom)

    function stops() {
        return model.map(function (r, i) {
            return i;
        }).filter(function (i) {
            return !model[i].heading;
        });
    }

    function heightOf(i) {
        var r = model[i];
        return !r ? 0 : r.heading ? headingHeight : rowHeight + (r.detail ? detailLine : 0);
    }

    function yOf(i) {
        return model.slice(0, i).reduce(function (y, r, k) {
            return y + heightOf(k);
        }, 0);
    }

    readonly property real contentHeight: yOf(model.length)

    function edit(row, apply) {
        var choices = row.choices || [];
        if (row.type === "enum" || ((row.type === "int" || row.type === "string") && choices.length > 0)) {
            var opts = choices.map(function (c) {
                return String(c);
            });
            if (row.type !== "enum")
                opts.push("Type a value…");
            var current = opts.indexOf(String(row.value));
            shell.pick({
                title: row.label,
                choices: opts,
                icons: row.icons || [],
                index: current >= 0 ? current : 0
            }, function (i) {
                if (i < 0)
                    return;
                if (i < choices.length) {
                    Sound.play("select");
                    apply(choices[i]);
                } else {
                    shell.prompt({
                        title: row.label,
                        value: row.value,
                        numeric: row.type === "int"
                    }, function (v) {
                        if (v !== null)
                            apply(v);
                    });
                }
            });
            return;
        }
        if (row.type === "path") {
            shell.browse({
                title: row.label,
                path: row.value,
                files: /(_path|_file|file|exe)$/.test(String(row.key || ""))
            }, function (path) {
                if (path !== null)
                    apply(path);
            });
            return;
        }
        shell.prompt({
            title: row.label,
            value: row.value,
            numeric: row.type === "int"
        }, function (v) {
            if (v !== null)
                apply(v);
        });
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
        if (next >= s.length) {
            rows.escapedDown();
            return;
        }
        if (next < 0) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        index = s[next];
    }

    function stepScreen(d) {
        var s = stops();
        if (s.indexOf(index) < 0) {
            reset();
            return;
        }
        var centre = yOf(index) + heightOf(index) / 2 + d * view.height, best = index, dist = Infinity;
        for (var k = 0; k < s.length; k++) {
            var dd = Math.abs(yOf(s[k]) + heightOf(s[k]) / 2 - centre);
            if (dd < dist) {
                best = s[k];
                dist = dd;
            }
        }
        if (best === index)
            best = s[d < 0 ? 0 : s.length - 1];
        Sound.play(best === index ? "edge" : "tick");
        index = best;
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
        Sound.play("tick");
        rows.escapedLeft();
    }
    Keys.onRightPressed: Sound.play("edge")

    Keys.onPressed: function (event) {
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (screen) {
            event.accepted = true;
            stepScreen(screen);
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (currentRow && !currentRow.heading && currentRow.type !== "info" && currentRow.type !== "static" && !currentRow.disabled)
                rows.activated(index, currentRow);
            else
                Sound.play("edge");
        }
    }

    Label {
        anchors.centerIn: parent
        visible: rows.model.length === 0
        text: "Nothing here yet."
        color: Theme.textMuted
    }

    Flickable {
        id: view

        anchors.fill: parent
        anchors.margins: -rows.room
        contentWidth: width
        contentHeight: rows.contentHeight + rows.room * 2 + Theme.dp(40)
        interactive: false
        clip: true

        function scrollToCurrent() {
            if (height <= 0)
                return;
            var top = rows.yOf(rows.index), bottom = top + rows.heightOf(rows.index) + rows.room * 2;
            if (rows.index > 0 && rows.model[rows.index - 1] && rows.model[rows.index - 1].heading)
                top -= rows.headingHeight;
            Theme.reveal(view, top, bottom, height);
        }

        Behavior on contentY {
            Ease {}
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
                readonly property bool keepsMark: hasMark || entry.iconSlot === true
                // A logo for the value (the runner picked), drawn just before it.
                readonly property bool hasValueMark: entry.valueIcon !== undefined && entry.valueIcon !== null && String(entry.valueIcon) !== "" && valueMark.status === Image.Ready
                // Dim reads as disabled but still opens: a runner whose program was not found.
                readonly property color ink: disabled || entry.dim === true ? Theme.textDisabled : Theme.text
                // A search hit: where the row lives, muted, in front of its label.
                readonly property string path: entry.path !== undefined && entry.path !== null ? String(entry.path) : ""
                // Where an inheritable value comes from when something sets it: this game or runner, or the global settings; the default reads plain.
                readonly property string originTag: entry.origin === "game" ? "THIS GAME" : entry.origin === "runner" ? "THIS RUNNER" : entry.origin === "global" ? "GLOBAL" : ""

                function esc(text) {
                    return String(text).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
                }

                x: rows.room
                y: rows.room + rows.yOf(index)
                width: view.width - rows.room * 2
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

                    Label {
                        x: Theme.dp(20)
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: Theme.dp(8)
                        text: row.entry.label || ""
                    }

                    Label {
                        anchors.right: parent.right
                        anchors.rightMargin: rows.inset
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: Theme.dp(8)
                        text: row.entry.display || ""
                        color: Theme.textSecondary
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }

                    Hairline {
                        color: Theme.hairline
                    }
                }

                Item {
                    visible: !row.heading
                    anchors.fill: parent

                    FocusPill {
                        width: parent.width
                        height: rows.rowHeight
                        focused: row.focused
                    }

                    Hairline {
                        visible: !row.focused
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
                            sourceComponent: Base.PadGlyph {
                                family: String(row.entry.family)
                                slot: String(row.entry.slot)
                                unit: Theme.dp(44)
                                ink: row.ink
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
                            source: !row.iconIsFile ? "" : String(row.entry.icon).indexOf("://") >= 0 ? row.entry.icon : Qt.resolvedUrl("../../" + row.entry.icon)
                            asynchronous: true
                            fillMode: Image.PreserveAspectFit
                            sourceSize.height: 128
                            smooth: true
                            mipmap: true
                            visible: row.hasMark
                            opacity: row.disabled || row.entry.dim === true ? 0.4 : 1.0
                        }
                    }

                    Label {
                        id: label
                        x: rows.inset + (lead.visible ? lead.width + Theme.dp(8) : 0) + (swatch.visible ? swatch.width + Theme.dp(30) : 0)
                        height: rows.rowHeight
                        width: (row.own ? ownTag.x : control.x) - x - Theme.dp(24)
                        verticalAlignment: Text.AlignVCenter
                        text: row.path !== "" ? "<font color=\"" + Theme.textSecondary + "\">" + row.esc(row.path) + " › </font>" + row.esc(row.entry.label || "") : row.entry.label || ""
                        textFormat: row.path !== "" ? Text.StyledText : Text.PlainText
                        color: row.ink
                        elide: Text.ElideRight
                    }

                    Label {
                        id: ownTag
                        anchors.right: control.left
                        anchors.rightMargin: Theme.dp(22)
                        anchors.verticalCenter: control.verticalCenter
                        visible: row.originTag !== ""
                        text: row.originTag
                        color: Theme.textSecondary
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                        font.letterSpacing: Theme.dp(2)
                    }

                    Item {
                        id: control
                        x: parent.width - width - rows.inset
                        height: rows.rowHeight
                        width: toggle.visible ? toggle.width : radio.visible ? radio.width : valueText.visible ? valueText.width + (row.hasValueMark ? valueMark.width + Theme.dp(16) : 0) : check.visible ? check.width : 0

                        Image {
                            id: valueMark
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(44)
                            height: width
                            source: row.entry.valueIcon !== undefined && row.entry.valueIcon !== null && String(row.entry.valueIcon) !== "" ? Qt.resolvedUrl("../../" + row.entry.valueIcon) : ""
                            asynchronous: true
                            fillMode: Image.PreserveAspectFit
                            sourceSize.height: 128
                            smooth: true
                            mipmap: true
                            visible: row.hasValueMark && valueText.visible
                        }

                        Toggle {
                            id: toggle
                            visible: row.entry.type === "bool" || (row.entry.switch === true && !row.entry.warning)
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

                        Label {
                            id: valueText
                            visible: row.entry.type !== "bool" && row.entry.type !== "radio" && !row.info && !toggle.visible
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.entry.display || ""
                            color: row.disabled ? Theme.textDisabled : row.entry.inherited === true || row.entry.type === "static" ? Theme.textSecondary : Theme.accent
                            elide: Text.ElideMiddle
                            width: Math.min(implicitWidth, row.width * 0.5)
                        }

                        Row {
                            id: check
                            visible: row.info
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: Theme.dp(18)

                            Label {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: text !== ""
                                text: row.entry.display || ""
                                color: Theme.textSecondary
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

                    Label {
                        visible: row.hasDetail
                        x: rows.inset
                        y: rows.rowHeight + Theme.dp(16)
                        width: parent.width - rows.inset * 2
                        height: rows.detailLine - Theme.dp(16)
                        verticalAlignment: Text.AlignTop
                        text: row.entry.detail || ""
                        color: Theme.textSecondary
                        elide: Text.ElideRight
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
