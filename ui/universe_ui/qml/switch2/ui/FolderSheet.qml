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

    readonly property real rowHeight: Theme.dp(96)
    readonly property real inner: Theme.dp(1400)
    readonly property real room: Theme.dp(Theme.ringRoom)

    carded: false
    scrimColor: Theme.ground

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

                FocusPill {
                    anchors.fill: parent
                    radius: height / 2
                    color: parent.focused ? Theme.focusFill : Theme.card
                    border.width: 1
                    border.color: Theme.hairline
                    visible: true
                    focused: parent.focused
                }

                Label {
                    id: chipLabel
                    anchors.centerIn: parent
                    text: modelData.label
                    color: parent.here ? Theme.accent : Theme.text
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }
            }
        }
    }

    ListView {
        id: list

        x: (parent.width - sheet.inner) / 2 - sheet.room
        y: chips.y + chips.height + Theme.dp(36) - sheet.room
        width: sheet.inner + sheet.room * 2
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20) + sheet.room
        model: sheet.rowCount
        currentIndex: sheet.index
        interactive: false
        clip: true
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: sheet.room
        preferredHighlightEnd: height - sheet.room
        highlightRangeMode: ListView.ApplyRange
        header: Item {
            height: sheet.room
        }
        footer: Item {
            height: sheet.room
        }

        delegate: Item {
            width: list.width
            height: sheet.rowHeight

            Item {
                readonly property bool focused: sheet.zone === "list" && index === sheet.index
                readonly property var entry: index === 0 ? null : sheet.entries[index - 1]

                x: sheet.room
                width: parent.width - sheet.room * 2
                height: sheet.rowHeight

                FocusPill {
                    anchors.fill: parent
                    focused: parent.focused
                }

                Hairline {
                    visible: !parent.focused
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

                Label {
                    x: rowIcon.x + rowIcon.width + Theme.dp(20)
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - x - Theme.dp(24)
                    text: index === 0 ? ".." : (parent.entry ? parent.entry.name : "")
                    elide: Text.ElideMiddle
                }
            }
        }
    }

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(Theme.edgeMargin)
        anchors.top: list.top
        anchors.topMargin: sheet.room
        anchors.bottom: list.bottom
        anchors.bottomMargin: sheet.room
        flickable: list
    }
}
