import QtQuick
import "../core"

// One group of settings: a header (a module's name and state, or a section's caps label),
// its Enabled switch when the group has one, and the rows below. Sizes come from the
// parent, which lays the columns out from the same numbers.
Item {
    id: card

    // { title, meta, warning, caps, control, off, rows }
    property var group: ({})
    property var rows: []
    property int cursor: -1
    property bool active: false
    property bool compact: false
    property real pad: Theme.dp(8)
    property real headerHeight: 0
    property real rowHeight: Theme.dp(66)

    readonly property bool hasControl: group.control >= 0
    readonly property bool headerFocused: hasControl && cursor === group.control && active
    readonly property color onFocus: Qt.rgba(0.063, 0.067, 0.086, 0.6)

    height: pad * 2 + headerHeight + group.rows.length * rowHeight + 2
    // A module that is off fades, except while its switch is the focused thing.
    opacity: group.off && !headerFocused ? 0.55 : 1.0

    Behavior on opacity {
        NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(24)
        color: Qt.rgba(1, 1, 1, 0.04)
        border.width: 1
        border.color: Qt.rgba(1, 1, 1, 0.10)
    }

    Rectangle {
        id: head

        x: 1 + card.pad
        y: 1 + card.pad
        width: parent.width - 2 - card.pad * 2
        height: card.headerHeight
        radius: Theme.dp(14)
        visible: card.headerHeight > 0
        color: card.headerFocused ? Theme.text : "transparent"

        Behavior on color {
            ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        Column {
            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(18)
            anchors.right: toggle.visible ? toggle.left : parent.right
            anchors.rightMargin: Theme.dp(20)
            anchors.verticalCenter: parent.verticalCenter
            spacing: Theme.dp(2)

            CapsLabel {
                visible: card.group.caps === true
                text: (card.group.title || "").toUpperCase()
                color: card.headerFocused ? card.onFocus : Theme.textMuted
            }

            Text {
                visible: card.group.caps !== true
                width: parent.width
                text: card.group.title || ""
                color: card.headerFocused ? Theme.onLight : Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(card.compact ? 24 : 27)
                elide: Text.ElideRight
            }

            Text {
                visible: text !== ""
                width: parent.width
                text: (card.group.meta || "")
                      + (card.group.warning
                         ? (card.group.meta ? " · " : "") + "<font color=\"#e0655a\">" + card.group.warning + "</font>"
                         : "")
                textFormat: Text.StyledText
                color: card.headerFocused ? card.onFocus : Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(card.compact ? 18 : 19)
                elide: Text.ElideRight
            }
        }

        SettingsToggle {
            id: toggle
            visible: card.hasControl
            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(16)
            anchors.verticalCenter: parent.verticalCenter
            on: card.hasControl && card.rows[card.group.control] ? card.rows[card.group.control].value === true : false
            focused: card.headerFocused
        }
    }

    Column {
        x: 1 + card.pad
        y: 1 + card.pad + card.headerHeight
        width: parent.width - 2 - card.pad * 2

        Repeater {
            model: card.group.rows

            SettingsRow {
                readonly property bool prevFocused: index > 0 && card.group.rows[index - 1] === card.cursor && card.active

                width: parent.width
                height: card.rowHeight
                entry: card.rows[modelData] || ({})
                focused: modelData === card.cursor && card.active
                compact: card.compact
                separator: index > 0 && !focused && !prevFocused
            }
        }
    }
}
