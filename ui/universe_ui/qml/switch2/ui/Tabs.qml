import QtQuick
import "../core"
import "../sound"

Item {
    id: tabs

    property var names: []
    property int index: 0

    signal changed(int index)

    implicitHeight: Theme.dp(112)

    function step(d) {
        var next = index + d;
        if (next < 0 || next >= names.length) {
            Sound.play("edge");
            return;
        }
        Sound.play("tab");
        index = next;
        tabs.changed(index);
    }

    component Bumper: HintGlyph {
        y: Theme.dp(63) - height / 2
        unit: Theme.dp(38)
        fill: "transparent"
        ink: Theme.textMuted
    }

    Row {
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: Theme.dp(100)

        Bumper {
            glyph: "LB"

            Touch {
                anchors.margins: -Theme.dp(24)
                direct: true
                action: "PrevPage"
            }
        }

        Repeater {
            model: tabs.names

            Item {
                readonly property bool open: index === tabs.index

                width: Theme.dp(330)
                height: Theme.dp(92)

                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.baseline: parent.top
                    anchors.baselineOffset: Theme.dp(78)
                    text: modelData
                    color: parent.open ? Theme.accent : Theme.text
                    font.pixelSize: Theme.dp(27)
                }

                Rectangle {
                    y: Theme.dp(88)
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: parent.width
                    height: Theme.dp(4)
                    color: Theme.accent
                    visible: parent.open
                }

                Touch {
                    current: parent.open
                    action: ""
                    onPicked: tabs.step(index - tabs.index)
                }
            }
        }

        Bumper {
            glyph: "RB"

            Touch {
                anchors.margins: -Theme.dp(24)
                direct: true
                action: "NextPage"
            }
        }
    }

    Hairline {
        anchors.leftMargin: Theme.dp(Theme.edgeMargin)
        anchors.rightMargin: Theme.dp(Theme.edgeMargin)
        color: Theme.hairline
    }
}
