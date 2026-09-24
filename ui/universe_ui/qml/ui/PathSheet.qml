import QtQuick
import "../core"
import "../sound"

Sheet {
    id: sheet

    property int index: 0

    signal accepted(string path)
    signal typeRequested(string path)
    signal dismissed

    readonly property var browser: api.screens.paths
    readonly property bool files: browser.files
    readonly property var entries: browser.entries
    readonly property int rowCount: entries.length + 1

    readonly property var hints: menu.open ? menu.hints : [
        {
            glyph: "A",
            label: index === 0 ? "Up" : (entries[index - 1] && !entries[index - 1].dir ? "Choose" : "Open")
        },
        {
            glyph: "Start",
            label: "More"
        },
        {
            glyph: "B",
            label: "Cancel"
        }
    ]

    readonly property real rowHeight: Theme.dp(58)
    readonly property int visibleRows: 8

    innerMax: Theme.dp(1100)
    contentHeight: Theme.dp(10) + pathText.height + Theme.dp(12) + list.height
    Keys.enabled: !menu.open

    function show(label, path, withFiles) {
        title = label;
        browser.open(path || "", withFiles === true);
        index = 0;
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

    function useFolder() {
        if (files) {
            Sound.edge();
        } else {
            Sound.enter();
            finish(browser.path);
        }
    }

    function typePath() {
        Sound.panel();
        open = false;
        focus = false;
        typeRequested(browser.path);
    }

    // Centred: a row runs the sheet's width and leaves the list no side to open on.
    function openMenu() {
        var items = files ? [] : [
            {
                icon: "check",
                label: "Use this folder",
                action: "use"
            }
        ];
        items.push({
            icon: "keyboard",
            label: "Type a path…",
            action: "type"
        });
        if (browser.shortcuts.length > 0)
            items.push({
                icon: "folder",
                label: "Go to",
                action: "go",
                more: true
            });
        Sound.panel();
        menu.show(items, null, Qt.rect(0, 0, 0, 0), "", function (action) {
            if (action === "go") {
                Sound.enter();
                menu.push(browser.shortcuts.map(function (s) {
                    return {
                        icon: "folder",
                        label: s.label,
                        action: s.path
                    };
                }), "Go to", function (path) {
                    Sound.enter();
                    browser.go(path);
                    index = 0;
                    sheet.forceActiveFocus();
                });
                return;
            }
            sheet.forceActiveFocus();
            if (action === "use")
                useFolder();
            else if (action === "type")
                typePath();
        });
    }

    Keys.onUpPressed: {
        if (index === 0) {
            Sound.edge();
        } else {
            index--;
            Sound.tick();
        }
    }
    Keys.onDownPressed: {
        if (index < rowCount - 1) {
            index++;
            Sound.tick();
        } else {
            Sound.edge();
        }
    }

    Keys.onPressed: function (event) {
        event.accepted = true;
        if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            index = Sound.stepped(index, api.keys.isFirst(event) ? -rowCount : rowCount, rowCount);
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event))
            activate();
        else if (api.keys.isCancel(event))
            cancel();
        else if (api.keys.isMenu(event))
            openMenu();
        else if (api.keys.isDetails(event))
            useFolder();
        else if (api.keys.isFilters(event))
            typePath();
        else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right)
            Sound.edge();
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

    ListView {
        id: list

        Wheel {
            step: sheet.rowHeight
        }

        anchors.top: pathText.bottom
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
            readonly property bool focused: index === sheet.index
            readonly property color ink: focused ? Theme.onLight : Theme.text

            width: list.width
            height: sheet.rowHeight

            Rectangle {
                anchors.fill: parent
                anchors.margins: Theme.dp(2)
                radius: Theme.dp(14)
                color: row.focused ? Theme.text : "transparent"

                Behavior on color {
                    ColorEase {}
                }
            }

            Pointer {
                current: row.focused
                radius: Theme.dp(14)
                onPicked: sheet.index = index
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

    // The sheet's own content sits in its panel; the menu covers the whole sheet, scrim and all.
    ActionMenu {
        id: menu

        parent: sheet
        anchors.fill: parent
        z: 5

        onDismissed: sheet.forceActiveFocus()
    }
}
