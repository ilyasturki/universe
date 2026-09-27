import QtQuick
import QtQuick.Window
import "core"
import "ui"

Window {
    id: overlay

    readonly property real unit: Math.max(1, height / 1080)

    color: "transparent"
    flags: Qt.FramelessWindowHint
    title: "Universe home"
    visible: false

    Loader {
        anchors.fill: parent
        source: api.theme.overlay
        focus: true
    }

    // An unlock over the game, whichever look: Home keeps the window painted (and the game's input) while it shows.
    Column {
        id: unlocks

        readonly property real s: overlay.height > 0 ? overlay.height / 1080 : 1

        anchors.top: parent.top
        anchors.topMargin: 36 * s
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: 12 * s
        z: 5

        Repeater {
            model: ListModel {
                id: shown
            }

            Rectangle {
                id: card

                width: 560 * unlocks.s
                height: 104 * unlocks.s
                radius: 20 * unlocks.s
                color: "#1b1d24"
                border.width: 1
                border.color: Qt.rgba(1, 1, 1, 0.16)
                opacity: 0

                Component.onCompleted: life.start()

                SequentialAnimation {
                    id: life
                    NumberAnimation {
                        target: card
                        property: "opacity"
                        to: 1
                        duration: 220
                        easing.type: Easing.OutCubic
                    }
                    PauseAnimation {
                        duration: 4300
                    }
                    NumberAnimation {
                        target: card
                        property: "opacity"
                        to: 0
                        duration: 300
                    }
                    ScriptAction {
                        script: shown.remove(0)
                    }
                }

                AchievementBadge {
                    id: badge
                    anchors.left: parent.left
                    anchors.leftMargin: 18 * unlocks.s
                    anchors.verticalCenter: parent.verticalCenter
                    width: 68 * unlocks.s
                    height: width
                    icon: model.icon
                }

                Column {
                    anchors.left: badge.right
                    anchors.leftMargin: 18 * unlocks.s
                    anchors.right: parent.right
                    anchors.rightMargin: 22 * unlocks.s
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 4 * unlocks.s

                    Text {
                        text: "ACHIEVEMENT UNLOCKED" + (model.rarityText !== "" ? "  ·  " + model.rarityText : "")
                        color: Qt.rgba(0.949, 0.953, 0.961, 0.6)
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.letterSpacing: 1.5 * unlocks.s
                        font.pixelSize: 15 * unlocks.s
                    }

                    Text {
                        width: parent.width
                        text: model.name
                        color: "#f2f3f5"
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.pixelSize: 26 * unlocks.s
                        elide: Text.ElideRight
                    }

                    Text {
                        width: parent.width
                        visible: text !== ""
                        text: model.description
                        color: Qt.rgba(0.949, 0.953, 0.961, 0.66)
                        font.family: Theme.sans
                        font.pixelSize: 18 * unlocks.s
                        elide: Text.ElideRight
                    }
                }
            }
        }

        Connections {
            target: api.home
            function onAchievementUnlocked(item) {
                shown.append({
                    name: item.name,
                    description: item.description,
                    icon: item.icon,
                    rarityText: item.rarityText
                });
            }
        }
    }

    Rectangle {
        id: osd

        readonly property bool silent: api.home.muted || api.home.volumePercent === 0

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 120 * overlay.unit
        width: 380 * overlay.unit
        height: 72 * overlay.unit
        radius: height / 2
        color: "#e0141418"
        opacity: api.home.osd && !api.home.open ? 1 : 0
        z: 5

        Behavior on opacity {
            NumberAnimation {
                duration: 160
            }
        }

        MenuGlyph {
            id: speaker

            anchors.left: parent.left
            anchors.leftMargin: 26 * overlay.unit
            anchors.verticalCenter: parent.verticalCenter
            width: 30 * overlay.unit
            height: width
            kind: osd.silent ? "mute" : "volume-up"
        }

        Text {
            anchors.left: speaker.right
            anchors.leftMargin: 18 * overlay.unit
            anchors.right: percent.left
            anchors.rightMargin: 12 * overlay.unit
            anchors.top: parent.top
            anchors.topMargin: 15 * overlay.unit
            text: api.home.volumeOutput
            color: "#ffffff"
            elide: Text.ElideRight
            font.pixelSize: 16 * overlay.unit
        }

        Text {
            id: percent

            anchors.right: parent.right
            anchors.rightMargin: 28 * overlay.unit
            anchors.top: parent.top
            anchors.topMargin: 15 * overlay.unit
            text: osd.silent ? "Muted" : api.home.volumePercent + "%"
            color: "#b8ffffff"
            font.pixelSize: 16 * overlay.unit
        }

        Rectangle {
            anchors.left: speaker.right
            anchors.leftMargin: 18 * overlay.unit
            anchors.right: parent.right
            anchors.rightMargin: 28 * overlay.unit
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 18 * overlay.unit
            height: 6 * overlay.unit
            radius: height / 2
            color: "#40ffffff"

            Rectangle {
                width: parent.width * (osd.silent ? 0 : Math.min(api.home.volumePercent, 100) / 100)
                height: parent.height
                radius: parent.radius
                color: "#ffffff"

                Behavior on width {
                    NumberAnimation {
                        duration: 120
                    }
                }
            }
        }
    }

    Rectangle {
        id: flash

        anchors.fill: parent
        color: "white"
        opacity: 0.0
        z: 10

        SequentialAnimation {
            id: flashAnim
            NumberAnimation {
                target: flash
                property: "opacity"
                to: 0.85
                duration: 40
            }
            NumberAnimation {
                target: flash
                property: "opacity"
                to: 0.0
                duration: 320
                easing.type: Easing.OutQuad
            }
        }

        Connections {
            target: api.home
            function onScreenshotTaken(path) {
                if (path)
                    flashAnim.restart();
            }
        }
    }
}
