import QtQuick
import Qt5Compat.GraphicalEffects
import "../core"

Item {
    id: frame

    property var game: null
    readonly property url heroSource: !game ? "" : String(game.assets.background) !== "" ? game.assets.background : game.assets.banner
    readonly property url boxSource: game ? game.assets.boxFront : ""
    readonly property url logoSource: game ? game.assets.logo : ""

    property alias artScale: art.scale

    readonly property bool heroMissing: String(heroSource) === "" || hero.status === Image.Error

    readonly property real boxWidth: Theme.dp(480)
    readonly property real boxHeight: Theme.dp(720)
    readonly property real logoWidth: Theme.dp(480)
    readonly property real logoHeight: Theme.dp(150)
    readonly property real captionBottom: Theme.dp(72)

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    Item {
        id: art

        anchors.fill: parent

        Item {
            id: fillSource

            anchors.fill: parent

            Image {
                anchors.fill: parent
                source: hero.status === Image.Ready ? frame.heroSource : (frame.heroMissing ? frame.boxSource : "")
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: true
                mipmap: true
            }
        }

        Loader {
            anchors.fill: parent
            active: !Theme.software

            sourceComponent: Item {
                ShaderEffectSource {
                    id: fillTexture
                    anchors.fill: parent
                    sourceItem: fillSource
                    live: true
                    hideSource: true
                }

                FastBlur {
                    anchors.fill: parent
                    source: fillTexture
                    radius: Math.min(64, Theme.dp(40))
                }
            }
        }

        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0.055, 0.059, 0.075, 0.42)
        }

        // A hard edge on the 3:1 hero crawls pixel by pixel under the launch's scale; feathered instead.
        Item {
            id: heroBand

            anchors.fill: parent
            visible: hero.status === Image.Ready

            readonly property real bandHeight: hero.paintedHeight
            readonly property bool letterboxed: bandHeight > 0 && bandHeight < height - 1

            layer.enabled: letterboxed && !Theme.software
            layer.smooth: true
            layer.effect: OpacityMask {
                maskSource: heroMask
            }

            Image {
                id: hero

                anchors.fill: parent
                source: frame.heroSource
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                cache: true
                mipmap: true
            }
        }

        Rectangle {
            id: heroMask

            // OpacityMask stretches the mask over the whole layer.
            readonly property real feather: 0.18
            readonly property real band: heroBand.height > 0 ? heroBand.bandHeight / heroBand.height : 1.0
            readonly property real edge: (1.0 - band) / 2
            readonly property real fade: feather * band

            anchors.fill: parent
            visible: false
            gradient: Gradient {
                GradientStop {
                    position: heroMask.edge
                    color: Qt.rgba(1, 1, 1, 0)
                }
                GradientStop {
                    position: heroMask.edge + heroMask.fade
                    color: "white"
                }
                GradientStop {
                    position: 1.0 - heroMask.edge - heroMask.fade
                    color: "white"
                }
                GradientStop {
                    position: 1.0 - heroMask.edge
                    color: Qt.rgba(1, 1, 1, 0)
                }
            }
        }

        RoundedMask {
            x: (parent.width - frame.boxWidth) / 2
            y: (parent.height - frame.boxHeight) / 2
            width: frame.boxWidth
            height: frame.boxHeight
            radius: Theme.dp(Theme.radiusCover)
            visible: frame.heroMissing && box.status === Image.Ready

            Image {
                id: box

                anchors.fill: parent
                source: frame.boxSource
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                cache: true
                mipmap: true
            }
        }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop {
                    position: 0.55
                    color: Qt.rgba(0.055, 0.059, 0.075, 0.00)
                }
                GradientStop {
                    position: 1.00
                    color: Qt.rgba(0.055, 0.059, 0.075, 0.62)
                }
            }
        }

        Image {
            id: logo

            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(Theme.edgeMargin)
            anchors.bottom: parent.bottom
            anchors.bottomMargin: frame.captionBottom
            width: frame.logoWidth
            height: frame.logoHeight
            source: frame.logoSource
            fillMode: Image.PreserveAspectFit
            horizontalAlignment: Image.AlignLeft
            verticalAlignment: Image.AlignBottom
            asynchronous: true
            // Must match HeroLogo's: sourceSize is part of the cache key.
            sourceSize.width: frame.logoWidth
            sourceSize.height: frame.logoHeight
            visible: status === Image.Ready
        }

        Text {
            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(Theme.edgeMargin)
            anchors.bottom: parent.bottom
            anchors.bottomMargin: frame.captionBottom
            width: Theme.dp(1000)
            visible: String(frame.logoSource) === "" || logo.status === Image.Error
            text: frame.game ? frame.game.title : ""
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(58)
            elide: Text.ElideRight
        }
    }
}
