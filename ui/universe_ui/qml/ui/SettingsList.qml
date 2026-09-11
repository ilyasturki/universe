import QtQuick
import "../core"
import "../sound"

// A column of setting rows, driven by the generic row shape the host builds:
// { section, key, label, type, value, display, choices, detail, module }.
// The list moves the cursor and reports A; the page decides what a type opens.
FocusScope {
    id: list

    property var rows: []
    property int index: 0
    property real rowHeight: Theme.dp(74)
    property real sectionHeight: Theme.dp(56)
    property bool dimmed: false

    signal activated(int index, var row)
    signal escapedUp()

    readonly property var currentRow: index >= 0 && index < rows.length ? rows[index] : null
    readonly property int count: rows.length

    function isStop(row) {
        return row && (row.type === "header" || row.type === "info");
    }

    function firstStop() {
        for (var i = 0; i < rows.length; i++)
            if (!isStop(rows[i]))
                return i;
        return 0;
    }

    function step(d) {
        var next = index + d;
        while (next >= 0 && next < rows.length && isStop(rows[next]))
            next += d;
        if (next < 0 || next >= rows.length) {
            if (d < 0)
                list.escapedUp();
            else
                Sound.edge();
            return;
        }
        index = next;
        Sound.tick();
    }

    // currentRow's binding is not refreshed yet when this runs; read the row directly.
    onRowsChanged: {
        if (index >= rows.length || isStop(rows[index]))
            index = Math.min(firstStop(), Math.max(0, rows.length - 1));
    }

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)

    Keys.onPressed: {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (currentRow && !isStop(currentRow)) {
                list.activated(index, currentRow);
            } else {
                Sound.edge();
            }
            return;
        }
    }

    Text {
        anchors.centerIn: parent
        visible: list.rows.length === 0
        text: "Nothing here yet."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    ListView {
        id: view

        anchors.fill: parent
        model: list.rows
        currentIndex: list.index
        interactive: false
        clip: true
        highlightFollowsCurrentItem: false
        spacing: Theme.dp(4)
        opacity: list.dimmed ? 0.55 : 1.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        function scrollToCurrent() {
            if (!currentItem || height <= 0)
                return;
            var top = currentItem.y;
            var bottom = top + currentItem.height;
            var target = contentY;
            if (top - list.sectionHeight < contentY)
                target = Math.max(0, top - list.sectionHeight);
            else if (bottom > contentY + height)
                target = bottom - height;
            contentY = Math.max(0, Math.min(target, Math.max(0, contentHeight - height)));
        }

        onCurrentIndexChanged: scrollToCurrent()
        onCountChanged: scrollToCurrent()

        Behavior on contentY {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutQuint }
        }

        delegate: Item {
            id: row

            readonly property var entry: modelData
            readonly property bool focused: index === list.index && list.activeFocus
            readonly property bool header: entry.type === "header"
            readonly property bool info: entry.type === "info"
            readonly property bool sectionStart: !header && (index === 0 || list.rows[index - 1].section !== entry.section)

            width: view.width
            height: (header ? list.sectionHeight : list.rowHeight) + (sectionStart ? list.sectionHeight : 0)

            CapsLabel {
                anchors.left: parent.left
                anchors.leftMargin: Theme.dp(22)
                anchors.top: parent.top
                anchors.topMargin: Theme.dp(22)
                visible: row.sectionStart
                text: row.entry.section ? row.entry.section.toUpperCase() : ""
                tracking: 0.11
            }

            // A module's own row: its name, and what kind of module it is.
            Item {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.leftMargin: Theme.dp(22)
                height: list.sectionHeight
                visible: row.header

                Text {
                    id: headerLabel
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: Theme.dp(6)
                    text: row.entry.label
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Bold
                    font.pixelSize: Theme.dp(30)
                }

                Text {
                    anchors.left: headerLabel.right
                    anchors.leftMargin: Theme.dp(18)
                    anchors.baseline: headerLabel.baseline
                    text: row.entry.detail || ""
                    color: Theme.textMuted
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(20)
                }
            }

            Rectangle {
                id: body

                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: list.rowHeight
                radius: Theme.dp(16)
                visible: !row.header
                color: row.focused ? Theme.text : (row.info ? "transparent" : Theme.surface)

                Behavior on color {
                    ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                }

                Text {
                    id: label
                    anchors.left: parent.left
                    anchors.leftMargin: Theme.dp(24)
                    anchors.right: control.left
                    anchors.rightMargin: Theme.dp(20)
                    anchors.verticalCenter: parent.verticalCenter
                    text: row.entry.label
                    color: row.focused ? Theme.onLight : Theme.text
                    font.family: Theme.sans
                    font.weight: row.focused ? Font.DemiBold : Font.Medium
                    font.pixelSize: Theme.dp(24)
                    elide: Text.ElideRight
                }

                Item {
                    id: control

                    anchors.right: parent.right
                    anchors.rightMargin: Theme.dp(22)
                    anchors.verticalCenter: parent.verticalCenter
                    width: childrenRect.width
                    height: parent.height

                    // bool: a switch
                    Rectangle {
                        visible: row.entry.type === "bool"
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(64)
                        height: Theme.dp(34)
                        radius: height / 2
                        color: row.entry.value
                               ? (row.focused ? Theme.onLight : Theme.text)
                               : (row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.25) : Qt.rgba(1, 1, 1, 0.18))

                        Behavior on color {
                            ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                        }

                        Rectangle {
                            width: parent.height - Theme.dp(8)
                            height: width
                            radius: width / 2
                            anchors.verticalCenter: parent.verticalCenter
                            x: row.entry.value ? parent.width - width - Theme.dp(4) : Theme.dp(4)
                            color: row.entry.value ? (row.focused ? Theme.text : Theme.onLight) : Theme.text

                            Behavior on x {
                                NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                            }
                        }
                    }

                    // enum, string, path, int, action: the value and a chevron
                    Row {
                        visible: row.entry.type !== "bool" && !row.info
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: Theme.dp(16)

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.entry.display || ""
                            color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textSecondary
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(22)
                            elide: Text.ElideMiddle
                            width: Math.min(implicitWidth, Theme.dp(560))
                        }

                        Canvas {
                            width: Theme.dp(14)
                            height: Theme.dp(22)
                            anchors.verticalCenter: parent.verticalCenter
                            readonly property color tint: row.focused ? Theme.onLight : Theme.textMuted
                            onTintChanged: requestPaint()
                            onPaint: {
                                var ctx = getContext("2d");
                                ctx.reset();
                                ctx.strokeStyle = tint;
                                ctx.lineWidth = Theme.dp(2.5);
                                ctx.lineCap = "round";
                                ctx.lineJoin = "round";
                                ctx.beginPath();
                                ctx.moveTo(width * 0.2, height * 0.15);
                                ctx.lineTo(width * 0.8, height * 0.5);
                                ctx.lineTo(width * 0.2, height * 0.85);
                                ctx.stroke();
                            }
                        }
                    }

                    // info: a status dot and the detail
                    Row {
                        visible: row.info
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: Theme.dp(14)

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: row.entry.detail || ""
                            color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textMuted
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(21)
                            elide: Text.ElideMiddle
                            width: Math.min(implicitWidth, Theme.dp(700))
                        }

                        Rectangle {
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(14)
                            height: width
                            radius: width / 2
                            color: row.entry.value ? "#5fd48a" : "#e0655a"
                        }
                    }
                }
            }
        }
    }
}
