import QtQuick
import "../core"

Item {
    id: root

    property var game: null
    property real logoWidth: Theme.dp(480)
    property real logoHeight: Theme.dp(150)
    property real titleWidth: width

    readonly property url logoSource: game && game.assets.logo ? game.assets.logo : ""
    readonly property bool titleMode: logoSource == "" && game !== null

    implicitWidth: logoWidth
    implicitHeight: titleMode ? title.height : logoHeight

    CrossfadeImage {
        width: root.logoWidth
        height: root.logoHeight
        source: root.logoSource
        fillMode: Image.PreserveAspectFit
        horizontalAlignment: Image.AlignLeft
        verticalAlignment: Image.AlignBottom
        sourceSize: Qt.size(root.logoWidth, root.logoHeight)
        duration: Theme.durBase
        opacity: root.titleMode ? 0.0 : 1.0

        Behavior on opacity { Ease {} }
    }

    Text {
        id: title

        anchors.bottom: parent.bottom
        width: root.titleWidth
        text: root.game ? root.game.title : ""
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(58)
        elide: Text.ElideRight
        opacity: root.titleMode ? 1.0 : 0.0

        Behavior on opacity { Ease {} }
    }
}
