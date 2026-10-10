import QtQuick
import QtQuick.Shapes
import QtMultimedia

Item {
    id: boot

    objectName: "boot"

    // The software scenegraph draws no ShaderEffect: the hairlines are left out, the rest plays.
    readonly property bool plain: GraphicsInfo.api === GraphicsInfo.Software
    readonly property real dp: Math.min(width / 1920, height / 1080)
    // One unit of the 128-unit mark.
    readonly property real u: dp * 2.4
    readonly property bool light: Qt.color(api.theme.ground).hslLightness > 0.5
    readonly property color ink: light ? "#101116" : "#f2f3f5"
    readonly property color accent: contrast(api.theme.accent, api.theme.ground) >= 2.2 ? api.theme.accent : ink
    // ms since the first frame: every part below is a function of it.
    property real t: 0
    // Before the first tick each part is drawn once, all but transparent, so none builds its pipeline or uploads its texture mid-intro.
    readonly property bool primer: t === 0

    readonly property int brake: 900
    readonly property int landAt: 1350
    readonly property int end: 1900

    readonly property real field: outQuad(seg(0, 260)) * (1 - outQuad(seg(960, 240)))
    readonly property real out: inOutQuad(seg(1420, 320))
    readonly property real echo: outCubic(seg(920, 560))
    readonly property real opening: outQuad(seg(landAt, 520))
    readonly property real hole: opening * (Math.sqrt(width * width + height * height) / 2 + 40 * dp)

    visible: api.boot.running

    function seg(start, duration) {
        return Math.max(0, Math.min(1, (t - start) / duration));
    }

    function outQuad(k) {
        return k * (2 - k);
    }

    function outCubic(k) {
        return Math.pow(k - 1, 3) + 1;
    }

    function inOutQuad(k) {
        return k < 0.5 ? 2 * k * k : 1 - 2 * (1 - k) * (1 - k);
    }

    function outBack(k) {
        return Math.pow(k - 1, 2) * (2.70158 * (k - 1) + 1.70158) + 1;
    }

    // The warp's speed (0 to 1): a cubic ramp to the brake, then an exponential stop.
    function speed(at) {
        if (at < brake)
            return 0.04 + 0.96 * Math.pow(Math.max(at, 0) / brake, 3);
        return Math.pow(2, -10 * (at - brake) / 90);
    }

    // The integral of speed, in closed form.
    function travel(at) {
        at = Math.max(at, 0);
        var x = Math.min(at, brake) / brake;
        var z = 0.04 * Math.min(at, brake) + 0.96 * brake * Math.pow(x, 4) / 4;
        if (at > brake)
            z += 90 * (1 - Math.pow(2, -10 * (at - brake) / 90)) / (10 * Math.LN2);
        return 2.4 * z / 1000;
    }

    function luminance(c) {
        function linear(v) {
            return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
        }
        return 0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b);
    }

    function contrast(a, b) {
        var x = luminance(Qt.color(a)), y = luminance(Qt.color(b));
        return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
    }

    function inked(alpha) {
        return Qt.rgba(ink.r, ink.g, ink.b, alpha);
    }

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

    Component.onCompleted: {
        if (!api.boot.running)
            return;
        // An installed theme's entry is a file: URL outside qml/: it takes the default look's chime.
        var look = api.theme.entry.indexOf("file:") === 0 ? "" : api.theme.entry.replace(/[^\/]*$/, "");
        chime.source = api.theme.soundFiles.boot || Qt.resolvedUrl("../" + look + "assets/sounds/boot.wav");
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

    // Opaque, so the scenegraph's depth test skips the home it covers until the hole opens.
    Rectangle {
        anchors.fill: parent
        visible: boot.hole === 0
        color: api.theme.ground
    }

    // The ground, with a hole that opens onto the look's home from landAt.
    Shape {
        anchors.fill: parent
        visible: boot.primer || boot.hole > 0
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: api.theme.ground
            fillRule: ShapePath.OddEvenFill
            strokeWidth: -1
            strokeColor: "transparent"

            PathRectangle {
                width: boot.width
                height: boot.height
            }
            PathAngleArc {
                moveToStart: true
                centerX: boot.width / 2
                centerY: boot.height / 2
                radiusX: boot.hole
                radiusY: boot.hole
                sweepAngle: 360
            }
        }
    }

    Loader {
        anchors.fill: parent
        active: api.boot.running && !boot.plain
        visible: boot.t < 1200

        sourceComponent: ShaderEffect {
            readonly property vector2d resolution: Qt.vector2d(width, height)
            readonly property vector2d center: Qt.vector2d(width / 2, height / 2)
            readonly property real travel: boot.travel(boot.t)
            readonly property real streak: 0.3 * Math.pow(boot.speed(boot.t), 1.2)
            readonly property real dp: boot.dp
            readonly property color ink: boot.inked(0.85 * boot.field)

            fragmentShader: Qt.resolvedUrl("../assets/shaders/hairlines.frag.qsb")
        }
    }

    // The stacked lockup's geometry (brand/universe-lockup-stacked-dark.svg), in the mark's units.
    Item {
        id: lockup

        width: 186.8 * boot.u
        height: 118.23 * boot.u
        anchors.centerIn: parent
        scale: (1.22 - 0.22 * boot.outBack(boot.seg(910, 480))) * (1 + 0.04 * boot.out)
        opacity: boot.primer ? 0.002 : boot.seg(boot.brake, 60) * (1 - boot.out)

        Layer {
            source: "../assets/brand/ring-back.svg"
        }

        Layer {
            source: "../assets/brand/planet.svg"
        }

        Layer {
            source: "../assets/brand/ring-front.svg"
        }

        Image {
            y: 96.2 * boot.u
            width: parent.width
            height: 21.73 * boot.u
            sourceSize: Qt.size(width, height)
            source: boot.light ? "../assets/brand/wordmark.svg" : "../assets/brand/wordmark-dark.svg"
        }
    }

    // The ring the mark sends out as it lands, about the planet, and the circle that opens the home.
    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        Ring {
            visible: boot.primer || boot.t >= 920 && boot.echo < 1
            centerX: boot.width / 2
            centerY: lockup.y + 35 * boot.u
            radius: 35 * boot.u * (1.05 + 2.2 * boot.echo)
            strokeColor: boot.inked(boot.primer ? 0.002 : 0.7 * (1 - boot.echo))
        }

        Ring {
            visible: boot.hole > 0
            centerX: boot.width / 2
            centerY: boot.height / 2
            radius: boot.hole
            strokeColor: boot.inked(0.85 - 0.5 * boot.opening)
        }
    }

    component Layer: Image {
        x: 29.4 * boot.u
        y: -29 * boot.u
        width: 128 * boot.u
        height: width
        sourceSize: Qt.size(width, height)
        smooth: true
    }

    component Ring: ShapePath {
        id: ring

        property real centerX
        property real centerY
        property real radius
        property bool visible

        fillColor: "transparent"
        strokeWidth: visible ? 1.5 * boot.dp : -1

        PathAngleArc {
            moveToStart: true
            centerX: ring.centerX
            centerY: ring.centerY
            radiusX: ring.radius
            radiusY: ring.radius
            sweepAngle: 360
        }
    }

    SoundEffect {
        id: chime
    }

    SequentialAnimation {
        id: intro

        NumberAnimation {
            target: boot
            property: "t"
            to: boot.landAt
            duration: boot.landAt
        }
        ScriptAction {
            script: api.boot.land()
        }
        NumberAnimation {
            target: boot
            property: "t"
            to: boot.end
            duration: boot.end - boot.landAt
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
