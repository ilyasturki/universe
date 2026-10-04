import QtQuick
import "../core"
import "../sound"

Modal {
    id: sheet

    property var shell: null
    property string title: ""
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
            out.push({
                glyph: "X",
                label: "Use this folder"
            });
        out.push({
            glyph: "Y",
            label: "Type a path"
        });
        out.push({
            glyph: "B",
            label: browser.atRoot ? "Cancel" : "Up"
        });
        out.push({
            glyph: "A",
            label: zone === "chips" ? "Go" : index === 0 ? "Up" : (entries[index - 1] && !entries[index - 1].dir ? "Choose" : "Open")
        });
        return out;
    }

    readonly property real rowHeight: Theme.dp(88)

    carded: false
    scrimColor: Theme.ground
    scrimOpacity: 0.97

    function show(spec, done) {
        title = spec.title || "Choose a folder";
        browser.open(spec.path || "", spec.files === true);
        zone = "list";
        index = 0;
        chipIndex = 0;
        present(done);
    }

    function activate() {
        if (zone === "chips") {
            Sound.play("ok");
            browser.go(browser.shortcuts[chipIndex].path);
            zone = "list";
            index = 0;
            return;
        }
        if (index === 0) {
            browser.up() ? Sound.play("ok") : Sound.play("edge");
            return;
        }
        var entry = entries[index - 1];
        if (!entry)
            return;
        Sound.play("ok");
        if (entry.dir) {
            browser.enter(index - 1);
            index = 0;
        } else {
            finish(entry.path);
        }
    }

    function typePath() {
        Sound.play("ok");
        var done = callback;
        callback = null;
        open = false;
        shell.prompt({
            title: "Path",
            value: browser.path,
            path: true
        }, done);
    }

    Keys.onUpPressed: {
        if (zone === "list" && index > 0)
            index--;
        else if (zone === "list" && browser.shortcuts.length > 0)
            zone = "chips";
        else {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
    }
    Keys.onDownPressed: {
        if (zone === "chips")
            zone = "list";
        else if (index < rowCount - 1)
            index++;
        else {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
    }
    function stepChip(d) {
        var next = chipIndex + d;
        if (zone === "chips" && next >= 0 && next < browser.shortcuts.length) {
            chipIndex = next;
            Sound.play("tick");
        } else {
            Sound.play("edge");
        }
    }
    Keys.onLeftPressed: stepChip(-1)
    Keys.onRightPressed: stepChip(1)

    Keys.onPressed: function (event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            activate();
        } else if (api.keys.isCancel(event)) {
            Sound.play("back");
            if (browser.up())
                index = 0;
            else
                finish(null);
        } else if (api.keys.isDetails(event)) {
            if (files)
                Sound.play("edge");
            else {
                Sound.play("ok");
                finish(browser.path);
            }
        } else if (api.keys.isFilters(event)) {
            typePath();
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "folder"
        title: sheet.title
        trailing: sheet.browser.display(sheet.browser.path)
    }

    // At least PillButton's ring offset, or the clip cuts the focused chip's ring.
    readonly property real chipRoom: Theme.dp(8)

    ListView {
        id: chips
        objectName: "chips"

        x: Theme.dp(Theme.edge) - sheet.chipRoom
        y: header.height + Theme.dp(10) - sheet.chipRoom
        width: parent.width - Theme.dp(Theme.edge) - Theme.dp(Theme.columnRight) + sheet.chipRoom * 2
        height: count > 0 ? Theme.dp(58) + sheet.chipRoom * 2 : 0
        orientation: ListView.Horizontal
        spacing: Theme.dp(20)
        model: sheet.browser.shortcuts
        currentIndex: sheet.chipIndex
        interactive: false
        clip: true
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: sheet.chipRoom
        preferredHighlightEnd: width - sheet.chipRoom
        highlightRangeMode: ListView.ApplyRange
        highlightMoveDuration: Theme.durMove
        highlightMoveVelocity: -1
        header: Item {
            width: sheet.chipRoom
        }
        footer: Item {
            width: sheet.chipRoom
        }

        delegate: Item {
            width: chip.width
            height: chips.height

            PillButton {
                id: chip
                y: sheet.chipRoom
                text: modelData.label
                height: Theme.dp(58)
                fontSize: Theme.dp(Theme.fontSmall)
                focused: sheet.zone === "chips" && index === sheet.chipIndex
                danger: false
                onPicked: {
                    sheet.zone = "chips";
                    sheet.chipIndex = index;
                }
            }
        }
    }

    ListView {
        id: list

        x: Theme.dp(Theme.edge)
        y: header.height + Theme.dp(10) + (chips.count > 0 ? Theme.dp(58) : 0) + Theme.dp(34)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: parent.height - y - Theme.dp(96)
        model: sheet.rowCount
        currentIndex: sheet.index
        interactive: false
        clip: true
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: Theme.dp(8)
        preferredHighlightEnd: height - Theme.dp(8)
        highlightRangeMode: ListView.ApplyRange
        header: Item {
            height: Theme.dp(8)
        }

        delegate: Item {
            id: line

            readonly property bool focused: sheet.zone === "list" && index === sheet.index
            readonly property var entry: index === 0 ? null : sheet.entries[index - 1]

            width: list.width
            height: sheet.rowHeight

            Rectangle {
                anchors.fill: parent
                anchors.margins: Theme.dp(2)
                radius: Theme.dp(Theme.radiusRow)
                color: line.focused ? Theme.focusFill : "transparent"
                border.width: line.focused ? Theme.dp(Theme.ringLine) : 0
                border.color: Theme.ringSoft
            }

            Rectangle {
                x: Theme.dp(88)
                anchors.bottom: parent.bottom
                width: parent.width - x - Theme.dp(20)
                height: 1
                color: Theme.hairline
                visible: !line.focused
            }

            Glyph {
                id: rowIcon
                x: Theme.dp(28)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(38)
                height: width
                kind: index === 0 ? "back" : (line.entry && line.entry.dir ? "folder" : "file")
            }

            Label {
                x: Theme.dp(88)
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - x - Theme.dp(24)
                text: index === 0 ? ".." : (line.entry ? line.entry.name : "")
                elide: Text.ElideMiddle
            }

            Touch {
                current: line.focused
                onPicked: {
                    Sound.play("tick");
                    sheet.zone = "list";
                    sheet.index = index;
                }
            }
        }
    }

    Swipe {
        flickable: list
    }

    Scrollbar {
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(24)
        anchors.top: list.top
        anchors.bottom: list.bottom
        flickable: list
    }
}
