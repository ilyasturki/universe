import QtQuick
import QtMultimedia

Item {
    id: boot

    objectName: "boot"

    // The software scenegraph draws no perspective: the mark only fades.
    readonly property bool plain: GraphicsInfo.api === GraphicsInfo.Software
    // One unit of the 128-unit mark.
    readonly property real u: Math.min(width / 1920, height / 1080) * 2.4
    readonly property bool light: Qt.color(api.theme.ground).hslLightness > 0.5
    property real ring: 0
    property real planet: 0
    property real word: 0

    visible: api.boot.running

    function play() {
        if (!api.boot.running || intro.running || leave.running)
            return;
        intro.start();
        if (!api.home.muted)
            chime.play();
    }

    function skip() {
        intro.stop();
        leave.start();
    }

    Connections {
        target: api.boot
        function onStarted() {
            boot.play();
        }
        function onSkipped() {
            boot.skip();
        }
    }

    Rectangle {
        anchors.fill: parent
        color: api.theme.ground
    }

    // The stacked lockup's geometry (brand/universe-lockup-stacked-dark.svg), in the mark's units.
    Item {
        id: lockup

        width: 186.8 * boot.u
        height: 118.23 * boot.u
        anchors.centerIn: parent
        scale: 1 + 0.04 * (1 - boot.opacity)

        Layer {
            source: "../assets/brand/ring-back.svg"
        }

        Image {
            x: 29.4 * boot.u
            y: -29 * boot.u + (boot.plain ? 0 : (1 - boot.planet) * 22 * boot.u)
            width: 128 * boot.u
            height: width
            sourceSize: Qt.size(width, height)
            source: "../assets/brand/planet.svg"
            scale: boot.plain ? 1 : 0.6 + 0.4 * boot.planet
            opacity: Math.max(0, Math.min(1, boot.plain ? boot.ring : boot.planet))
        }

        Layer {
            source: "../assets/brand/ring-front.svg"
        }

        Image {
            y: 96.2 * boot.u + (boot.plain ? 0 : (1 - boot.word) * 10 * boot.u)
            width: parent.width
            height: 21.73 * boot.u
            sourceSize: Qt.size(width, height)
            source: boot.light ? "../assets/brand/wordmark.svg" : "../assets/brand/wordmark-dark.svg"
            opacity: boot.plain ? boot.ring : boot.word
        }
    }

    component Layer: Image {
        x: 29.4 * boot.u
        y: -29 * boot.u
        width: 128 * boot.u
        height: width
        sourceSize: Qt.size(width, height)
        smooth: true
        transform: [
            Scale {
                origin.x: 64 * boot.u
                origin.y: 64 * boot.u
                xScale: boot.plain ? 1 : 0.8 + 0.2 * boot.ring
                yScale: xScale
            },
            Rotation {
                origin.x: 64 * boot.u
                origin.y: 64 * boot.u
                axis {
                    x: 1
                    y: 0
                    z: 0
                }
                angle: boot.plain ? 0 : -78 * (1 - boot.ring)
            }
        ]
        opacity: boot.plain ? boot.ring : Math.min(1, boot.ring * 2.5)
    }

    SoundEffect {
        id: chime
        source: api.theme.soundFiles.boot || Qt.resolvedUrl("../" + api.theme.entry.replace(/[^\/]*$/, "") + "assets/sounds/boot.wav")
    }

    SequentialAnimation {
        id: intro

        ParallelAnimation {
            NumberAnimation {
                target: boot
                property: "ring"
                to: 1
                duration: 600
                easing.type: Easing.OutCubic
            }
            SequentialAnimation {
                PauseAnimation {
                    duration: 280
                }
                NumberAnimation {
                    target: boot
                    property: "planet"
                    to: 1
                    duration: 520
                    easing.type: Easing.OutBack
                }
            }
            SequentialAnimation {
                PauseAnimation {
                    duration: 620
                }
                NumberAnimation {
                    target: boot
                    property: "word"
                    to: 1
                    duration: 420
                    easing.type: Easing.OutCubic
                }
            }
        }
        PauseAnimation {
            duration: 160
        }
        ScriptAction {
            script: api.boot.land()
        }
        NumberAnimation {
            target: boot
            property: "opacity"
            to: 0
            duration: 380
            easing.type: Easing.InOutQuad
        }
        ScriptAction {
            script: api.boot.finish()
        }
    }

    SequentialAnimation {
        id: leave

        ParallelAnimation {
            NumberAnimation {
                target: boot
                property: "opacity"
                to: 0
                duration: 180
                easing.type: Easing.OutQuad
            }
            NumberAnimation {
                target: chime
                property: "volume"
                to: 0
                duration: 180
            }
        }
        ScriptAction {
            script: {
                chime.stop();
                api.boot.finish();
            }
        }
    }
}
