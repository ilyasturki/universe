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
        return { source: game.assets.boxFront, cropped: true };
    }

    property url source: art.source
    // 0 paints the art directly; anything above routes it through one blur pass.
    property real blurRadius: 18
    // FastBlur caps near 64, short of hiding a cover's lettering; dark finishes it.
    property real darkAmount: art.cropped ? 0.45 : 0.0
    property bool ambientEnabled: true
    property bool zoomEnabled: true
    // How far the art is oversized past the frame, giving the drift room to run.
    property real overscan: 1.2

    readonly property real zoomLow: zoomEnabled ? 1.02 : 1.0
    readonly property real zoomHigh: 1.10

    // At the shallowest zoom the overscan still has to cover the excursion, or
    // the drift walks an edge into frame.
    readonly property real driftRangeX: Math.min(Theme.dp(48), Math.max(0, (overscan * zoomLow - 1) * width / 2))
    readonly property real driftRangeY: zoomEnabled
        ? Math.min(Theme.dp(28), Math.max(0, (overscan * zoomLow - 1) * height / 2))
        : 0

    // Which of the two layers is currently the visible one.
    property bool showA: true
    // The software scenegraph (offscreen tests) cannot blur; the art then shows as is.
    readonly property bool software: GraphicsInfo.api === GraphicsInfo.Software
    property url pending

    onSourceChanged: {
        if (source == "") {
            pending = "";
            a.source = "";
            b.source = "";
            return;
        }
        pending = source;
        var incoming = showA ? b : a;
        incoming.source = source;
        // Going back to a game the hidden layer still holds changes nothing,
        // so no statusChanged arrives to commit the crossfade.
        if (incoming.status === Image.Ready)
            _commit(incoming);
    }

    function _commit(layer) {
        if (layer.source != pending || pending == "")
            return;
        // The incoming layer is the hidden one; flip only once it has decoded,
        // so a slow disk never shows a half-painted crossfade.
        if ((showA && layer === b) || (!showA && layer === a))
            showA = !showA;
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    Item {
        id: motionSource
        anchors.fill: parent

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

            Image {
                id: a
                anchors.fill: parent
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: true
                smooth: true
                mipmap: true
                opacity: root.showA ? 1.0 : 0.0
                onStatusChanged: if (status === Image.Ready) root._commit(a)

                Behavior on opacity {
                    NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic }
                }
            }

            Image {
                id: b
                anchors.fill: parent
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: true
                smooth: true
                mipmap: true
                opacity: root.showA ? 0.0 : 1.0
                onStatusChanged: if (status === Image.Ready) root._commit(b)

                Behavior on opacity {
                    NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic }
                }
            }

            // Three loops on coprime-ish periods, so the pair never resynchronises
            // into an obvious cycle. InOutQuad, because an eased-out leg stops dead
            // at the turnaround and reads as a stutter.
            SequentialAnimation {
                running: root.ambientEnabled && root.zoomEnabled
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
                running: root.ambientEnabled
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
                running: root.ambientEnabled && root.zoomEnabled
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
    }

    // One texture, one blur, for the whole backdrop — never one per delegate.
    Loader {
        id: blur
        anchors.fill: parent
        active: root.blurRadius > 0 && !root.software

        sourceComponent: Item {
            ShaderEffectSource {
                id: motionTexture
                anchors.fill: parent
                sourceItem: motionSource
                live: true
                hideSource: true
            }

            FastBlur {
                anchors.fill: parent
                source: motionTexture
                radius: Math.min(64, Theme.dp(root.blurRadius))
                transparentBorder: true
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        visible: root.darkAmount > 0
        color: Qt.rgba(0.055, 0.059, 0.075, root.darkAmount)
    }
}
