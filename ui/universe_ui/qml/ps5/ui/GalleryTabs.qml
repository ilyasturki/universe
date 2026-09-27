import QtQuick
import "../core"

// The Media Gallery's tabs: the one shown boxed as the console's "All", the focused one ringed.
Item {
    id: tabs

    property var labels: []
    property int current: 0
    property bool active: false
    property int index: 0

    signal pointed(int index)

    implicitWidth: row.implicitWidth
    implicitHeight: Theme.dp(82)

    Row {
        id: row
        spacing: Theme.dp(8)

        Repeater {
            model: tabs.labels

            Item {
                id: tab

                readonly property bool shown: index === tabs.current
                readonly property bool focused: tabs.active && index === tabs.index

                width: label.implicitWidth + Theme.dp(56)
                height: tabs.implicitHeight

                Rectangle {
                    anchors.fill: parent
                    radius: Theme.dp(Theme.radiusRow)
                    color: Qt.rgba(1, 1, 1, tab.shown ? 0.16 : 0)
                    border.width: tab.shown ? Theme.dp(2) : 0
                    border.color: Qt.rgba(1, 1, 1, 0.42)

                    Behavior on color {
                        ColorAnimation {
                            duration: Theme.durTab
                        }
                    }
                }

                Label {
                    id: label
                    anchors.centerIn: parent
                    text: modelData
                    color: tab.shown || tab.focused ? Theme.text : Theme.textSecondary
                    font.pixelSize: Theme.dp(Theme.fontTitle)
                }

                FocusFrame {
                    target: tab
                    shown: tab.focused
                    radius: Theme.dp(Theme.radiusRow)
                    gap: Theme.dp(3)
                }

                Touch {
                    current: tab.focused
                    onPicked: tabs.pointed(index)
                }
            }
        }
    }
}
