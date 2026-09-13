import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: sheet

    property bool open: false
    property string title: ""
    property var callback: null
    property string zone: "list"
    property int chipIndex: 0
    property int index: 0

    readonly property var browser: api.screens.paths
    readonly property bool files: browser.files
    readonly property var entries: browser.entries
    readonly property int rowCount: entries.length + 1

    readonly property var hints: {
        var out = [];
        if (!files)
            out.push({ glyph: "X", label: "Use this folder" });
        out.push({ glyph: "Y", label: "Type a path" });
        out.push({ glyph: "B", label: browser.atRoot ? "Cancel" : "Up" });
        out.push({ glyph: "A", label: zone === "chips" ? "Go" : index === 0 ? "Up" : (entries[index - 1] && !entries[index - 1].dir ? "Choose" : "Open") });
        return out;
    }

    readonly property real rowHeight: Theme.dp(96)
    readonly property real inner: Theme.dp(1400)

    function show(spec, done) {
        title = spec.title || "Choose a folder";
        browser.open(spec.path || "", spec.files === true);
        callback = done || null;
        zone = "list";
        index = 0;
        chipIndex = 0;
        Sound.open();
        open = true;
        forceActiveFocus();
    }

    function finish(path) {
        var cb = callback;
        callback = null;
        open = false;
        if (cb)
            cb(path);
    }

    function activate() {
        if (zone === "chips") {
            Sound.ok();
            browser.go(browser.shortcuts[chipIndex].path);
            zone = "list";
            index = 0;
            return;
        }
        if (index === 0) {
            browser.up() ? Sound.ok() : Sound.edge();
            return;
        }
        var entry = entries[index - 1];
        if (!entry)
            return;
        Sound.ok();
        if (entry.dir) {
            browser.enter(index - 1);
            index = 0;
        } else {
            finish(entry.path);
        }
    }

    anchors.fill: parent
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
    function stepChip(d) {
        var next = chipIndex + d;
        if (zone === "chips" && next >= 0 && next < browser.shortcuts.length) {
            chipIndex = next;
            Sound.tick();
        } else {
            Sound.edge();
        }
    }
    Keys.onLeftPressed: stepChip(-1)
    Keys.onRightPressed: stepChip(1)

    Keys.onPressed: function(event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            activate();
        } else if (api.keys.isCancel(event)) {
            Sound.back();
            if (browser.up())
                index = 0;
            else
                finish(null);
        } else if (api.keys.isDetails(event)) {
            if (files) {
                Sound.edge();
            } else {
                Sound.ok();
                finish(browser.path);
            }
        } else if (api.keys.isFilters(event)) {
            Sound.ok();
            var current = browser.path;
            open = false;
            sheet.typeRequested(current);
        }
    }

    // The page that owns the sheet answers with the keyboard and then calls finish().
    signal typeRequested(string path)

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Theme.ground
        opacity: sheet.open ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "folder"
        title: sheet.title
        trailing: sheet.browser.display(sheet.browser.path)
    }

    Row {
        id: chips

        x: (parent.width - sheet.inner) / 2
        y: header.height + Theme.dp(36)
        spacing: Theme.dp(18)

        Repeater {
            model: sheet.browser.shortcuts

            Item {
                readonly property bool focused: sheet.zone === "chips" && index === sheet.chipIndex
                readonly property bool here: modelData.path === sheet.browser.path

                width: chipLabel.implicitWidth + Theme.dp(56)
                height: Theme.dp(72)

                Rectangle {
                    id: chipFill
                    anchors.fill: parent
                    radius: height / 2
                    color: parent.focused ? Theme.focusFill : Theme.card
                    border.width: 1
                    border.color: Theme.hairline
                }

                FocusOutline {
                    target: chipFill
                    cornerRadius: chipFill.radius
                    gap: 0
                    shown: parent.focused && sheet.open
                }

                Text {
                    id: chipLabel
                    anchors.centerIn: parent
                    text: modelData.label
                    color: parent.here ? Theme.accent : Theme.text
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }
            }
        }
    }

    ListView {
        id: list

        x: (parent.width - sheet.inner) / 2
        y: chips.y + chips.height + Theme.dp(36)
        width: sheet.inner
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        model: sheet.rowCount
        currentIndex: sheet.index
        interactive: false
        clip: true
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: 0
        preferredHighlightEnd: height
        highlightRangeMode: ListView.ApplyRange

        delegate: Item {
            readonly property bool focused: sheet.zone === "list" && index === sheet.index
            readonly property var entry: index === 0 ? null : sheet.entries[index - 1]

            width: list.width
            height: sheet.rowHeight

            Rectangle {
                id: pill
                anchors.fill: parent
                radius: Theme.dp(Theme.radiusRow)
                color: Theme.focusFill
                visible: parent.focused
            }

            FocusOutline {
                target: pill
                cornerRadius: pill.radius
                gap: 0
                shown: parent.focused && sheet.open
            }

            Rectangle {
                anchors.bottom: parent.bottom
                anchors.left: parent.left
                anchors.right: parent.right
                height: 1
                visible: !parent.focused
                color: Theme.hairlineSoft
            }

            Glyph {
                id: rowIcon
                x: Theme.dp(24)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(40)
                height: width
                kind: index === 0 ? "chevron-left" : (parent.entry && parent.entry.dir ? "folder" : "film")
                tint: Theme.text
            }

            Text {
                x: rowIcon.x + rowIcon.width + Theme.dp(20)
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - x - Theme.dp(24)
                text: index === 0 ? ".." : (parent.entry ? parent.entry.name : "")
                color: Theme.text
                elide: Text.ElideMiddle
                font.family: Theme.sans
                font.pixelSize: Theme.dp(Theme.fontBody)
            }
        }
    }

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(Theme.edgeMargin)
        anchors.top: list.top
        anchors.bottom: list.bottom
        flickable: list
    }
}
