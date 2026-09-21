import QtQuick
import "../core"

Row {
    id: badge

    property color tint: Theme.textSecondary
    property real size: Theme.dp(22)
    property string fontFamily: Theme.sans
    property int fontWeight: Font.Medium
    property Component padGlyph: MenuGlyph {
        anchors.fill: parent
        kind: "gamepad"
        tint: parent.ink
    }

    readonly property int lowPercent: 15
    readonly property color lowTint: "#e0655a"
    property color currentTint: "#3cbc3c"

    spacing: Theme.dp(22)
    visible: api.power.count > 0

    Repeater {
        model: api.power.sources

        Row {
            id: source

            readonly property bool low: modelData.percent <= badge.lowPercent && !modelData.charging
            readonly property bool current: modelData.kind === "pad" && modelData.inputs.indexOf(api.screens.controller.current) >= 0
            readonly property color ink: low ? badge.lowTint : current ? badge.currentTint : badge.tint

            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(7)

            Item {
                anchors.verticalCenter: parent.verticalCenter
                width: badge.size * 1.15
                height: badge.size

                Loader {
                    readonly property color ink: source.ink

                    anchors.centerIn: parent
                    width: badge.size * 1.1
                    height: width
                    active: modelData.kind === "pad"
                    sourceComponent: badge.padGlyph
                }

                Item {
                    id: cell

                    anchors.centerIn: parent
                    width: badge.size * 1.05
                    height: badge.size * 0.52
                    visible: modelData.kind !== "pad"

                    Rectangle {
                        id: shell
                        anchors.fill: parent
                        anchors.rightMargin: parent.height * 0.18
                        radius: height * 0.22
                        color: "transparent"
                        border.width: Math.max(1, badge.size * 0.08)
                        border.color: source.ink
                    }

                    Rectangle {
                        anchors.left: shell.left
                        anchors.top: shell.top
                        anchors.bottom: shell.bottom
                        anchors.margins: shell.border.width * 2
                        width: Math.max(0, (shell.width - shell.border.width * 4) * modelData.percent / 100)
                        radius: shell.radius * 0.5
                        color: source.ink
                    }

                    Rectangle {
                        anchors.left: shell.right
                        anchors.verticalCenter: shell.verticalCenter
                        width: cell.height * 0.16
                        height: cell.height * 0.42
                        radius: width * 0.5
                        color: source.ink
                    }
                }

                MenuGlyph {
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.rightMargin: -badge.size * 0.16
                    anchors.bottomMargin: -badge.size * 0.1
                    width: badge.size * 0.6
                    height: width
                    visible: modelData.charging
                    kind: "bolt"
                    tint: source.ink
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: modelData.percent + "%"
                color: source.ink
                font.family: badge.fontFamily
                font.weight: badge.fontWeight
                font.pixelSize: badge.size
            }
        }
    }
}
