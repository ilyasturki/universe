import QtQuick
import Qt5Compat.GraphicalEffects
import "../core"
import "Macros.js" as Macros

// One setting inside a card: its label, and by type a switch, a value with a chevron,
// or a check's detail and status dot. Focused, it is the one white row on the screen.
// A row with an `image` (a game of a source) shows it as a small cover before the label; one
// with an `icon` (a runner's logo) shows it whole, in a square. A pad button's row prints its
// macros as chips, one per trigger.
Item {
    id: row

    property var entry: ({})
    property bool focused: false
    property bool compact: false
    // Hidden next to the focused row, where the white pill already draws the edge.
    property bool separator: false

    readonly property bool info: entry.type === "info"
    // A button with no code on this connection says "Unbound" instead: its macros cannot fire.
    readonly property var macros: {
        var out = [];
        if (entry.bound === false)
            return out;
        if (entry.press)
            out.push({ tag: "PRESS", macro: entry.press });
        if (entry.hold)
            out.push({ tag: "HOLD", macro: entry.hold });
        return out;
    }
    readonly property bool hasImage: entry.image !== undefined && entry.image !== null && String(entry.image) !== ""
    // A controller row carries its slot and family: the button is drawn before its name.
    readonly property bool hasGlyph: entry.slot !== undefined && String(entry.slot) !== "" && entry.family !== undefined
    // An icon naming a file (a runner's logo) is drawn whole in a square; a bare name is a menu glyph.
    readonly property bool iconIsFile: entry.icon !== undefined && entry.icon !== null && String(entry.icon).indexOf("/") >= 0
    readonly property bool hasMark: !hasImage && iconIsFile && mark.status === Image.Ready
    // A list where most rows carry a mark keeps the label aligned on the ones without.
    readonly property bool keepsMark: hasMark || entry.iconSlot === true
    readonly property bool hasIcon: !hasGlyph && !iconIsFile && entry.icon !== undefined && String(entry.icon) !== ""
    readonly property real labelInset: hasImage ? thumb.width + Theme.dp(24) : keepsMark ? mark.width + Theme.dp(28) : hasGlyph || hasIcon ? Theme.dp(18) + lead.width + Theme.dp(16) : Theme.dp(18)
    readonly property color onFocus: Qt.rgba(0.063, 0.067, 0.086, 0.7)
    // A value or a detail leaves the label at least a third of the row.
    readonly property real valueMax: Math.max(Theme.dp(120), width * 0.6 - (hasImage ? thumb.width : 0))
    // The software scenegraph (offscreen tests) drops every ShaderEffect: corners go square there.
    readonly property bool software: GraphicsInfo.api === GraphicsInfo.Software

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Theme.dp(18)
        anchors.rightMargin: Theme.dp(16)
        anchors.top: parent.top
        height: 1
        visible: row.separator
        color: Qt.rgba(1, 1, 1, 0.06)
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(14)
        color: row.focused ? Theme.text : "transparent"

        Behavior on color {
            ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
    }

    Item {
        id: thumb

        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(10)
        anchors.verticalCenter: parent.verticalCenter
        height: parent.height - Theme.dp(12)
        width: Math.round(height * 2 / 3)
        visible: row.hasImage
        layer.enabled: visible && !row.software
        layer.smooth: true
        layer.effect: row.software ? null : thumbEffect

        Component {
            id: thumbEffect
            OpacityMask { maskSource: thumbMask }
        }

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(6)
            color: Theme.cardBase
        }

        Image {
            anchors.fill: parent
            source: row.hasImage ? row.entry.image : ""
            asynchronous: true
            fillMode: Image.PreserveAspectCrop
            sourceSize.height: 256
            smooth: true
        }
    }

    Rectangle {
        id: thumbMask
        anchors.fill: thumb
        radius: Theme.dp(6)
        color: "white"
        antialiasing: true
        visible: false
    }

    Item {
        id: lead

        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(18)
        anchors.verticalCenter: parent.verticalCenter
        width: row.hasGlyph ? glyph.implicitWidth : row.hasIcon ? icon.width : 0
        height: parent.height
        visible: row.hasGlyph || row.hasIcon

        PadGlyph {
            id: glyph
            anchors.verticalCenter: parent.verticalCenter
            visible: row.hasGlyph
            family: row.hasGlyph ? String(row.entry.family) : "xbox"
            slot: row.hasGlyph ? String(row.entry.slot) : "south"
            unit: Theme.dp(row.compact ? 26 : 28)
            ink: row.focused ? Theme.onLight : Theme.text
        }

        MenuGlyph {
            id: icon
            anchors.verticalCenter: parent.verticalCenter
            visible: row.hasIcon
            width: Theme.dp(row.compact ? 24 : 26)
            height: width
            kind: row.hasIcon ? String(row.entry.icon) : ""
            tint: row.focused ? Theme.onLight : Theme.text
        }
    }

    Image {
        id: mark
        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(16)
        anchors.verticalCenter: parent.verticalCenter
        height: parent.height - Theme.dp(22)
        width: height
        source: !row.hasImage && row.iconIsFile ? Qt.resolvedUrl("../" + row.entry.icon) : ""
        asynchronous: true
        fillMode: Image.PreserveAspectFit
        sourceSize.height: 128
        smooth: true
        mipmap: true
        visible: row.hasMark
    }

    Text {
        anchors.left: parent.left
        anchors.leftMargin: row.labelInset
        anchors.right: control.left
        anchors.rightMargin: Theme.dp(20)
        anchors.verticalCenter: parent.verticalCenter
        text: row.entry.label || ""
        color: row.focused ? Theme.onLight : Theme.text
        font.family: Theme.sans
        font.weight: row.focused ? Font.DemiBold : Font.Medium
        font.pixelSize: Theme.dp(row.compact ? 22 : 23)
        elide: Text.ElideRight
    }

    Item {
        id: control

        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(16)
        anchors.verticalCenter: parent.verticalCenter
        // The shown variant alone: childrenRect would count the hidden ones too.
        width: toggle.visible ? toggle.width : valueRow.visible ? valueRow.width : infoRow.width
        height: parent.height

        SettingsToggle {
            id: toggle
            visible: row.entry.type === "bool"
            anchors.verticalCenter: parent.verticalCenter
            on: row.entry.value === true
            focused: row.focused
        }

        // enum, string, path, int, action: an inherited tag, the value and a chevron
        Row {
            id: valueRow
            visible: row.entry.type !== "bool" && !row.info
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(16)

            Rectangle {
                visible: row.entry.inherited === true
                anchors.verticalCenter: parent.verticalCenter
                width: tag.width + Theme.dp(18)
                height: tag.height + Theme.dp(8)
                radius: Theme.dp(8)
                color: "transparent"
                border.width: 1
                border.color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.25) : Qt.rgba(1, 1, 1, 0.14)

                CapsLabel {
                    id: tag
                    anchors.centerIn: parent
                    text: "INHERITED"
                    size: Theme.dp(15)
                    tracking: 0.08
                    color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Theme.textFaint
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: text !== "" && row.macros.length === 0
                text: row.entry.display || ""
                color: row.focused ? row.onFocus : Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                elide: Text.ElideMiddle
                width: Math.min(implicitWidth, row.valueMax)
            }

            // One chip per trigger: its tag, the action's glyph, what it does.
            Row {
                visible: row.macros.length > 0
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(10)

                Repeater {
                    model: row.macros

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: chip.width + Theme.dp(24)
                        height: Theme.dp(38)
                        radius: height / 2
                        color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.08) : Qt.rgba(1, 1, 1, 0.07)
                        border.width: 1
                        border.color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.14) : Qt.rgba(1, 1, 1, 0.12)

                        Row {
                            id: chip
                            anchors.centerIn: parent
                            spacing: Theme.dp(9)

                            CapsLabel {
                                anchors.verticalCenter: parent.verticalCenter
                                text: modelData.tag
                                size: Theme.dp(14)
                                tracking: 0.1
                                color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Theme.textMuted
                            }

                            MenuGlyph {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: kind !== ""
                                width: Theme.dp(20)
                                height: width
                                kind: Macros.icon(modelData.macro.action)
                                tint: row.focused ? Theme.onLight : Theme.text
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: modelData.macro.label || ""
                                color: row.focused ? Theme.onLight : Theme.text
                                font.family: Theme.sans
                                font.weight: Font.Medium
                                font.pixelSize: Theme.dp(19)
                                elide: Text.ElideRight
                                width: Math.min(implicitWidth, Theme.dp(260))
                            }
                        }
                    }
                }
            }

            Canvas {
                width: Theme.dp(14)
                height: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                readonly property color tint: row.focused ? Theme.onLight : Theme.textMuted
                onTintChanged: requestPaint()
                onPaint: {
                    var ctx = getContext("2d");
                    ctx.reset();
                    ctx.strokeStyle = tint;
                    ctx.lineWidth = Theme.dp(2.5);
                    ctx.lineCap = "round";
                    ctx.lineJoin = "round";
                    ctx.beginPath();
                    ctx.moveTo(width * 0.2, height * 0.15);
                    ctx.lineTo(width * 0.8, height * 0.5);
                    ctx.lineTo(width * 0.2, height * 0.85);
                    ctx.stroke();
                }
            }
        }

        // info: the detail and a status dot
        Row {
            id: infoRow
            visible: row.info
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(14)

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: text !== ""
                text: row.entry.detail || ""
                color: row.focused ? row.onFocus : Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(20)
                elide: Text.ElideMiddle
                width: Math.min(implicitWidth, row.valueMax)
            }

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(14)
                height: width
                radius: width / 2
                color: row.entry.value ? "#5fd48a" : "#e0655a"
            }
        }
    }
}
