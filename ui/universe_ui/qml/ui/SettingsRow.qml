import QtQuick
import "../core"

Item {
    id: row

    property var entry: ({})
    property bool focused: false
    property bool compact: false
    property bool separator: false
    property real baseHeight: Theme.dp(66)

    readonly property bool info: entry.type === "info"
    // A failed check reads whole: what is wrong and what to do wrap under the label, and the row grows to hold them.
    readonly property bool explains: info && entry.fix !== undefined && entry.fix !== ""
    readonly property real naturalHeight: explains ? notes.y + notes.height + Theme.dp(20) : baseHeight
    readonly property bool hasSwitch: entry.switch === true
    // A pad button reads its macros by name, the hold one marked, and nothing without one; with no code it keeps "Unbound".
    readonly property bool button: entry.bound === true && entry.home !== true
    readonly property string value: button ? [entry.press && entry.press.label, entry.hold && "Hold · " + entry.hold.label].filter(Boolean).join(" · ") : entry.display || ""
    readonly property bool hasImage: entry.image != null && String(entry.image) !== ""
    readonly property bool hasGlyph: entry.slot !== undefined && String(entry.slot) !== "" && entry.family !== undefined
    // An icon naming a file (a runner's logo) is drawn whole in a square; a bare name is a menu glyph.
    readonly property bool iconIsFile: entry.icon != null && String(entry.icon).indexOf("/") >= 0
    readonly property bool hasMark: !hasImage && iconIsFile && mark.status === Image.Ready
    readonly property bool keepsMark: hasMark || entry.iconSlot === true
    readonly property bool hasIcon: !hasGlyph && !iconIsFile && entry.icon !== undefined && String(entry.icon) !== ""
    // A logo for the value (the runner picked), drawn just before it.
    readonly property bool hasValueMark: entry.valueIcon != null && String(entry.valueIcon) !== "" && valueMark.status === Image.Ready
    readonly property real labelInset: hasImage ? thumb.width + Theme.dp(24) : keepsMark ? mark.width + Theme.dp(28) : hasGlyph || hasIcon ? Theme.dp(18) + lead.width + Theme.dp(16) : Theme.dp(18)
    readonly property color onFocus: Qt.rgba(0.063, 0.067, 0.086, 0.7)
    readonly property real valueMax: Math.max(Theme.dp(120), width * 0.6 - (hasImage ? thumb.width : 0))
    // A search hit: where the row lives, muted, in front of its label; a tag (ADVANCED) next to the value.
    readonly property string path: entry.path !== undefined && entry.path !== null ? String(entry.path) : ""
    readonly property string tag: entry.tag !== undefined && entry.tag !== null ? String(entry.tag) : ""
    // Where an inheritable value comes from when something sets it: this game or runner, or the global settings; the
    // default, most rows, reads plain. A row inherited with no `origin` (a runner's found program) keeps the plain chip.
    readonly property string origin: entry.origin !== undefined && entry.origin !== null ? String(entry.origin) : ""
    readonly property var tags: [tag, origin === "game" ? "THIS GAME" : origin === "runner" ? "THIS RUNNER" : origin === "global" ? "GLOBAL" : origin === "" && entry.inherited === true ? "INHERITED" : ""].filter(Boolean)

    opacity: entry.disabled === true && !focused ? 0.45 : 1.0

    function esc(text) {
        return String(text).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    }

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
            ColorEase {}
        }
    }

    // How much of a download is on the disk, as a hairline along the row's foot.
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: row.labelInset
        anchors.rightMargin: Theme.dp(16)
        anchors.bottomMargin: Theme.dp(5)
        height: Theme.dp(3)
        radius: height / 2
        visible: row.entry.progress !== undefined && row.entry.progress > 0
        color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.15) : Qt.rgba(1, 1, 1, 0.10)

        Rectangle {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: parent.width * Math.min(1, row.entry.progress || 0)
            radius: height / 2
            color: row.focused ? Theme.onLight : Theme.textSecondary
        }
    }

    Item {
        id: band
        width: parent.width
        height: row.explains ? row.baseHeight : row.height
    }

    Loader {
        id: thumb

        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(10)
        anchors.verticalCenter: band.verticalCenter
        height: band.height - Theme.dp(12)
        width: height
        active: row.hasImage

        sourceComponent: RoundedMask {
            radius: Theme.dp(6)

            Rectangle {
                anchors.fill: parent
                color: Theme.cardBase
            }

            Image {
                id: art
                anchors.fill: parent
                source: row.entry.image
                asynchronous: true
                fillMode: Image.PreserveAspectCrop
                sourceSize.height: 256
                smooth: true
            }
        }
    }

    Loader {
        id: lead

        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(18)
        anchors.verticalCenter: band.verticalCenter
        height: band.height
        active: row.hasGlyph || row.hasIcon

        sourceComponent: row.hasGlyph ? padGlyph : menuGlyph
    }

    Component {
        id: padGlyph

        Item {
            implicitWidth: glyph.implicitWidth

            PadGlyph {
                id: glyph
                anchors.verticalCenter: parent.verticalCenter
                family: String(row.entry.family)
                slot: String(row.entry.slot)
                unit: Theme.dp(row.compact ? 26 : 28)
                ink: row.focused ? Theme.onLight : Theme.text
            }
        }
    }

    Component {
        id: menuGlyph

        Item {
            implicitWidth: icon.width

            MenuGlyph {
                id: icon
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(row.compact ? 24 : 26)
                height: width
                kind: String(row.entry.icon)
                tint: row.focused ? Theme.onLight : Theme.text
            }
        }
    }

    Image {
        id: mark
        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(16)
        anchors.verticalCenter: band.verticalCenter
        height: band.height - Theme.dp(22)
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
        anchors.verticalCenter: band.verticalCenter
        text: row.path !== "" ? "<font color=\"" + (row.focused ? "#5c5f69" : "#8a8d96") + "\">" + row.esc(row.path) + " › </font>" + row.esc(row.entry.label || "") : row.entry.label || ""
        textFormat: row.path !== "" ? Text.StyledText : Text.PlainText
        color: row.focused ? Theme.onLight : Theme.text
        font.family: Theme.sans
        font.weight: row.focused ? Font.DemiBold : Font.Medium
        font.pixelSize: Theme.dp(row.compact ? 22 : 23)
        elide: Text.ElideRight
    }

    component TagChip: Rectangle {
        property string text: ""

        anchors.verticalCenter: parent.verticalCenter
        width: tagText.width + Theme.dp(18)
        height: tagText.height + Theme.dp(8)
        radius: Theme.dp(8)
        color: "transparent"
        border.width: 1
        border.color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.25) : Qt.rgba(1, 1, 1, 0.14)

        CapsLabel {
            id: tagText
            anchors.centerIn: parent
            text: parent.text
            size: Theme.dp(15)
            tracking: 0.08
            color: row.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Theme.textFaint
        }
    }

    Item {
        id: control

        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(16)
        anchors.verticalCenter: band.verticalCenter
        // The shown variant alone: childrenRect would count the hidden ones too.
        width: boolRow.visible ? boolRow.width : valueRow.visible ? valueRow.width : infoRow.width
        height: band.height

        Row {
            id: boolRow
            visible: row.entry.type === "bool"
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(16)

            Repeater {
                model: row.tags

                TagChip {
                    text: modelData
                }
            }

            SettingsToggle {
                anchors.verticalCenter: parent.verticalCenter
                on: row.entry.value === true
                focused: row.focused
            }
        }

        Row {
            id: valueRow
            visible: row.entry.type !== "bool" && !row.info
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(16)

            Repeater {
                model: row.tags

                TagChip {
                    text: modelData
                }
            }

            // A game's size, in figures of one width so a column of them lines up.
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: text !== ""
                text: row.entry.size || ""
                color: row.focused ? Theme.onLight : Theme.text
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(21)
                font.features: {
                    "tnum": 1
                }
            }

            Image {
                id: valueMark
                anchors.verticalCenter: parent.verticalCenter
                height: Theme.dp(row.compact ? 28 : 30)
                width: height
                source: row.entry.valueIcon != null && String(row.entry.valueIcon) !== "" ? Qt.resolvedUrl("../" + row.entry.valueIcon) : ""
                asynchronous: true
                fillMode: Image.PreserveAspectFit
                sourceSize.height: 128
                smooth: true
                mipmap: true
                visible: row.hasValueMark
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: text !== "" && !(row.hasSwitch && !row.entry.warning)
                text: row.value
                color: row.focused ? row.onFocus : row.entry.accent === true ? "#5aa0ff" : Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                elide: Text.ElideMiddle
                width: Math.min(implicitWidth, row.valueMax)
            }

            SettingsToggle {
                visible: row.hasSwitch
                anchors.verticalCenter: parent.verticalCenter
                on: row.entry.value === true
                focused: row.focused
                opacity: row.entry.warning && row.entry.value !== true ? 0.35 : 1.0
            }

            Canvas {
                width: Theme.dp(14)
                height: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                visible: row.entry.type !== "static"
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

        Row {
            id: infoRow
            visible: row.info
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(14)

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: text !== "" && !row.explains
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

    Column {
        id: notes

        x: row.labelInset
        y: row.baseHeight - Theme.dp(10)
        width: parent.width - x - Theme.dp(16) - Theme.dp(28)
        visible: row.explains
        spacing: Theme.dp(8)

        Text {
            width: parent.width
            text: row.entry.detail || ""
            wrapMode: Text.Wrap
            color: row.focused ? row.onFocus : Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(20)
        }

        Text {
            width: parent.width
            text: "<b>To fix:</b> " + row.esc(row.entry.fix || "")
            textFormat: Text.StyledText
            wrapMode: Text.Wrap
            color: row.focused ? Theme.onLight : Theme.text
            font.family: Theme.sans
            font.pixelSize: Theme.dp(20)
        }
    }
}
