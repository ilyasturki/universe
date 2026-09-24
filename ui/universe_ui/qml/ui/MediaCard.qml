import QtQuick
import "../core"

// One cell of the Media grid, shaped by its kind: a screenshot is the picture, a recording the frame under a play disc and its length, a journal entry its words.
Item {
    id: root

    // "shot" | "recording" | "journal"
    property string kind: "shot"
    property string source: ""
    property string heading: ""
    property string excerpt: ""
    property string durationText: ""
    property bool focused: false
    property bool dimmed: false

    readonly property real radius: Theme.dp(14)
    readonly property bool words: kind === "journal"

    scale: focused ? 1.0 : 0.96
    opacity: dimmed ? Theme.idleOpacity : 1.0

    Behavior on scale {
        Ease {
            duration: Theme.durQuick
        }
    }
    Behavior on opacity {
        Ease {
            duration: Theme.durQuick
        }
    }

    FocusRing {
        anchors.fill: frame
        cornerRadius: root.radius
        gapWidth: Theme.dp(Theme.ringGap)
        opacity: root.focused ? 1.0 : 0.0
        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }
    }

    RoundedMask {
        id: frame
        anchors.fill: parent
        radius: root.radius

        Rectangle {
            anchors.fill: parent
            color: root.words || root.source === "" ? Theme.surface : Theme.cardBase
        }

        Image {
            anchors.fill: parent
            source: root.words ? "" : root.source
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            sourceSize.width: 960
        }

        MenuGlyph {
            anchors.centerIn: parent
            width: Theme.dp(44)
            height: Theme.dp(44)
            visible: root.kind === "shot" && root.source === ""
            kind: "camera"
            tint: Theme.textFaint
        }

        Item {
            anchors.fill: parent
            visible: root.kind === "recording"

            Rectangle {
                anchors.fill: parent
                color: Qt.rgba(0.02, 0.02, 0.03, 0.30)
            }

            Rectangle {
                anchors.centerIn: parent
                width: Theme.dp(56)
                height: width
                radius: width / 2
                color: Qt.rgba(1, 1, 1, 0.92)

                MenuGlyph {
                    anchors.centerIn: parent
                    anchors.horizontalCenterOffset: Theme.dp(2)
                    width: Theme.dp(28)
                    height: width
                    kind: "play"
                    tint: Theme.onLight
                }
            }

            Rectangle {
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: Theme.dp(10)
                width: lengthText.width + Theme.dp(20)
                height: Theme.dp(30)
                radius: height / 2
                color: Qt.rgba(0, 0, 0, 0.62)
                visible: root.durationText !== ""

                Text {
                    id: lengthText
                    anchors.centerIn: parent
                    text: root.durationText
                    color: "#f2f3f5"
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(17)
                }
            }
        }

        Item {
            anchors.fill: parent
            anchors.margins: Theme.dp(22)
            visible: root.words

            MenuGlyph {
                id: kindRow
                anchors.top: parent.top
                anchors.left: parent.left
                width: Theme.dp(20)
                height: width
                kind: "book"
                tint: Theme.textMuted
            }

            Text {
                id: headingText
                anchors.top: kindRow.bottom
                anchors.topMargin: Theme.dp(10)
                anchors.left: parent.left
                anchors.right: parent.right
                text: root.heading
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(24)
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
            }

            Text {
                anchors.top: headingText.bottom
                anchors.topMargin: Theme.dp(6)
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: lengthLine.visible ? lengthLine.top : parent.bottom
                anchors.bottomMargin: lengthLine.visible ? Theme.dp(6) : 0
                text: root.excerpt
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(18)
                lineHeight: 1.2
                wrapMode: Text.WordWrap
                elide: Text.ElideRight
                clip: true
            }

            Text {
                id: lengthLine
                anchors.left: parent.left
                anchors.bottom: parent.bottom
                text: root.durationText
                visible: root.durationText !== ""
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(17)
            }
        }
    }
}
