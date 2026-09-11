import QtQuick
import "../core"

Item {
    id: root

    property bool selected: false
    property real idleScale: 1.0
    property int count: 0
    property real cornerRadius: Theme.dp(Theme.radiusTile)
    property real ringOpacity: 1.0

    Behavior on ringOpacity {
        enabled: root.selected
        NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
    }

    Item {
        id: body

        anchors.fill: parent
        transformOrigin: Item.Bottom
        opacity: root.selected ? 1.0 : Theme.idleOpacity
        scale: root.selected ? 1.0 : root.idleScale

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
        }
        Behavior on scale {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
        }

        Rectangle {
            anchors.fill: parent
            radius: root.cornerRadius
            color: Theme.surface
            border.width: Math.max(1, Theme.dp(2))
            border.color: Theme.surfaceBorder
        }

        Column {
            anchors.centerIn: parent
            spacing: Theme.dp(16)

            Canvas {
                anchors.horizontalCenter: parent.horizontalCenter
                width: Theme.dp(56)
                height: Theme.dp(56)
                onPaint: {
                    var ctx = getContext("2d");
                    ctx.reset();
                    var s = width / 24;
                    ctx.fillStyle = Theme.text;
                    var cells = [[2, 2], [13, 2], [2, 13], [13, 13]];
                    for (var i = 0; i < cells.length; i++) {
                        ctx.beginPath();
                        ctx.roundedRect(cells[i][0] * s, cells[i][1] * s, 9 * s, 9 * s, 2.5 * s, 2.5 * s);
                        ctx.fill();
                    }
                }
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: "Library"
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(30)
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.count + (root.count === 1 ? " game" : " games")
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(22)
            }
        }

        Loader {
            anchors.fill: parent
            active: root.selected
            opacity: root.ringOpacity
            sourceComponent: FocusRing { cornerRadius: root.cornerRadius }
        }
    }
}
