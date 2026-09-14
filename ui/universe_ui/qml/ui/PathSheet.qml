import QtQuick
import "../core"
import "../sound"

// A folder picked at the gamepad: shortcuts across the top, the folder's entries below,
// ".." first. X takes the folder shown; a file, when files are wanted, is taken with A.
FocusScope {
    id: sheet

    property bool open: false
    property string title: ""
    // "chips" or "list"
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

    readonly property real inner: Math.min(Theme.dp(1100), width - Theme.dp(280))
    readonly property real pad: Theme.dp(28)
    readonly property real rowHeight: Theme.dp(58)
    readonly property int visibleRows: 8

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
        if (entry.dir) {
            Sound.enter();
            browser.enter(index - 1);
            index = 0;
        } else {
            Sound.enter();
            finish(entry.path);
        }
    }

    visible: scrim.opacity > 0.01
    focus: open

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
    Keys.onLeftPressed: {
        if (zone === "chips" && chipIndex > 0) {
            chipIndex--;
            Sound.tick();
        } else {
            Sound.edge();
        }
    }
    Keys.onRightPressed: {
        if (zone === "chips" && chipIndex < browser.shortcuts.length - 1) {
            chipIndex++;
            Sound.tick();
        } else {
            Sound.edge();
        }
    }

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

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Qt.rgba(0.02, 0.02, 0.03, 1)
        opacity: sheet.open ? 0.72 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }
    }

    Item {
        id: panel

        anchors.left: parent.left
        anchors.right: parent.right
        height: sheet.pad * 2 + heading.height + Theme.dp(10) + pathText.height + Theme.dp(12)
                + chips.height + Theme.dp(12) + list.height
        y: sheet.open ? parent.height - height - Theme.dp(Theme.hintBarHeight) : parent.height

        Behavior on y {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutQuint }
        }

        Rectangle {
            anchors.fill: parent
            anchors.bottomMargin: -Theme.dp(120)
            radius: Theme.dp(30)
            color: Qt.rgba(0.071, 0.075, 0.094, 1.0)
            border.width: 1
            border.color: Theme.surfaceBorder
        }

        Text {
            id: heading
            anchors.top: parent.top
            anchors.topMargin: sheet.pad
            anchors.horizontalCenter: parent.horizontalCenter
            width: sheet.inner
            text: sheet.title
            color: Theme.textSecondary
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(22)
        }

        Text {
            id: pathText
            anchors.top: heading.bottom
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

        // More shortcuts than fit slide under the edges, the focused one kept in view.
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

                Behavior on x {
                    NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                }

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
            highlightFollowsCurrentItem: false
            clip: true

            function scrollToCurrent() {
                if (height <= 0 || contentHeight <= height) {
                    contentY = 0;
                    return;
                }
                var top = currentIndex * sheet.rowHeight;
                var target = contentY;
                if (top < contentY)
                    target = top;
                else if (top + sheet.rowHeight > contentY + height)
                    target = top + sheet.rowHeight - height;
                contentY = Math.max(0, Math.min(target, contentHeight - height));
            }

            onCurrentIndexChanged: scrollToCurrent()
            onCountChanged: scrollToCurrent()

            Behavior on contentY {
                NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutQuint }
            }

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

                    Behavior on color {
                        ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                    }
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
}
