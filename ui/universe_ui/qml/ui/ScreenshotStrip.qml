import QtQuick
import "../core"

Column {
    id: strip

    property var images: []
    property int index: 0
    property bool focused: false
    property real sideMargin: 0

    readonly property real shotWidth: Theme.dp(336)
    readonly property real shotHeight: Theme.dp(189)

    spacing: Theme.dp(18)
    visible: images.length > 0

    CapsLabel {
        text: "SCREENSHOTS"
        tracking: 0.11
        color: strip.focused ? Theme.textSecondary : Theme.textMuted
    }

    ListView {
        id: list

        // Room for the focus ring's halo inside the clip on every side.
        readonly property real inset: Theme.dp(24)

        x: -inset
        width: parent.width + strip.sideMargin + inset
        height: strip.shotHeight + inset * 2
        leftMargin: inset
        orientation: ListView.Horizontal
        spacing: Theme.dp(20)
        model: strip.images
        interactive: false
        clip: true
        currentIndex: strip.index
        boundsBehavior: Flickable.StopAtBounds
        highlightRangeMode: ListView.ApplyRange
        preferredHighlightBegin: (width - strip.sideMargin - strip.shotWidth) / 2
        preferredHighlightEnd: preferredHighlightBegin + strip.shotWidth
        highlightMoveDuration: Theme.durNudge

        delegate: Item {
            width: strip.shotWidth
            height: list.height

            readonly property bool current: index === strip.index && strip.focused

            RoundedMask {
                id: shotCard
                width: strip.shotWidth
                height: strip.shotHeight
                anchors.verticalCenter: parent.verticalCenter
                radius: Theme.dp(10)
                opacity: strip.focused && !current ? 0.6 : 1.0
                scale: current ? 1.03 : 1.0

                Behavior on opacity {
                    NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                }
                Behavior on scale {
                    NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
                }

                Rectangle {
                    anchors.fill: parent
                    color: Theme.cardBase
                }

                Image {
                    anchors.fill: parent
                    source: modelData
                    sourceSize.width: 720
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                }
            }

            Loader {
                anchors.fill: shotCard
                active: current
                sourceComponent: FocusRing {
                    cornerRadius: shotCard.radius
                    gapWidth: Theme.dp(4)
                }
            }
        }
    }
}
