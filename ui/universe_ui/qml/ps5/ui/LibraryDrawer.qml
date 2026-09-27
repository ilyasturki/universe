import QtQuick
import "../core"
import "../sound"

// Sort and Filter: the panel that slides out beside the Game Library's rail, the grid dimmed behind it.
FocusScope {
    id: drawer

    property bool open: false
    // [{ id, label, value, toggle, button, caption, disabled }]
    property var rows: []
    property int index: 0
    property real panelX: Theme.dp(142)
    property real panelY: Theme.dp(300)

    signal activated(string id)
    signal closed

    readonly property real rowHeight: Theme.dp(72)
    readonly property real captionHeight: Theme.dp(64)
    readonly property alias panel: panel
    readonly property var currentRow: index >= 0 && index < rows.length ? rows[index] : null

    anchors.fill: parent
    visible: scrim.opacity > 0.01
    focus: open

    function heightOf(i) {
        var r = rows[i];
        return r && r.caption ? captionHeight : r && r.button ? rowHeight + Theme.dp(30) : rowHeight;
    }

    function rowY(i) {
        var y = Theme.dp(12);
        for (var k = 0; k < i; k++)
            y += heightOf(k);
        return y;
    }

    function stops() {
        var out = [];
        for (var i = 0; i < rows.length; i++)
            if (!rows[i].caption)
                out.push(i);
        return out;
    }

    function reset() {
        var s = stops();
        index = s.length > 0 ? s[0] : 0;
    }

    function step(d) {
        var s = stops();
        var at = s.indexOf(index);
        var next = at + d;
        if (at < 0 || next < 0 || next >= s.length) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        index = s[next];
    }

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)
    Keys.onLeftPressed: Sound.play("edge")
    Keys.onRightPressed: Sound.play("edge")
    Keys.onPressed: function (event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            var r = drawer.currentRow;
            if (!r || r.caption || r.disabled) {
                Sound.play("edge");
                return;
            }
            Sound.play(r.toggle !== undefined ? "select" : "ok");
            drawer.activated(r.id);
        } else if (api.keys.isCancel(event) || api.keys.isMenu(event)) {
            Sound.play("back");
            drawer.closed();
        }
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Qt.rgba(0, 0, 0, 0.62)
        opacity: drawer.open ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }

        Block {
            onTapped: if (drawer.open)
                drawer.closed()
        }
    }

    Rectangle {
        id: panel

        x: drawer.panelX - (drawer.open ? 0 : Theme.dp(40))
        y: drawer.panelY
        width: Theme.dp(360)
        height: Math.min(drawer.rowY(drawer.rows.length) + Theme.dp(20), drawer.height - y - Theme.dp(40))
        color: Qt.rgba(0.09, 0.1, 0.13, 0.97)
        border.width: 1
        border.color: Theme.glassEdge
        opacity: drawer.open ? 1.0 : 0.0
        enabled: drawer.open
        clip: true

        Behavior on x {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }

        Block {}

        Repeater {
            model: drawer.rows

            Item {
                id: line

                readonly property var row: modelData
                readonly property bool focused: drawer.open && drawer.activeFocus && index === drawer.index

                y: drawer.rowY(index)
                width: panel.width
                height: drawer.heightOf(index)

                Label {
                    visible: line.row.caption === true
                    x: Theme.dp(30)
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: Theme.dp(12)
                    text: line.row.label || ""
                    color: Theme.textMuted
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }

                Rectangle {
                    visible: line.row.caption === true && index > 0
                    x: Theme.dp(30)
                    width: parent.width - Theme.dp(60)
                    height: 1
                    color: Theme.hairline
                }

                Item {
                    visible: line.row.caption !== true
                    anchors.fill: parent
                    anchors.topMargin: line.row.button ? Theme.dp(30) : 0

                    Rectangle {
                        anchors.fill: parent
                        anchors.leftMargin: Theme.dp(10)
                        anchors.rightMargin: Theme.dp(10)
                        radius: Theme.dp(Theme.radiusRow)
                        color: line.focused ? Theme.focusFill : line.row.button ? Qt.rgba(1, 1, 1, 0.06) : "transparent"
                        border.width: line.focused ? Theme.dp(Theme.ringLine) : 0
                        border.color: Theme.ringSoft

                        Behavior on color {
                            ColorAnimation {
                                duration: Theme.durFocus
                            }
                        }
                    }

                    Label {
                        x: line.row.button ? (parent.width - width) / 2 : Theme.dp(30)
                        anchors.verticalCenter: parent.verticalCenter
                        width: line.row.button ? implicitWidth : Math.min(implicitWidth, parent.width - Theme.dp(60) - (valueText.visible ? valueText.implicitWidth + Theme.dp(16) : 0) - (toggle.visible ? toggle.width + Theme.dp(16) : 0))
                        text: line.row.label || ""
                        color: line.row.disabled ? Theme.textDisabled : Theme.text
                        elide: Text.ElideRight
                        font.weight: line.row.button ? Font.DemiBold : Font.Light
                        font.pixelSize: Theme.dp(line.row.button ? 28 : 30)
                    }

                    Label {
                        id: valueText
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(30)
                        anchors.verticalCenter: parent.verticalCenter
                        visible: line.row.value !== undefined && line.row.value !== ""
                        width: Math.min(implicitWidth, parent.width * 0.5)
                        text: line.row.value || ""
                        color: Theme.textSecondary
                        elide: Text.ElideRight
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }

                    Toggle {
                        id: toggle
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(30)
                        anchors.verticalCenter: parent.verticalCenter
                        visible: line.row.toggle !== undefined
                        on: line.row.toggle === true
                    }

                    Touch {
                        direct: true
                        onPicked: {
                            if (drawer.index !== index)
                                Sound.play("tick");
                            drawer.index = index;
                            drawer.forceActiveFocus();
                        }
                    }
                }
            }
        }
    }
}
