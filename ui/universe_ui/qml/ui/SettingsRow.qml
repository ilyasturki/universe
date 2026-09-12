import QtQuick
import "../core"

// One setting inside a card: its label, and by type a switch, a value with a chevron,
// or a check's detail and status dot. Focused, it is the one white row on the screen.
Item {
    id: row

    property var entry: ({})
    property bool focused: false
    property bool compact: false
    // Hidden next to the focused row, where the white pill already draws the edge.
    property bool separator: false

    readonly property bool info: entry.type === "info"
    readonly property color onFocus: Qt.rgba(0.063, 0.067, 0.086, 0.7)

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Theme.dp(18)
        anchors.rightMargin: Theme.dp(16)
        anchors.top: parent.top
        height: 1
        visible: row.separator
        color: Qt.rgba(1, 1, 1, 0.06)
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(14)
        color: row.focused ? Theme.text : "transparent"

        Behavior on color {
            ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(18)
        anchors.right: control.left
        anchors.rightMargin: Theme.dp(20)
        anchors.verticalCenter: parent.verticalCenter
        text: row.entry.label || ""
        color: row.focused ? Theme.onLight : Theme.text
        font.family: Theme.sans
        font.weight: row.focused ? Font.DemiBold : Font.Medium
        font.pixelSize: Theme.dp(row.compact ? 22 : 23)
        elide: Text.ElideRight
    }

    Item {
        id: control

        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(16)
        anchors.verticalCenter: parent.verticalCenter
        width: childrenRect.width
        height: parent.height

        SettingsToggle {
            visible: row.entry.type === "bool"
            anchors.verticalCenter: parent.verticalCenter
            on: row.entry.value === true
            focused: row.focused
        }

        // enum, string, path, int, action: an inherited tag, the value and a chevron
        Row {
            visible: row.entry.type !== "bool" && !row.info
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(16)

            Rectangle {
                visible: row.entry.inherited === true
                anchors.verticalCenter: parent.verticalCenter
                width: tag.width + Theme.dp(18)
                height: tag.height + Theme.dp(8)
                radius: Theme.dp(8)
                color: "transparent"
                border.width: 1
                border.color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.25) : Qt.rgba(1, 1, 1, 0.14)

                CapsLabel {
                    id: tag
                    anchors.centerIn: parent
                    text: "INHERITED"
                    size: Theme.dp(15)
                    tracking: 0.08
                    color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Theme.textFaint
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: text !== ""
                text: row.entry.display || ""
                color: row.focused ? row.onFocus : Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                elide: Text.ElideMiddle
                width: Math.min(implicitWidth, Theme.dp(460))
            }

            Canvas {
                width: Theme.dp(14)
                height: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                readonly property color tint: row.focused ? Theme.onLight : Theme.textMuted
                onTintChanged: requestPaint()
                onPaint: {
                    var ctx = getContext("2d");
                    ctx.reset();
                    ctx.strokeStyle = tint;
                    ctx.lineWidth = Theme.dp(2.5);
                    ctx.lineCap = "round";
                    ctx.lineJoin = "round";
                    ctx.beginPath();
                    ctx.moveTo(width * 0.2, height * 0.15);
                    ctx.lineTo(width * 0.8, height * 0.5);
                    ctx.lineTo(width * 0.2, height * 0.85);
                    ctx.stroke();
                }
            }
        }

        // info: the detail and a status dot
        Row {
            visible: row.info
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(14)

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: row.entry.detail || ""
                color: row.focused ? row.onFocus : Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(20)
                elide: Text.ElideMiddle
                width: Math.min(implicitWidth, Theme.dp(520))
            }

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(14)
                height: width
                radius: width / 2
                color: row.entry.value ? "#5fd48a" : "#e0655a"
            }
        }
    }
}
