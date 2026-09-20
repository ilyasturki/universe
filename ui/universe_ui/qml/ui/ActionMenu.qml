import QtQuick
import "../core"
import "../sound"

// Items: [{ icon, label, action, danger, detail, more, gap }]; the row stays lit above the scrim as a live copy.
// `detail` sits at the row's right, `more` draws a chevron there and lets Right open it, `gap` parts the row from the one above.
FocusScope {
    id: menu

    property bool open: false
    property string title: ""
    property var items: []
    property int index: 0
    property var done: null

    signal dismissed

    readonly property var hints: [
        {
            glyph: "A",
            label: "Select"
        },
        {
            glyph: "B",
            label: stack.length > 0 ? "Back" : "Close"
        }
    ]

    // The row's rect in the menu's coordinates, taken once at show().
    property rect row: Qt.rect(0, 0, 0, 0)
    property real copyMargin: Theme.dp(6)
    property real gap: Theme.dp(28)
    readonly property real gapHeight: Theme.dp(17)
    readonly property bool onRight: row.x + row.width + gap + panel.width <= width - Theme.dp(40)
    property real slide: open ? 0.0 : 1.0

    focus: open
    visible: scrim.opacity > 0.01

    Behavior on slide {
        Ease {}
    }

    function show(list, anchor, rect, heading, after) {
        items = list;
        title = heading || "";
        done = after || null;
        index = 0;
        var p = anchor.mapToItem(menu, rect.x, rect.y);
        row = Qt.rect(p.x, p.y, rect.width, rect.height);
        copy.sourceRect = Qt.rect(rect.x - copyMargin, rect.y - copyMargin, rect.width + copyMargin * 2, rect.height + copyMargin * 2);
        copy.sourceItem = anchor;
        shown++;
        asking = false;
        open = true;
        forceActiveFocus();
    }

    // Keep or do: `done` runs on the second item; the focus goes back to `anchor` either way.
    function confirm(keep, icon, label, title, anchor, rect, done) {
        show([
            {
                icon: "",
                label: keep,
                action: ""
            },
            {
                icon: icon,
                label: label,
                action: "yes",
                danger: true
            }
        ], anchor, rect, title, function (action) {
            if (action === "yes")
                done();
            anchor.forceActiveFocus();
        });
        asking = true;
    }

    // A list over the one showing: B comes back to it, A on a row runs `after` in its place.
    property var stack: []

    function push(list, heading, after) {
        stack = stack.concat([
            {
                items: items,
                title: title,
                done: done,
                index: index
            }
        ]);
        items = list;
        title = heading || "";
        done = after || null;
        index = 0;
        shown++;
    }

    function pop() {
        var top = stack[stack.length - 1];
        stack = stack.slice(0, -1);
        items = top.items;
        title = top.title;
        done = top.done;
        index = top.index;
    }

    function hide() {
        open = false;
        focus = false;
        done = null;
        stack = [];
    }

    // A question B just closed: holding on does not ask to quit over it.
    property bool asking: false

    function cancel() {
        Sound.cancel();
        if (stack.length > 0) {
            pop();
            return;
        }
        if (asking)
            api.keys.dropHold();
        hide();
        dismissed();
    }

    // A handler may show() a follow-up (a confirmation) in place; the menu closes otherwise.
    property int shown: 0

    function activate() {
        var was = shown, after = done;
        if (after)
            after(items[index].action);
        if (shown === was)
            hide();
    }

    onVisibleChanged: {
        if (!visible)
            copy.sourceItem = null;
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Qt.rgba(0.02, 0.02, 0.03, 1)
        opacity: menu.open ? 0.62 : 0.0

        Behavior on opacity {
            Ease {}
        }
    }

    ShaderEffectSource {
        id: copy
        live: true
        hideSource: false
        x: menu.row.x - menu.copyMargin
        y: menu.row.y - menu.copyMargin
        width: menu.row.width + menu.copyMargin * 2
        height: menu.row.height + menu.copyMargin * 2
        opacity: menu.open ? 1.0 : 0.0

        Behavior on opacity {
            Ease {}
        }
    }

    Rectangle {
        id: panel

        width: Theme.dp(460)
        height: rows.height + Theme.dp(24) + (heading.visible ? heading.height + Theme.dp(8) : 0)
        radius: Theme.dp(24)
        color: "#1b1d24"
        border.width: 1
        border.color: Theme.surfaceBorder
        x: (menu.onRight ? menu.row.x + menu.row.width + menu.gap : menu.row.x - menu.gap - width) + (menu.onRight ? -1 : 1) * menu.slide * Theme.dp(16)
        y: Math.max(Theme.dp(40), Math.min(menu.row.y + menu.row.height / 2 - height / 2, menu.height - height - Theme.dp(40)))
        opacity: 1.0 - menu.slide
        scale: 1.0 - menu.slide * 0.04
        transformOrigin: menu.onRight ? Item.Left : Item.Right

        Text {
            id: heading

            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: Theme.dp(12)
            anchors.leftMargin: Theme.dp(34)
            anchors.rightMargin: Theme.dp(34)
            height: visible ? Theme.dp(44) : 0
            verticalAlignment: Text.AlignVCenter
            visible: menu.title !== ""
            text: menu.title
            color: Theme.textSecondary
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(21)
            elide: Text.ElideRight
        }

        Column {
            id: rows

            anchors.top: heading.visible ? heading.bottom : parent.top
            anchors.topMargin: heading.visible ? Theme.dp(8) : Theme.dp(12)
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: Theme.dp(12)
            anchors.rightMargin: Theme.dp(12)
            spacing: Theme.dp(4)

            Repeater {
                model: menu.items

                Item {
                    id: slot

                    readonly property bool parted: modelData.gap === true

                    width: rows.width
                    height: row.height + (parted ? menu.gapHeight : 0)

                    Rectangle {
                        anchors.top: parent.top
                        anchors.topMargin: (menu.gapHeight - height) / 2
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.leftMargin: Theme.dp(10)
                        anchors.rightMargin: Theme.dp(10)
                        height: 1
                        visible: slot.parted
                        color: Theme.surfaceBorder
                    }

                    Rectangle {
                        id: row

                        readonly property bool focused: index === menu.index
                        readonly property bool danger: modelData.danger === true
                        readonly property color ink: focused ? Theme.onLight : danger ? "#e0655a" : Theme.text
                        readonly property color inkSoft: focused ? Theme.onLight : Theme.textSecondary

                        anchors.bottom: parent.bottom
                        width: parent.width
                        height: Theme.dp(66)
                        radius: Theme.dp(16)
                        color: focused ? (danger ? "#e0655a" : Theme.text) : "transparent"

                        Behavior on color {
                            ColorEase {}
                        }

                        MenuGlyph {
                            id: glyph

                            anchors.left: parent.left
                            anchors.leftMargin: Theme.dp(22)
                            anchors.verticalCenter: parent.verticalCenter
                            width: Theme.dp(26)
                            height: Theme.dp(26)
                            visible: modelData.icon !== undefined && modelData.icon !== ""
                            kind: modelData.icon || ""
                            tint: row.ink
                        }

                        Text {
                            anchors.left: glyph.visible ? glyph.right : parent.left
                            anchors.leftMargin: glyph.visible ? Theme.dp(18) : Theme.dp(22)
                            anchors.right: trailing.left
                            anchors.rightMargin: Theme.dp(12)
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.label
                            color: row.ink
                            font.family: Theme.sans
                            font.weight: row.focused ? Font.DemiBold : Font.Medium
                            font.pixelSize: Theme.dp(25)
                            elide: Text.ElideRight
                        }

                        Row {
                            id: trailing

                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(20)
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: Theme.dp(8)

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: modelData.detail !== undefined && modelData.detail !== ""
                                text: modelData.detail || ""
                                color: row.inkSoft
                                font.family: Theme.sans
                                font.weight: Font.Medium
                                font.pixelSize: Theme.dp(21)
                            }

                            MenuGlyph {
                                anchors.verticalCenter: parent.verticalCenter
                                width: Theme.dp(22)
                                height: Theme.dp(22)
                                visible: modelData.more === true
                                kind: "chevron"
                                tint: row.inkSoft
                            }
                        }
                    }
                }
            }
        }
    }

    Keys.onPressed: function (event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (event.key === Qt.Key_Up || event.key === Qt.Key_Down)
            index = Sound.stepped(index, event.key === Qt.Key_Up ? -1 : 1, items.length);
        else if (event.key === Qt.Key_Right)
            items[index].more === true ? activate() : Sound.edge();
        else if (event.key === Qt.Key_Left)
            stack.length > 0 ? cancel() : Sound.edge();
        else if (api.keys.isAccept(event))
            activate();
        else if (api.keys.isCancel(event))
            cancel();
        else if (api.keys.isMenu(event)) {
            stack = [];
            cancel();
        }
    }
}
