import QtQuick
import "../core"

// The slim strip under the screens the console never had: bottom right, as its "L1 / R1 Switch Tabs".
Item {
    id: bar

    property var hints: []
    property string key: ""

    implicitHeight: Theme.dp(76)

    onHintsChanged: {
        var k = JSON.stringify(hints);
        if (k !== key) {
            key = k;
            rep.model = hints;
        }
    }

    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop {
                position: 0.0
                color: Qt.rgba(0, 0, 0, 0)
            }
            GradientStop {
                position: 1.0
                color: Qt.rgba(0, 0, 0, 0.45)
            }
        }
    }

    Row {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(Theme.columnRight - 24)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(36)

        Repeater {
            id: rep

            Row {
                spacing: Theme.dp(10)

                readonly property var hint: modelData

                Repeater {
                    model: String(hint.glyph).split(" ")

                    HintGlyph {
                        anchors.verticalCenter: parent.verticalCenter
                        glyph: modelData
                        dim: hint.dim === true
                    }
                }

                Label {
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.label
                    color: modelData.dim === true ? Theme.textDisabled : Theme.text
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }
            }
        }
    }
}
