import QtQuick
import "../core"
import "../sound"

// The console's pop-up list: the tile menu beside a tile, the Power panel, a setting's drop-down.
// Items: { label, detail, glyph, check, toggle, danger, gap }; `check` marks the value in force, `toggle` draws a switch.
Modal {
    id: menu

    property string title: ""
    property var items: []
    property int index: 0
    // Where the panel's top left goes, in this item's coordinates; unset, the panel stands right of centre.
    property var at: null
    property real panelWidth: Theme.dp(560)
    // △ picks for all the games a game's setting reaches; the strip then says which button does what.
    property string alt: ""

    readonly property var hints: alt === "" ? [] : [
        {
            glyph: "Y",
            label: alt
        },
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: "This Game"
        }
    ]
    readonly property real rowHeight: Theme.dp(72)
    // Index → a detail row's height, reported by the row once its text is laid out.
    property var detailHeights: ({})
    readonly property bool marks: items.some(function (i) {
        return i.check !== undefined;
    })
    readonly property bool glyphs: items.some(function (i) {
        return (i.glyph !== undefined && i.glyph !== "") || (i.icon !== undefined && i.icon !== "");
    })

    carded: false
    scrimOpacity: 0.55

    function show(spec, done) {
        title = spec.title || "";
        detailHeights = {};
        items = spec.items || [];
        at = spec.at || null;
        panelWidth = spec.width || Theme.dp(560);
        alt = spec.alt || "";
        index = Math.max(0, Math.min(items.length - 1, spec.index !== undefined ? spec.index : 0));
        present(done);
        list.positionViewAtIndex(index, ListView.Contain);
    }

    function heightOf(i) {
        var item = items[i];
        if (!item || !item.detail)
            return rowHeight;
        return detailHeights[i] !== undefined ? detailHeights[i] : rowHeight + Theme.dp(40);
    }

    function measured(i, h) {
        if (detailHeights[i] === h)
            return;
        var next = Object.assign({}, detailHeights);
        next[i] = h;
        detailHeights = next;
    }

    Keys.onPressed: function (event) {
        event.accepted = true;
        if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            index = Sound.stepped(index, event.key === Qt.Key_Up ? -1 : 1, items.length);
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            Sound.play(items[index] && items[index].toggle !== undefined ? "select" : "ok");
            finish(index);
        } else if (api.keys.isFilters(event) && alt !== "") {
            Sound.play("ok");
            finish(index, true);
        } else if (api.keys.isCancel(event) || api.keys.isMenu(event)) {
            Sound.play("back");
            api.keys.dropHold();
            finish(-1);
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            Sound.play("edge");
        }
    }

    Rectangle {
        id: panel

        readonly property real listHeight: {
            var h = 0;
            for (var i = 0; i < menu.items.length; i++)
                h += menu.heightOf(i) + (menu.items[i].gap ? Theme.dp(18) : 0);
            return Math.min(h, menu.height - Theme.dp(240));
        }

        width: menu.panelWidth
        height: heading.height + panel.listHeight + Theme.dp(20)
        x: menu.at ? Math.max(Theme.dp(40), Math.min(menu.at.x, menu.width - width - Theme.dp(40))) : menu.width - width - Theme.dp(88)
        y: menu.at ? Math.max(Theme.dp(40), Math.min(menu.at.y, menu.height - height - Theme.dp(40))) : (menu.height - height) / 2 + Theme.dp(40)
        radius: Theme.dp(Theme.radiusCard)
        color: Qt.rgba(0.1, 0.11, 0.14, 0.97)
        border.width: 1
        border.color: Theme.glassEdge
        opacity: menu.open ? 1.0 : 0.0
        scale: menu.open ? 1.0 : 0.98
        transformOrigin: Item.TopLeft
        enabled: menu.open

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }

        Block {}

        Label {
            id: heading
            x: Theme.dp(26)
            width: parent.width - x * 2
            height: menu.title !== "" ? Theme.dp(78) : Theme.dp(10)
            verticalAlignment: Text.AlignVCenter
            visible: menu.title !== ""
            text: menu.title
            elide: Text.ElideRight
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        ListView {
            id: list

            y: heading.height
            width: parent.width
            height: panel.listHeight
            model: menu.items
            currentIndex: menu.index
            interactive: false
            clip: true
            highlightFollowsCurrentItem: false
            highlightRangeMode: ListView.ApplyRange
            preferredHighlightBegin: 0
            preferredHighlightEnd: height

            onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)

            delegate: Item {
                id: row

                readonly property bool focused: menu.open && index === menu.index
                readonly property var item: modelData

                width: list.width
                height: menu.heightOf(index) + (item.gap ? Theme.dp(18) : 0)

                Rectangle {
                    visible: row.item.gap === true
                    x: Theme.dp(26)
                    y: Theme.dp(8)
                    width: parent.width - Theme.dp(52)
                    height: 1
                    color: Theme.hairline
                }

                Item {
                    id: body
                    y: row.item.gap ? Theme.dp(18) : 0
                    width: parent.width
                    height: menu.heightOf(index)

                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.dp(4)
                        anchors.rightMargin: Theme.dp(4)
                        radius: Theme.dp(Theme.radiusRow)
                        color: row.focused ? Theme.focusFill : "transparent"
                        border.width: row.focused ? Theme.dp(Theme.ringLine) : 0
                        border.color: Theme.ringSoft

                        Behavior on color {
                            ColorAnimation {
                                duration: Theme.durFocus
                            }
                        }
                    }

                    Item {
                        id: mark
                        x: Theme.dp(28)
                        y: row.item.detail ? Theme.dp(22) : (parent.height - height) / 2
                        width: Theme.dp(menu.glyphs ? 38 : 26)
                        height: width
                        visible: menu.glyphs || menu.marks

                        Glyph {
                            anchors.fill: parent
                            visible: !logo.visible
                            kind: row.item.check === true ? "check" : row.item.glyph || ""
                            tint: row.item.danger ? Theme.danger : Theme.text
                        }

                        // A file (a runner's logo), from the qml root.
                        Image {
                            id: logo
                            anchors.fill: parent
                            visible: row.item.icon !== undefined && row.item.icon !== "" && row.item.check !== true
                            source: visible ? Qt.resolvedUrl("../../" + row.item.icon) : ""
                            asynchronous: true
                            fillMode: Image.PreserveAspectFit
                            sourceSize.height: 128
                            mipmap: true
                        }
                    }

                    Column {
                        x: mark.visible ? mark.x + mark.width + Theme.dp(22) : Theme.dp(30)
                        y: row.item.detail ? Theme.dp(20) : (parent.height - height) / 2
                        width: parent.width - x - (toggle.visible ? toggle.width + Theme.dp(52) : Theme.dp(30))
                        spacing: Theme.dp(8)

                        function report() {
                            if (row.item.detail)
                                menu.measured(index, height + Theme.dp(40));
                        }

                        onHeightChanged: report()
                        Component.onCompleted: report()

                        Label {
                            width: parent.width
                            text: row.item.label || ""
                            color: row.item.danger ? Theme.danger : Theme.text
                            elide: Text.ElideRight
                        }

                        Label {
                            width: parent.width
                            visible: row.item.detail !== undefined && row.item.detail !== ""
                            text: row.item.detail || ""
                            color: Theme.textSecondary
                            wrapMode: Text.WordWrap
                            maximumLineCount: 3
                            elide: Text.ElideRight
                            lineHeight: 1.2
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }
                    }

                    Toggle {
                        id: toggle
                        visible: row.item.toggle !== undefined
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(26)
                        anchors.verticalCenter: parent.verticalCenter
                        on: row.item.toggle === true
                    }

                    Touch {
                        direct: true
                        onPicked: menu.index = index
                    }
                }
            }
        }

        Swipe {
            flickable: list
        }
    }
}
