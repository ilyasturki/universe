import QtQuick
import "../core"
import "../sound"

// Items: MenuRow's plus `action` and `gap`; the row stays lit above the scrim as a live copy. `detail` sits at the row's right,
// `more` draws a chevron there and lets Right open it, `gap` parts the row from the one above. With no anchor the list sits
// in the middle of the screen, a question: `title` is asked, `note` says more under it.
FocusScope {
    id: menu

    property bool open: false
    property string title: ""
    property string note: ""
    property bool centered: false
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

    function show(list, anchor, rect, heading, after, start) {
        items = list;
        title = heading || "";
        note = "";
        done = after || null;
        index = start || 0;
        centered = !anchor;
        if (anchor) {
            var p = anchor.mapToItem(menu, rect.x, rect.y);
            row = Qt.rect(p.x, p.y, rect.width, rect.height);
            copy.sourceRect = Qt.rect(rect.x - copyMargin, rect.y - copyMargin, rect.width + copyMargin * 2, rect.height + copyMargin * 2);
        }
        copy.sourceItem = anchor || null;
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
        note = "";
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

    // A list taller than the screen scrolls under the cursor.
    onIndexChanged: Qt.callLater(reveal)

    function reveal() {
        var slot = slots.itemAt(index);
        var top = scroller.contentY;
        if (slot && slot.y < top)
            top = slot.y;
        else if (slot && slot.y + slot.height > top + scroller.height)
            top = slot.y + slot.height - scroller.height;
        scroller.contentY = Math.max(0, Math.min(top, scroller.contentHeight - scroller.height));
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Qt.rgba(0.02, 0.02, 0.03, 1)
        opacity: menu.open ? 0.62 : 0.0

        Behavior on opacity {
            Ease {}
        }

        // Keeps the mouse off the page beneath; a click outside the panel is B.
        HoverHandler {}
        TapHandler {
            onTapped: api.keys.press("Cancel")
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

        readonly property real room: menu.height - Theme.dp(80)

        width: Theme.dp(menu.centered ? 640 : 460)
        height: Math.min(room, scroller.contentHeight + Theme.dp(24) + (head.visible ? head.height + Theme.dp(8) : 0))
        radius: Theme.dp(24)
        color: "#1b1d24"
        border.width: 1
        border.color: Theme.surfaceBorder
        x: menu.centered ? (menu.width - width) / 2 : (menu.onRight ? menu.row.x + menu.row.width + menu.gap : menu.row.x - menu.gap - width) + (menu.onRight ? -1 : 1) * menu.slide * Theme.dp(16)
        y: menu.centered ? (menu.height - height) / 2 : Math.max(Theme.dp(40), Math.min(menu.row.y + menu.row.height / 2 - height / 2, menu.height - height - Theme.dp(40)))
        opacity: 1.0 - menu.slide
        scale: 1.0 - menu.slide * 0.04
        transformOrigin: menu.centered ? Item.Center : menu.onRight ? Item.Left : Item.Right

        // The panel's own padding is neither a row nor the scrim.
        HoverHandler {}
        TapHandler {}

        Column {
            id: head

            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.topMargin: Theme.dp(menu.centered ? 30 : 12)
            anchors.leftMargin: Theme.dp(34)
            anchors.rightMargin: Theme.dp(34)
            spacing: Theme.dp(10)
            visible: menu.title !== ""

            Text {
                width: parent.width
                topPadding: menu.centered ? 0 : Theme.dp(9)
                bottomPadding: menu.centered ? 0 : Theme.dp(9)
                text: menu.title
                color: menu.centered ? Theme.text : Theme.textSecondary
                font.family: Theme.sans
                font.weight: menu.centered ? Font.DemiBold : Font.Medium
                font.pixelSize: Theme.dp(menu.centered ? 28 : 21)
                wrapMode: menu.centered ? Text.WordWrap : Text.NoWrap
                elide: menu.centered ? Text.ElideNone : Text.ElideRight
            }

            Text {
                width: parent.width
                visible: menu.note !== ""
                bottomPadding: Theme.dp(10)
                text: menu.note
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                wrapMode: Text.WordWrap
                lineHeight: 1.2
            }
        }

        Flickable {
            id: scroller

            anchors.top: head.visible ? head.bottom : parent.top
            anchors.topMargin: head.visible ? Theme.dp(8) : Theme.dp(12)
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.dp(12)
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: Theme.dp(12)
            anchors.rightMargin: Theme.dp(12)
            contentHeight: rows.height
            interactive: false
            clip: contentHeight > height

            // The panel settles its height after the rows: the cursor's row is placed again once it has.
            onHeightChanged: menu.reveal()
            onContentHeightChanged: menu.reveal()

            Behavior on contentY {
                Ease {
                    duration: Theme.durQuick
                }
            }

            Column {
                id: rows

                width: parent.width
                spacing: Theme.dp(4)

                Repeater {
                    id: slots

                    model: menu.items

                    Item {
                        id: slot

                        readonly property bool parted: modelData.gap === true

                        width: rows.width
                        height: line.height + (parted ? menu.gapHeight : 0)

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

                        MenuRow {
                            id: line

                            anchors.bottom: parent.bottom
                            width: parent.width
                            item: modelData
                            focused: index === menu.index
                            onPicked: menu.index = index
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
        else if (api.keys.isFirst(event) || api.keys.isLast(event))
            index = Sound.stepped(index, api.keys.isFirst(event) ? -items.length : items.length, items.length);
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
