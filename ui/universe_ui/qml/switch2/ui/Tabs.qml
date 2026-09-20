import QtQuick
import "../core"
import "../sound"

Item {
    id: tabs

    property var names: []
    property int index: 0

    signal changed(int index)

    implicitHeight: Theme.dp(110)

    function step(d) {
        var next = index + d;
        if (next < 0 || next >= names.length) {
            Sound.play("edge");
            return;
        }
        Sound.play("select");
        index = next;
        tabs.changed(index);
    }

    component Bumper: HintGlyph {
        anchors.verticalCenter: parent.verticalCenter
        unit: Theme.dp(38)
        fill: "transparent"
        ink: Theme.textMuted
    }

    Row {
        anchors.centerIn: parent
        anchors.verticalCenterOffset: -Theme.dp(6)
        spacing: Theme.dp(100)

        Bumper {
            glyph: "LB"
        }

        Repeater {
            model: tabs.names

            Item {
                readonly property bool open: index === tabs.index

                width: Theme.dp(330)
                height: Theme.dp(60)

                Label {
                    anchors.centerIn: parent
                    text: modelData
                    color: parent.open ? Theme.accent : Theme.text
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: parent.width
                    height: Theme.dp(4)
                    color: Theme.accent
                    visible: parent.open
                }
            }
        }

        Bumper {
            glyph: "RB"
        }
    }

    Hairline {
        anchors.leftMargin: Theme.dp(Theme.edgeMargin)
        anchors.rightMargin: Theme.dp(Theme.edgeMargin)
        color: Theme.hairline
    }
}
