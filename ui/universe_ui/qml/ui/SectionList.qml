import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: list

    // [{ name, icon, group }]: a `group` label heads the entries that share it.
    property var sections: []
    property var badges: []
    property int current: 0

    signal requested(int index)
    signal entered
    // Up on the first entry; `cancelled` is B, which a page under the tabs treats the same.
    signal escapedUp
    signal cancelled

    // Thirteen entries and four group heads fit the page under the title.
    readonly property real entryHeight: Theme.dp(46)
    readonly property real entrySpacing: Theme.dp(3)
    readonly property real groupHeight: Theme.dp(34)

    function startsGroup(index) {
        var g = sections[index].group || "";
        return g !== "" && (index === 0 || (sections[index - 1].group || "") !== g);
    }

    implicitHeight: column.height

    function step(d) {
        var n = sections.length;
        list.requested((current + d + n) % n);
        Sound.tick();
    }

    Keys.onUpPressed: function (event) {
        if (list.current === 0)
            list.escapedUp();
        else
            list.step(-1);
    }
    Keys.onDownPressed: function (event) {
        if (list.current === list.sections.length - 1)
            Sound.edge();
        else
            list.step(1);
    }
    Keys.onLeftPressed: Sound.edge()
    Keys.onRightPressed: function (event) {
        Sound.panel();
        list.entered();
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            Sound.panel();
            list.entered();
        } else if (api.keys.isFirst(event) || api.keys.isLast(event)) {
            event.accepted = true;
            var to = api.keys.isFirst(event) ? 0 : list.sections.length - 1;
            if (to === list.current) {
                Sound.edge();
            } else {
                list.requested(to);
                Sound.tick();
            }
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            Sound.cancel();
            list.cancelled();
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
                readonly property bool headed: list.startsGroup(index)

                width: parent.width
                height: list.entryHeight + (headed ? list.groupHeight : 0)

                CapsLabel {
                    x: Theme.dp(18)
                    y: Theme.dp(12)
                    visible: headed
                    text: (modelData.group || "").toUpperCase()
                    size: Theme.dp(14)
                }

                Rectangle {
                    anchors.fill: parent
                    anchors.topMargin: headed ? list.groupHeight : 0
                    radius: Theme.dp(14)
                    color: focused ? Theme.text : active ? Qt.rgba(1, 1, 1, 0.09) : "transparent"

                    Behavior on color {
                        ColorEase {}
                    }

                    Pointer {
                        current: focused
                        radius: Theme.dp(14)
                        onPicked: {
                            list.forceActiveFocus();
                            if (!active)
                                list.requested(index);
                        }
                    }
                }

                MenuGlyph {
                    id: icon
                    anchors.left: parent.left
                    anchors.leftMargin: Theme.dp(18)
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: headed ? list.groupHeight / 2 : 0
                    width: Theme.dp(22)
                    height: width
                    kind: modelData.icon
                    tint: focused ? Theme.onLight : active ? Theme.text : Theme.textSecondary
                }

                Text {
                    anchors.left: icon.right
                    anchors.leftMargin: Theme.dp(16)
                    anchors.right: badgePill.visible ? badgePill.left : parent.right
                    anchors.rightMargin: Theme.dp(16)
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: headed ? list.groupHeight / 2 : 0
                    text: modelData.name
                    color: ink
                    font.family: Theme.sans
                    font.weight: active ? Font.DemiBold : Font.Medium
                    font.pixelSize: Theme.dp(21)
                    elide: Text.ElideRight
                }

                Rectangle {
                    id: badgePill
                    visible: badge !== ""
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.dp(14)
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: headed ? list.groupHeight / 2 : 0
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
