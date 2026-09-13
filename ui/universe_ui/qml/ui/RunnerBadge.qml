import QtQuick
import "../core"

Item {
    id: root

    property string runner: ""
    property string name: ""
    property real size: Theme.dp(26)
    property real labelSize: Theme.dp(24)

    readonly property string logo: runner !== "" ? api.screens.runners.logo(runner) : ""
    readonly property bool hasIcon: logo !== "" && icon.status === Image.Ready

    implicitWidth: (hasIcon ? icon.width + Theme.dp(12) : 0) + label.implicitWidth
    implicitHeight: Math.max(size, label.implicitHeight)

    Image {
        id: icon
        width: root.hasIcon ? root.size * Math.max(1, Math.min(2.4, implicitHeight > 0 ? implicitWidth / implicitHeight : 1)) : 0
        height: root.size
        anchors.verticalCenter: parent.verticalCenter
        source: root.logo !== "" ? Qt.resolvedUrl("../" + root.logo) : ""
        sourceSize.height: Math.round(root.size * 2)
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        smooth: true
        mipmap: true
        visible: root.hasIcon
    }

    Text {
        id: label
        anchors.left: root.hasIcon ? icon.right : parent.left
        anchors.leftMargin: root.hasIcon ? Theme.dp(12) : 0
        anchors.verticalCenter: parent.verticalCenter
        text: root.name !== "" ? root.name : root.runner
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Medium
        font.pixelSize: root.labelSize
    }
}
