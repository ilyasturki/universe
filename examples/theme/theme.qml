import QtQuick

FocusScope {
    id: root
    objectName: "sampleTheme"

    readonly property real unit: height > 0 ? height / 1080 : 1

    focus: true

    Component.onCompleted: {
        if (api.theme.takeLanding() === "themes")
            picker.open();
    }

    Rectangle {
        anchors.fill: parent
        color: "#14161c"
    }

    Text {
        id: heading
        x: 96 * root.unit
        y: 72 * root.unit
        text: "Sample"
        color: "#f2f2f2"
        font.pixelSize: 56 * root.unit
        font.weight: Font.DemiBold
    }

    Text {
        anchors.left: heading.right
        anchors.leftMargin: 24 * root.unit
        anchors.baseline: heading.baseline
        text: api.allGames.count + " games"
        color: "#9aa0ad"
        font.pixelSize: 30 * root.unit
    }

    Shelf {
        id: shelf
        objectName: "shelf"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        height: 500 * root.unit
        unit: root.unit
        focus: true
    }

    Text {
        anchors.left: heading.left
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 64 * root.unit
        text: "A  Play        Start  Themes"
        color: "#9aa0ad"
        font.pixelSize: 26 * root.unit
    }

    ThemePicker {
        id: picker
        objectName: "themePicker"
        anchors.fill: parent
        unit: root.unit
        onClosed: shelf.forceActiveFocus()
    }

    Keys.onPressed: function (event) {
        if (api.keys.isMenu(event)) {
            event.accepted = true;
            picker.open();
        }
    }
}
