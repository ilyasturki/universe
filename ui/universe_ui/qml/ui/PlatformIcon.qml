import QtQuick
import Qt5Compat.GraphicalEffects
import "../core"

// The collection's logo by shortname; a collection without one shows its name.
Item {
    id: root

    property var game: null
    property real size: Theme.dp(26)
    property color color: Theme.text
    property real labelSize: Theme.dp(24)

    readonly property var collection: game && game.collections.count > 0 ? game.collections.get(0) : null
    readonly property string shortName: collection ? collection.shortName : ""
    readonly property bool hasIcon: icon.status === Image.Ready

    implicitWidth: hasIcon ? icon.width : label.implicitWidth
    implicitHeight: hasIcon ? size : label.implicitHeight

    // Wordmarks (DS, PS3, Wii) may run up to this many times wider than tall.
    readonly property real maxAspect: 3.6
    readonly property real aspect: icon.implicitHeight > 0 ? icon.implicitWidth / icon.implicitHeight : 1
    readonly property real iconHeight: Math.min(size, size * maxAspect / aspect)

    Image {
        id: icon
        width: root.iconHeight * root.aspect
        height: root.iconHeight
        anchors.verticalCenter: parent.verticalCenter
        source: root.shortName !== "" ? Qt.resolvedUrl("../assets/platforms/" + root.shortName + ".svg") : ""
        // Height alone keeps the aspect; a width as well makes Qt size the SVG by it.
        sourceSize.height: Math.round(root.size * 2)
        fillMode: Image.PreserveAspectFit
        smooth: true
        mipmap: true
        // Uncoloured where the overlay cannot run.
        visible: root.hasIcon && Theme.software
    }

    ColorOverlay {
        anchors.fill: icon
        source: icon
        color: root.color
        visible: root.hasIcon && !Theme.software
    }

    Text {
        id: label
        anchors.verticalCenter: parent.verticalCenter
        visible: !root.hasIcon
        text: root.collection ? root.collection.name : ""
        color: root.color
        font.family: Theme.sans
        font.pixelSize: root.labelSize
    }
}
