import QtQuick
import "../core"

Item {
    id: root

    property real unit: 1
    readonly property bool shown: api.home.osd && !api.home.open
    readonly property bool muted: api.home.muted
    readonly property int level: Math.max(0, Math.min(100, api.home.volumePercent))

    Rectangle {
        id: card
        objectName: "osdCard"

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 120 * root.unit
        width: 560 * root.unit
        height: 100 * root.unit
        radius: height / 2
        color: "#1b1d24"
        border.width: 1
        border.color: Theme.surfaceBorder
        opacity: root.shown ? 1 : 0
        visible: opacity > 0.01

        transform: Translate {
            y: root.shown ? 0 : 14 * root.unit

            Behavior on y {
                Ease {}
            }
        }

        Behavior on opacity {
            Ease {
                duration: root.shown ? Theme.durBase : Theme.durDismiss
            }
        }

        Rectangle {
            id: badge

            anchors.left: parent.left
            anchors.leftMargin: 20 * root.unit
            anchors.verticalCenter: parent.verticalCenter
            width: 60 * root.unit
            height: width
            radius: width / 2
            color: Theme.surface

            MenuGlyph {
                objectName: "osdGlyph"

                anchors.centerIn: parent
                width: 30 * root.unit
                height: width
                kind: root.muted || root.level === 0 ? "mute" : root.level < 50 ? "volume-down" : "volume-up"
                tint: Theme.text
            }
        }

        Column {
            anchors.left: badge.right
            anchors.leftMargin: 22 * root.unit
            anchors.right: parent.right
            anchors.rightMargin: 36 * root.unit
            anchors.verticalCenter: parent.verticalCenter
            spacing: 12 * root.unit

            Item {
                width: parent.width
                height: percent.height

                Text {
                    anchors.left: parent.left
                    anchors.right: percent.left
                    anchors.rightMargin: 16 * root.unit
                    anchors.baseline: percent.baseline
                    text: api.home.volumeOutput || "Volume"
                    color: Theme.textSecondary
                    font.family: Theme.sans
                    font.weight: Font.Medium
                    font.pixelSize: 20 * root.unit
                    elide: Text.ElideRight
                }

                Text {
                    id: percent
                    objectName: "osdPercent"

                    anchors.right: parent.right
                    text: api.home.volumePercent + "%"
                    color: root.muted ? Theme.textFaint : Theme.text
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: 26 * root.unit

                    Behavior on color {
                        ColorEase {}
                    }
                }
            }

            Rectangle {
                width: parent.width
                height: 8 * root.unit
                radius: height / 2
                color: Qt.rgba(1, 1, 1, 0.14)

                Rectangle {
                    objectName: "osdFill"

                    width: parent.width * root.level / 100
                    height: parent.height
                    radius: height / 2
                    color: Theme.text
                    opacity: root.muted ? 0.32 : 1

                    Behavior on width {
                        Ease {
                            duration: Theme.durQuick
                        }
                    }
                    Behavior on opacity {
                        Ease {
                            duration: Theme.durQuick
                        }
                    }
                }
            }
        }
    }
}
