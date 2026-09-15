import QtQuick
import "../core"
import "../sound"

// Items: [{ icon, label, action, danger }]; the row stays lit above the scrim as a live copy.
FocusScope {
    id: menu

    property bool open: false
    property string title: ""
    property var items: []
    property int index: 0
    property var done: null

    signal dismissed()

    readonly property var hints: [
        { glyph: "A", label: "Select" },
        { glyph: "B", label: "Close" }
    ]

    // The row's rect in the menu's coordinates, taken once at show().
    property real ax: 0
    property real ay: 0
    property real aw: 0
    property real ah: 0
    property real copyMargin: Theme.dp(6)
    property real gap: Theme.dp(28)
    readonly property bool onRight: ax + aw + gap + panel.width <= width - Theme.dp(40)
    property real slide: open ? 0.0 : 1.0

    focus: open
    visible: scrim.opacity > 0.01

    Behavior on slide {
        NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
    }

    function show(list, anchor, rect, heading, after) {
        items = list;
        title = heading || "";
        done = after || null;
        index = 0;
        var p = anchor.mapToItem(menu, rect.x, rect.y);
        ax = p.x;
        ay = p.y;
        aw = rect.width;
        ah = rect.height;
        copy.sourceRect = Qt.rect(rect.x - copyMargin, rect.y - copyMargin, rect.width + copyMargin * 2, rect.height + copyMargin * 2);
        copy.sourceItem = anchor;
        shown++;
        open = true;
        forceActiveFocus();
    }

    function hide() {
        open = false;
        focus = false;
        done = null;
    }

    function cancel() {
        Sound.cancel();
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
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }
    }

    ShaderEffectSource {
        id: copy
        live: true
        hideSource: false
        x: menu.ax - menu.copyMargin
        y: menu.ay - menu.copyMargin
        width: menu.aw + menu.copyMargin * 2
        height: menu.ah + menu.copyMargin * 2
        opacity: menu.open ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
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
        x: (menu.onRight ? menu.ax + menu.aw + menu.gap : menu.ax - menu.gap - width)
           + (menu.onRight ? -1 : 1) * menu.slide * Theme.dp(16)
        y: Math.max(Theme.dp(40), Math.min(menu.ay + menu.ah / 2 - height / 2, menu.height - height - Theme.dp(40)))
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

                Rectangle {
                    id: row

                    readonly property bool focused: index === menu.index
                    readonly property bool danger: modelData.danger === true
                    readonly property color ink: focused ? Theme.onLight : danger ? "#e0655a" : Theme.text

                    width: rows.width
                    height: Theme.dp(66)
                    radius: Theme.dp(16)
                    color: focused ? (danger ? "#e0655a" : Theme.text) : "transparent"

                    Behavior on color {
                        ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
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
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(20)
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.label
                        color: row.ink
                        font.family: Theme.sans
                        font.weight: row.focused ? Font.DemiBold : Font.Medium
                        font.pixelSize: Theme.dp(25)
                        elide: Text.ElideRight
                    }
                }
            }
        }
    }

    Keys.onPressed: function(event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (event.key === Qt.Key_Up || event.key === Qt.Key_Down)
            index = Sound.stepped(index, event.key === Qt.Key_Up ? -1 : 1, items.length);
        else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right)
            Sound.edge();
        else if (api.keys.isAccept(event))
            activate();
        else if (api.keys.isCancel(event) || api.keys.isMenu(event))
            cancel();
    }
}
