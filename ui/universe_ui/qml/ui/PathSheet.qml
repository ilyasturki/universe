import QtQuick
import "../core"
import "../sound"

Sheet {
    id: sheet

    property string zone: "list"
    property int chipIndex: 0
    property int index: 0

    signal accepted(string path)
    signal typeRequested(string path)
    signal dismissed()

    readonly property var browser: api.screens.paths
    readonly property bool files: browser.files
    readonly property var entries: browser.entries
    readonly property int rowCount: entries.length + 1

    readonly property var hints: zone === "chips"
        ? [ { glyph: "A", label: "Go" } ].concat(commonHints)
        : [ { glyph: "A", label: index === 0 ? "Up" : (entries[index - 1] && !entries[index - 1].dir ? "Choose" : "Open") } ].concat(commonHints)
    readonly property var commonHints: (files ? [] : [ { glyph: "X", label: "Use this folder" } ]).concat(
        [ { glyph: "Y", label: "Type a path" }, { glyph: "B", label: "Cancel" } ])

    readonly property real rowHeight: Theme.dp(58)
    readonly property int visibleRows: 8

    innerMax: Theme.dp(1100)
    contentHeight: Theme.dp(10) + pathText.height + Theme.dp(12) + chips.height + Theme.dp(12) + list.height

    function show(label, path, withFiles) {
        title = label;
        browser.open(path || "", withFiles === true);
        zone = "list";
        index = 0;
        chipIndex = 0;
        open = true;
        forceActiveFocus();
    }

    function finish(path) {
        open = false;
        focus = false;
        accepted(path);
    }

    function cancel() {
        open = false;
        focus = false;
        Sound.cancel();
        dismissed();
    }

    function activate() {
        if (zone === "chips") {
            Sound.enter();
            browser.go(browser.shortcuts[chipIndex].path);
            zone = "list";
            index = 0;
            return;
        }
        if (index === 0) {
            if (browser.up()) {
                Sound.enter();
                index = 0;
            } else {
                Sound.edge();
            }
            return;
        }
        var entry = entries[index - 1];
        if (!entry)
            return;
        Sound.enter();
        if (entry.dir) {
            browser.enter(index - 1);
            index = 0;
        } else {
            finish(entry.path);
        }
    }

    function moveChip(d) {
        var n = chipIndex + d;
        if (zone !== "chips" || n < 0 || n >= browser.shortcuts.length) {
            Sound.edge();
            return;
        }
        chipIndex = n;
        Sound.tick();
    }

    Keys.onUpPressed: {
        if (zone === "chips") {
            Sound.edge();
        } else if (index === 0) {
            if (browser.shortcuts.length > 0) {
                zone = "chips";
                Sound.tick();
            } else {
                Sound.edge();
            }
        } else {
            index--;
            Sound.tick();
        }
    }
    Keys.onDownPressed: {
        if (zone === "chips") {
            zone = "list";
            Sound.tick();
        } else if (index < rowCount - 1) {
            index++;
            Sound.tick();
        } else {
            Sound.edge();
        }
    }
    Keys.onLeftPressed: moveChip(-1)
    Keys.onRightPressed: moveChip(1)

    Keys.onPressed: function(event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            activate();
        } else if (api.keys.isCancel(event)) {
            cancel();
        } else if (api.keys.isDetails(event)) {
            if (files) {
                Sound.edge();
            } else {
                Sound.enter();
                finish(browser.path);
            }
        } else if (api.keys.isFilters(event)) {
            Sound.panel();
            open = false;
            focus = false;
            typeRequested(browser.path);
        }
    }

    Text {
        id: pathText
        anchors.top: sheet.head.bottom
        anchors.topMargin: Theme.dp(10)
        anchors.horizontalCenter: parent.horizontalCenter
        width: sheet.inner
        text: sheet.browser.display(sheet.browser.path)
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(30)
        elide: Text.ElideMiddle
    }

    Item {
        id: chips

        anchors.top: pathText.bottom
        anchors.topMargin: Theme.dp(12)
        anchors.horizontalCenter: parent.horizontalCenter
        // Room for the focus ring, which the clip would otherwise cut at the edges.
        readonly property real edge: Theme.dp(8)
        width: sheet.inner + edge * 2
        height: Theme.dp(45) + edge * 2
        clip: true

        Row {
            id: chipRow

            y: chips.edge
            spacing: Theme.dp(12)
            x: {
                var view = chips.width - chips.edge * 2;
                var item = chipRepeater.itemAt(sheet.chipIndex);
                if (!item || width <= view)
                    return chips.edge;
                var left = item.x, right = item.x + item.width;
                var shift = Math.max(0, Math.min(right - view, left));
                return chips.edge - Math.min(shift, width - view);
            }

            Behavior on x { Ease { duration: Theme.durQuick } }

            Repeater {
                id: chipRepeater
                model: sheet.browser.shortcuts

                Chip {
                    label: modelData.label
                    active: modelData.path === sheet.browser.path
                    focused: sheet.zone === "chips" && index === sheet.chipIndex
                }
            }
        }
    }

    ListView {
        id: list

        anchors.top: chips.bottom
        anchors.topMargin: Theme.dp(12)
        anchors.horizontalCenter: parent.horizontalCenter
        width: sheet.inner
        height: sheet.rowHeight * sheet.visibleRows
        model: sheet.rowCount
        currentIndex: sheet.index
        interactive: false
        clip: true
        highlightRangeMode: ListView.ApplyRange
        preferredHighlightBegin: 0
        preferredHighlightEnd: height
        highlightMoveDuration: Theme.durQuick

        delegate: Item {
            id: row

            readonly property bool up: index === 0
            readonly property var entry: up ? null : sheet.entries[index - 1]
            readonly property bool focused: sheet.zone === "list" && index === sheet.index
            readonly property color ink: focused ? Theme.onLight : Theme.text

            width: list.width
            height: sheet.rowHeight

            Rectangle {
                anchors.fill: parent
                anchors.margins: Theme.dp(2)
                radius: Theme.dp(14)
                color: row.focused ? Theme.text : "transparent"

                Behavior on color { ColorEase {} }
            }

            MenuGlyph {
                id: icon
                anchors.left: parent.left
                anchors.leftMargin: Theme.dp(20)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(24)
                height: Theme.dp(24)
                visible: !row.up && row.entry && row.entry.dir
                kind: "folder"
                tint: row.ink
            }

            Text {
                anchors.left: parent.left
                anchors.leftMargin: icon.visible ? Theme.dp(60) : Theme.dp(20)
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(20)
                anchors.verticalCenter: parent.verticalCenter
                text: row.up ? (sheet.browser.atRoot ? "/" : "‹ ..") : (row.entry ? row.entry.name : "")
                color: row.up && sheet.browser.atRoot ? (row.focused ? row.ink : Theme.textMuted) : row.ink
                font.family: Theme.sans
                font.weight: row.focused ? Font.DemiBold : Font.Medium
                font.pixelSize: Theme.dp(23)
                elide: Text.ElideMiddle
            }
        }

        Text {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: sheet.rowHeight / 2
            visible: sheet.entries.length === 0
            text: sheet.files ? "Nothing here." : "No folders here."
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(22)
        }
    }
}
