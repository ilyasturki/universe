import QtQuick
import QtQuick.Window
import "core"
import "ui"

Window {
    id: overlay

    readonly property real unit: Math.max(0.66, height / 1080)

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
        objectName: "unlocks"

        readonly property real s: overlay.height > 0 ? overlay.height / 1080 : 1

        anchors.top: parent.top
        anchors.topMargin: 36 * s
        anchors.right: parent.right
        anchors.rightMargin: 36 * s
        spacing: 12 * s
        z: 5

        Repeater {
            objectName: "unlockCards"

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
                    // The fades and a margin: the whole card sits inside the overlay's lift.
                    PauseAnimation {
                        duration: api.home.bannerMs - 700
                    }
                    NumberAnimation {
                        target: card
                        property: "opacity"
                        to: 0
                        duration: 300
                    }
                    ScriptAction {
                        // Removing the card ends this script: the word goes first.
                        script: {
                            api.home.bannerDone();
                            shown.remove(0);
                        }
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
                        text: api.theme.unlocked + (model.rarityText !== "" ? "  ·  " + model.rarityText : "")
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

        Rectangle {
            anchors.right: parent.right
            visible: api.home.bannersWaiting > 0
            width: moreText.implicitWidth + 44 * unlocks.s
            height: 48 * unlocks.s
            radius: height / 2
            color: "#1b1d24"
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.16)

            Text {
                id: moreText
                anchors.centerIn: parent
                text: "+" + api.home.bannersWaiting + " more unlocked"
                color: Qt.rgba(0.949, 0.953, 0.961, 0.8)
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: 18 * unlocks.s
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

    // The look's own volume level, at the window's scale: Theme.dp follows only Reprise's root.
    Loader {
        id: osd
        objectName: "volumeOsd"

        anchors.fill: parent
        source: api.theme.osd
        z: 5
        onLoaded: item.unit = Qt.binding(() => overlay.unit)
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
