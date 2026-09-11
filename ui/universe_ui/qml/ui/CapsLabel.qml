import QtQuick
import "../core"

Text {
    id: root

    property real tracking: 0.13
    // Reading font.pixelSize to set font.letterSpacing loops: they are one group
    // property, so the write reinvalidates the read.
    property real size: Theme.dp(19)

    color: Theme.textMuted
    font.family: Theme.sans
    font.weight: Font.DemiBold
    font.pixelSize: root.size
    font.letterSpacing: root.size * root.tracking
}
