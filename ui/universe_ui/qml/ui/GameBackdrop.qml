import QtQuick
import "../core"

// Faded as one layer, or the gradient thins too and lets the art's edge through.
Item {
    id: root

    property var game: null

    height: Theme.dp(560)
    opacity: 0.55
    layer.enabled: true

    BackgroundStage {
        anchors.fill: parent
        game: root.game
        blurRadius: 30
        drift: false
        zoomEnabled: false
        overscan: 1.06
    }

    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop { position: 0.00; color: Qt.rgba(0.055, 0.059, 0.075, 0.30) }
            GradientStop { position: 0.45; color: Qt.rgba(0.055, 0.059, 0.075, 0.70) }
            GradientStop { position: 0.75; color: Qt.rgba(0.055, 0.059, 0.075, 0.94) }
            GradientStop { position: 1.00; color: Theme.ground }
        }
    }
}
