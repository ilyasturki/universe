import QtQuick
import "../core"
import "../../ui" as Base

// The home's top: Games and Media at the left, search, settings and power at the right, the clock last.
Item {
    id: bar

    property int tab: 0
    property bool active: false
    // 0 and 1 the tabs, then the icons.
    property int index: 0
    readonly property var icons: [
        {
            kind: "search",
            label: "Search"
        },
        {
            kind: "settings",
            label: "Settings"
        },
        {
            kind: "power",
            label: "Power"
        }
    ]
    readonly property int count: 2 + icons.length

    signal pointed(int index)

    height: Theme.dp(126)

    Row {
        id: tabs
        x: Theme.dp(Theme.tabX - 18)
        y: Theme.dp(Theme.barY) - height / 2
        spacing: Theme.dp(82 - 36)

        Repeater {
            model: ["Games", "Media"]

            TabLabel {
                text: modelData
                current: index === bar.tab
                focused: bar.active && index === bar.index
                weight: current ? Font.DemiBold : Font.Light
                fontSize: Theme.dp(Theme.fontTab)
                pad: Theme.dp(18)
                boxHeight: Theme.dp(64)
                onPicked: bar.pointed(index)
            }
        }
    }

    Label {
        id: clock
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(87)
        y: Theme.dp(Theme.barY) - height / 2
        text: Theme.clock
        font.weight: Font.Light
        font.pixelSize: Theme.dp(Theme.fontClock)
    }

    Base.PowerBadge {
        id: badge
        anchors.right: clock.left
        anchors.rightMargin: Theme.dp(34)
        anchors.verticalCenter: clock.verticalCenter
        tint: Qt.rgba(1, 1, 1, 0.85)
        size: Theme.dp(26)
        fontFamily: Theme.sans
        fontWeight: Font.Normal
    }

    // The icons stand left of the battery and the clock, 105 apart as the console's.
    readonly property real iconsRight: badge.width > 0 ? badge.x - Theme.dp(40) : clock.x - Theme.dp(60)

    Repeater {
        model: bar.icons

        Item {
            id: icon

            readonly property bool focused: bar.active && bar.index === index + 2

            x: bar.iconsRight - Theme.dp((bar.icons.length - 1 - index) * 105) - width
            y: Theme.dp(Theme.barY) - height / 2
            width: Theme.dp(66)
            height: width

            Rectangle {
                anchors.fill: parent
                radius: width / 2
                color: "#ffffff"
                opacity: icon.focused ? 1.0 : 0.0
                scale: icon.focused ? 1.0 : 0.8

                Behavior on opacity {
                    Ease {}
                }
                Behavior on scale {
                    Ease {}
                }
            }

            Rectangle {
                anchors.fill: parent
                anchors.margins: -Theme.dp(6)
                radius: width / 2
                color: "transparent"
                border.width: Theme.dp(2)
                border.color: Qt.rgba(1, 1, 1, 0.42)
                opacity: icon.focused ? 1.0 : 0.0

                Behavior on opacity {
                    Ease {}
                }
            }

            Glyph {
                anchors.centerIn: parent
                width: Theme.dp(38)
                height: width
                kind: modelData.kind
                tint: icon.focused ? Theme.onLight : Theme.text
            }

            Label {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.top: parent.bottom
                anchors.topMargin: Theme.dp(10)
                text: modelData.label
                opacity: icon.focused ? 1.0 : 0.0
                font.pixelSize: Theme.dp(Theme.fontSmall)

                Behavior on opacity {
                    Ease {}
                }
            }

            Touch {
                current: icon.focused
                onPicked: bar.pointed(index + 2)
            }
        }
    }
}
