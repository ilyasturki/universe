import QtQuick
import Qt5Compat.GraphicalEffects
import "../core"
import "../sound"

FocusScope {
    id: picker

    // [{ label: "All collections", trailing: "47", icon: "assets/runners/proton.svg" }, ...]: an icon is a file drawn before the label.
    property var options: []
    property int activeIndex: 0
    property int index: 0
    property bool open: false
    property Item anchorItem: null

    signal chosen(int index)
    signal dismissed

    readonly property var hints: [
        {
            glyph: "A",
            label: "Select"
        },
        {
            glyph: "B",
            label: "Close"
        }
    ]

    readonly property real rowHeight: Theme.dp(52)
    readonly property real pad: Theme.dp(10)
    readonly property real inset: Theme.dp(20)
    readonly property real iconSize: Theme.dp(30)
    readonly property bool hasIcons: options.some(function (o) {
        return o.icon !== undefined && o.icon !== "";
    })
    readonly property real maxHeight: Theme.dp(430)

    property real panelWidth: Theme.dp(320)

    width: panelWidth
    height: list.height + pad * 2
    visible: panel.opacity > 0.01

    x: anchorItem ? Math.min(anchorItem.x, parent.width - width) : 0
    y: anchorItem ? anchorItem.y + anchorItem.height + Theme.dp(16) : 0

    // TextMetrics is the only way to size the panel before the rows exist.
    TextMetrics {
        id: metric
        font.family: Theme.sans
        font.weight: Font.Medium
        font.pixelSize: Theme.dp(22)
    }

    function measure() {
        var w = 0;
        for (var i = 0; i < options.length; i++) {
            metric.text = options[i].label;
            var row = metric.width;
            if (options[i].trailing) {
                metric.text = options[i].trailing;
                row += Theme.dp(40) + metric.width;
            }
            w = Math.max(w, row);
        }
        return Math.max(Theme.dp(300), Math.ceil(w) + inset * 2 + pad * 2 + (hasIcons ? iconSize + Theme.dp(14) : 0));
    }

    function show(anchor, opts, active) {
        anchorItem = anchor;
        options = opts;
        activeIndex = active;
        index = active;
        panelWidth = measure();
        open = true;
        list.positionViewAtIndex(active, ListView.Contain);
        forceActiveFocus();
    }

    function hide() {
        open = false;
        focus = false;
    }

    function step(d) {
        index = Sound.stepped(index, d, options.length);
    }

    // The mouse landing anywhere else closes the list, as B would.
    Connections {
        target: Theme
        function onPointed(item) {
            if (!picker.open)
                return;
            for (var i = item; i; i = i.parent)
                if (i === picker)
                    return;
            picker.dismissed();
        }
    }

    Keys.onUpPressed: picker.step(-1)
    Keys.onDownPressed: picker.step(1)
    Keys.onLeftPressed: Sound.edge()
    Keys.onRightPressed: Sound.edge()

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            picker.chosen(picker.index);
        } else if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            event.accepted = true;
            picker.step(api.keys.isFirst(event) ? -options.length : options.length);
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            Sound.cancel();
            picker.dismissed();
        } else if (api.keys.isDetails(event) || api.keys.isFilters(event) || api.keys.isPageUp(event) || api.keys.isPageDown(event)) {
            // The chip bar and the grid below own these; an open list must eat them.
            event.accepted = true;
            Sound.edge();
        }
    }

    Item {
        id: panel

        anchors.fill: parent
        opacity: picker.open ? 1.0 : 0.0
        transform: Translate {
            y: picker.open ? 0 : -Theme.dp(10)
            Behavior on y {
                Ease {
                    duration: Theme.durQuick
                }
            }
        }

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }

        // RectangularGlow paints its whole bounds, so it sits behind the panel.
        RectangularGlow {
            anchors.fill: parent
            glowRadius: Theme.dp(24)
            cornerRadius: Theme.dp(22) + glowRadius
            color: Qt.rgba(0, 0, 0, 0.5)
        }

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(22)
            color: "#121318"
            border.width: 1
            border.color: Theme.surfaceBorder

            HoverHandler {}
            TapHandler {}
        }

        ListView {
            id: list

            Wheel {
                step: picker.rowHeight
            }

            x: picker.pad
            y: picker.pad
            width: parent.width - picker.pad * 2
            height: Math.min(picker.options.length * picker.rowHeight, picker.maxHeight)

            model: picker.options
            currentIndex: picker.index
            interactive: false
            clip: true
            highlightRangeMode: ListView.ApplyRange
            preferredHighlightBegin: 0
            preferredHighlightEnd: height
            highlightMoveDuration: Theme.durQuick

            delegate: Item {
                id: row

                readonly property bool focused: index === picker.index
                readonly property bool active: index === picker.activeIndex

                width: list.width
                height: picker.rowHeight

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
                    direct: true
                    radius: Theme.dp(14)
                    onPicked: picker.index = index
                }

                Image {
                    id: icon
                    anchors.left: parent.left
                    anchors.leftMargin: picker.inset
                    anchors.verticalCenter: parent.verticalCenter
                    width: picker.hasIcons ? picker.iconSize : 0
                    height: picker.iconSize
                    source: modelData.icon ? Qt.resolvedUrl("../" + modelData.icon) : ""
                    asynchronous: true
                    fillMode: Image.PreserveAspectFit
                    sourceSize.height: 128
                    smooth: true
                    mipmap: true
                    visible: picker.hasIcons
                }

                Text {
                    anchors.left: icon.right
                    anchors.leftMargin: picker.hasIcons ? Theme.dp(14) : 0
                    anchors.right: trailing.left
                    anchors.rightMargin: Theme.dp(20)
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.label
                    color: row.focused ? Theme.onLight : row.active ? Theme.text : Theme.textSecondary
                    font.family: Theme.sans
                    font.weight: row.active ? Font.DemiBold : Font.Medium
                    font.pixelSize: Theme.dp(22)
                    elide: Text.ElideRight
                }

                Text {
                    id: trailing
                    anchors.right: parent.right
                    anchors.rightMargin: picker.inset
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.trailing ? modelData.trailing : ""
                    color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Theme.textMuted
                    font.family: Theme.sans
                    font.weight: Font.Medium
                    font.pixelSize: Theme.dp(22)
                }
            }
        }
    }
}
