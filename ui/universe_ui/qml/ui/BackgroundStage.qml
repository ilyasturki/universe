import QtQuick
import Qt5Compat.GraphicalEffects
import "../core"

Item {
    id: root

    property var game: null
    readonly property var art: {
        if (!game)
            return { source: "", cropped: false };
        if (String(game.assets.background) !== "")
            return { source: game.assets.background, cropped: false };
        var shots = game.assets.screenshotList;
        if (shots && shots.length > 0)
            return { source: shots[0], cropped: false };
        if (String(game.assets.banner) !== "")
            return { source: game.assets.banner, cropped: false };
        return { source: game.assets.boxFront, cropped: true };
    }

    // 0 paints the art directly; anything above routes it through one blur pass.
    property real blurRadius: 18
    property bool drift: true
    property bool zoomEnabled: true
    property real overscan: 1.2

    readonly property real zoomLow: zoomEnabled ? 1.02 : 1.0
    readonly property real zoomHigh: 1.10

    // At the shallowest zoom the overscan still has to cover the excursion, or the drift walks an edge into frame.
    readonly property real driftRangeX: Math.min(Theme.dp(48), Math.max(0, (overscan * zoomLow - 1) * width / 2))
    readonly property real driftRangeY: zoomEnabled
        ? Math.min(Theme.dp(28), Math.max(0, (overscan * zoomLow - 1) * height / 2))
        : 0

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    // The drift and zoom move the blurred result, so the blur itself runs once per crossfade, not per frame.
    Item {
        id: motion
        anchors.centerIn: parent
        width: parent.width * root.overscan
        height: parent.height * root.overscan
        scale: root.zoomLow

        transform: Translate {
            id: bgTranslate
            x: -root.driftRangeX
            y: -root.driftRangeY
        }

        CrossfadeImage {
            id: still
            anchors.fill: parent
            source: root.art.source
            mipmap: true
        }

        Loader {
            anchors.fill: parent
            active: root.blurRadius > 0 && !Theme.software

            sourceComponent: Item {
                ShaderEffectSource {
                    id: texture
                    anchors.fill: parent
                    sourceItem: still
                    live: true
                    hideSource: true
                }

                FastBlur {
                    anchors.fill: parent
                    source: texture
                    // FastBlur caps near 64, short of hiding a cover's lettering; dark finishes it.
                    radius: Math.min(64, Theme.dp(root.blurRadius))
                    transparentBorder: true
                }
            }
        }

        // Three loops on coprime-ish periods, so the pair never resynchronises; InOutQuad, or the turnaround stutters.
        SequentialAnimation {
            running: root.drift && root.zoomEnabled
            // Off-screen it stops ticking; paused rather than stopped keeps the phase.
            paused: running && !root.visible
            loops: Animation.Infinite

            NumberAnimation {
                target: motion; property: "scale"
                from: root.zoomLow; to: root.zoomHigh
                duration: Theme.durZoom; easing.type: Easing.InOutQuad
            }
            NumberAnimation {
                target: motion; property: "scale"
                from: root.zoomHigh; to: root.zoomLow
                duration: Theme.durZoom; easing.type: Easing.InOutQuad
            }
        }

        SequentialAnimation {
            running: root.drift
            paused: running && !root.visible
            loops: Animation.Infinite

            NumberAnimation {
                target: bgTranslate; property: "x"
                from: -root.driftRangeX; to: root.driftRangeX
                duration: Theme.durDriftX; easing.type: Easing.InOutQuad
            }
            NumberAnimation {
                target: bgTranslate; property: "x"
                from: root.driftRangeX; to: -root.driftRangeX
                duration: Theme.durDriftX; easing.type: Easing.InOutQuad
            }
        }

        SequentialAnimation {
            running: root.drift && root.zoomEnabled
            paused: running && !root.visible
            loops: Animation.Infinite

            NumberAnimation {
                target: bgTranslate; property: "y"
                from: -root.driftRangeY; to: root.driftRangeY
                duration: Theme.durDriftY; easing.type: Easing.InOutQuad
            }
            NumberAnimation {
                target: bgTranslate; property: "y"
                from: root.driftRangeY; to: -root.driftRangeY
                duration: Theme.durDriftY; easing.type: Easing.InOutQuad
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        visible: root.art.cropped
        color: Qt.rgba(0.055, 0.059, 0.075, 0.45)
    }
}
