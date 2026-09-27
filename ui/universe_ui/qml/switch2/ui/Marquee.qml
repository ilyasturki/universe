import QtQuick
import "../core"

Item {
    id: marquee

    property alias text: label.text
    property alias color: label.color
    property alias font: label.font
    property real maxWidth: 0
    property bool running: true
    property color fade: Theme.ground
    readonly property real travel: Math.max(0, label.implicitWidth - maxWidth)
    readonly property bool overflows: travel > 1
    property real offset: 0
    property real shift: 0

    width: Math.min(label.implicitWidth, maxWidth)
    height: label.implicitHeight
    clip: overflows

    function reset() {
        scroll.stop();
        offset = 0;
        label.opacity = 1;
        if (overflows && running && visible)
            scroll.start();
    }

    onTextChanged: {
        reset();
        enter.restart();
    }
    onTravelChanged: reset()
    onRunningChanged: reset()
    onVisibleChanged: reset()

    Label {
        id: label

        x: marquee.offset + marquee.shift
        font.family: Theme.sans
    }

    ParallelAnimation {
        id: enter

        NumberAnimation {
            target: marquee
            property: "shift"
            from: Theme.dp(8)
            to: 0
            duration: 90
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: marquee
            property: "opacity"
            from: 0
            to: 1
            duration: 60
        }
    }

    SequentialAnimation {
        id: scroll

        loops: Animation.Infinite

        PauseAnimation {
            duration: 1070
        }
        NumberAnimation {
            target: marquee
            property: "offset"
            to: -marquee.travel
            duration: marquee.travel / Theme.dp(150) * 1000
        }
        PauseAnimation {
            duration: 1070
        }
        NumberAnimation {
            target: label
            property: "opacity"
            to: 0
            duration: Theme.durFocus
        }
        PropertyAction {
            target: marquee
            property: "offset"
            value: 0
        }
        NumberAnimation {
            target: label
            property: "opacity"
            to: 1
            duration: Theme.durQuick
        }
    }

    Rectangle {
        anchors.left: parent.left
        width: Theme.dp(36)
        height: parent.height
        visible: marquee.overflows
        opacity: Math.min(1, -marquee.offset / width)
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop {
                position: 0
                color: marquee.fade
            }
            GradientStop {
                position: 1
                color: Qt.rgba(marquee.fade.r, marquee.fade.g, marquee.fade.b, 0)
            }
        }
    }

    Rectangle {
        anchors.right: parent.right
        width: Theme.dp(36)
        height: parent.height
        visible: marquee.overflows
        opacity: Math.min(1, (marquee.travel + marquee.offset) / width)
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop {
                position: 0
                color: Qt.rgba(marquee.fade.r, marquee.fade.g, marquee.fade.b, 0)
            }
            GradientStop {
                position: 1
                color: marquee.fade
            }
        }
    }
}
