import QtQuick
import "../core"

Text {
    id: root

    property real tracking: 0.13
    // font.pixelSize and font.letterSpacing are one group property: reading one to set the other loops.
    property real size: Theme.dp(19)

    color: Theme.textMuted
    font.family: Theme.sans
    font.weight: Font.DemiBold
    font.pixelSize: root.size
    font.letterSpacing: root.size * root.tracking
}
