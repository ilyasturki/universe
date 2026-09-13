import QtQuick
import "../core"
import "../sound"

// The Settings tab's sidebar: one entry per section. The open one is the white cursor while the
// list has the focus and a quiet fill once the content has it, so one thing on screen is white.
// Up and Down change the section as they go; Right or A hand the focus to the content, B and Up
// past the top leave for the tab bar.
FocusScope {
    id: list

    property var sections: []
    property var icons: []
    property var badges: []
    property int current: 0

    signal requested(int index)
    signal entered()
    signal escapedUp()

    readonly property real entryHeight: Theme.dp(58)
    readonly property real entrySpacing: Theme.dp(6)

    implicitHeight: column.height

    // Round trip: past the last section comes the first. The page owns `current`.
    function step(d) {
        var n = sections.length;
        list.requested((current + d + n) % n);
        Sound.tick();
    }

    Keys.onUpPressed: function(event) {
        if (list.current === 0)
            list.escapedUp();
        else
            list.step(-1);
    }
    Keys.onDownPressed: function(event) {
        if (list.current === list.sections.length - 1)
            Sound.edge();
        else
            list.step(1);
    }
    Keys.onLeftPressed: Sound.edge()
    Keys.onRightPressed: function(event) {
        Sound.panel();
        list.entered();
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            Sound.panel();
            list.entered();
            return;
        }
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            Sound.cancel();
            list.escapedUp();
            return;
        }
    }

    Column {
        id: column

        width: parent.width
        spacing: list.entrySpacing

        Repeater {
            model: list.sections

            Item {
                readonly property bool active: index === list.current
                readonly property bool focused: active && list.activeFocus
                readonly property string badge: index < list.badges.length ? String(list.badges[index] || "") : ""
                readonly property color ink: focused ? Theme.onLight : Theme.text

                width: parent.width
                height: list.entryHeight

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.dp(14)
                    color: focused ? Theme.text : active ? Qt.rgba(1, 1, 1, 0.09) : "transparent"

                    Behavior on color {
                        ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                    }
                }

                MenuGlyph {
                    id: icon
                    anchors.left: parent.left
                    anchors.leftMargin: Theme.dp(18)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(24)
                    height: width
                    kind: index < list.icons.length ? list.icons[index] : ""
                    tint: focused ? Theme.onLight : active ? Theme.text : Theme.textSecondary
                }

                Text {
                    anchors.left: icon.right
                    anchors.leftMargin: Theme.dp(16)
                    anchors.right: badgePill.visible ? badgePill.left : parent.right
                    anchors.rightMargin: Theme.dp(16)
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData
                    color: ink
                    font.family: Theme.sans
                    font.weight: active ? Font.DemiBold : Font.Medium
                    font.pixelSize: Theme.dp(23)
                    elide: Text.ElideRight
                }

                Rectangle {
                    id: badgePill
                    visible: badge !== ""
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.dp(14)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.max(height, badgeText.width + Theme.dp(16))
                    height: Theme.dp(26)
                    radius: height / 2
                    color: focused ? Theme.onLight : Qt.rgba(1, 1, 1, 0.14)

                    Text {
                        id: badgeText
                        anchors.centerIn: parent
                        text: badge
                        color: Theme.text
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.pixelSize: Theme.dp(17)
                    }
                }
            }
        }
    }
}
