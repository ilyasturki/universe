import QtQuick
import "../core"

// The Media Gallery's tabs.
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
        objectName: "tabBar"
        spacing: Theme.dp(8)

        Repeater {
            model: tabs.labels

            TabLabel {
                text: modelData
                current: index === tabs.current
                focused: tabs.active && index === tabs.index
                weight: Font.Normal
                boxHeight: tabs.implicitHeight
                onPicked: tabs.pointed(index)
            }
        }
    }
}
