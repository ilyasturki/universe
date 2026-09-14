import QtQuick
import Qt5Compat.GraphicalEffects
import "../core"

Item {
    id: root

    property var game: null
    property bool selected: false
    property url artSource: game ? game.assets.boxFront : ""
    property real cornerRadius: Theme.dp(Theme.radiusCover)
    property real selectedScale: 1.05
    property real idleScale: 1.0
    property bool showHeart: true
    // The session's game: a PLAYING mark over the art.
    property bool playing: false
    property int focusOrigin: Item.Center
    property int focusDuration: Theme.durBase
    property real ringOpacity: 1.0

    readonly property bool artMissing: String(artSource) === "" || cover.status === Image.Error
    // The software scenegraph (offscreen tests) drops every ShaderEffect: corners go square there.
    readonly property bool software: GraphicsInfo.api === GraphicsInfo.Software

    // Only the selected card paints a ring, so only it needs to animate.
    Behavior on ringOpacity {
        enabled: root.selected
        NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
    }

    Item {
        id: body

        anchors.fill: parent
        transformOrigin: root.focusOrigin
        opacity: root.selected ? 1.0 : Theme.idleOpacity
        scale: root.selected ? root.selectedScale : root.idleScale

        Behavior on opacity {
            NumberAnimation { duration: root.focusDuration; easing.type: Easing.OutQuint }
        }
        Behavior on scale {
            NumberAnimation { duration: root.focusDuration; easing.type: Easing.OutQuint }
        }

        // Behind the artwork: RectangularGlow paints its whole bounds, not just the halo.
        Loader {
            anchors.fill: parent
            active: root.selected
            opacity: root.ringOpacity
            sourceComponent: RectangularGlow {
                y: Theme.dp(18)
                glowRadius: Theme.dp(22)
                color: Qt.rgba(0, 0, 0, 0.55)
                cornerRadius: root.cornerRadius + glowRadius
            }
        }

        // clip:true would square the corners off — it ignores radius.
        Item {
            id: art

            anchors.fill: parent
            layer.enabled: true
            layer.smooth: true
            layer.effect: root.software ? null : maskEffect

            Component {
                id: maskEffect
                OpacityMask { maskSource: artMask }
            }

            Rectangle {
                anchors.fill: parent
                color: Theme.cardBase
            }

            Image {
                id: cover

                anchors.fill: parent
                source: root.artSource
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                mipmap: true
            }

            Loader {
                anchors.fill: parent
                active: root.artMissing

                sourceComponent: Rectangle {
                    gradient: Gradient {
                        GradientStop { position: 0.0; color: Qt.lighter(Theme.cardBase, 1.5) }
                        GradientStop { position: 1.0; color: Theme.cardBase }
                    }

                    Text {
                        anchors.centerIn: parent
                        width: parent.width - Theme.dp(24) * 2
                        text: root.game ? root.game.title : ""
                        color: Theme.text
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.pixelSize: Math.max(Theme.dp(13), parent.width * 0.1)
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        maximumLineCount: 5
                        elide: Text.ElideRight
                    }
                }
            }
        }

        Rectangle {
            id: artMask

            anchors.fill: art
            radius: root.cornerRadius
            color: "white"
            antialiasing: true
            visible: false
        }

        Loader {
            active: root.playing
            anchors.left: parent.left
            anchors.bottom: parent.bottom
            anchors.margins: Theme.dp(12)

            sourceComponent: Rectangle {
                width: mark.width + Theme.dp(26)
                height: Theme.dp(30)
                radius: height / 2
                color: Qt.rgba(0.02, 0.02, 0.03, 0.72)

                Row {
                    id: mark
                    anchors.centerIn: parent
                    spacing: Theme.dp(8)

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(8)
                        height: width
                        radius: width / 2
                        color: "#5fd48a"

                        SequentialAnimation on opacity {
                            running: root.playing
                            loops: Animation.Infinite
                            NumberAnimation { to: 0.3; duration: 900; easing.type: Easing.InOutQuad }
                            NumberAnimation { to: 1.0; duration: 900; easing.type: Easing.InOutQuad }
                        }
                    }

                    CapsLabel {
                        anchors.verticalCenter: parent.verticalCenter
                        text: "PLAYING"
                        color: Theme.text
                        size: Theme.dp(13)
                    }
                }
            }
        }

        Loader {
            active: root.showHeart && root.game !== null && root.game.favorite
            width: Theme.dp(24)
            height: Theme.dp(24)
            anchors.top: parent.top
            anchors.right: parent.right
            anchors.margins: Theme.dp(12)

            sourceComponent: Canvas {
                onPaint: {
                    var ctx = getContext("2d");
                    ctx.reset();
                    var s = width / 24;
                    ctx.fillStyle = "#ffffff";
                    ctx.strokeStyle = Qt.rgba(0, 0, 0, 0.45);
                    ctx.lineWidth = 1.6 * s;
                    ctx.beginPath();
                    ctx.moveTo(12 * s, 21 * s);
                    ctx.bezierCurveTo(12 * s, 21 * s, 4 * s, 16 * s, 4 * s, 10.8 * s);
                    ctx.bezierCurveTo(4 * s, 7.4 * s, 9 * s, 5.6 * s, 12 * s, 8 * s);
                    ctx.bezierCurveTo(15 * s, 5.6 * s, 20 * s, 7.4 * s, 20 * s, 10.8 * s);
                    ctx.bezierCurveTo(20 * s, 16 * s, 12 * s, 21 * s, 12 * s, 21 * s);
                    ctx.closePath();
                    ctx.fill();
                    ctx.stroke();
                }
            }
        }

        // Outside the masked item, so the halo is not cut off.
        Loader {
            anchors.fill: parent
            active: root.selected
            opacity: root.ringOpacity
            sourceComponent: FocusRing { cornerRadius: root.cornerRadius }
        }
    }
}
