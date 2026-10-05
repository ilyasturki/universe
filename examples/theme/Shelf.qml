import QtQuick

ListView {
    id: shelf

    property real unit: 1

    orientation: ListView.Horizontal
    spacing: 32 * unit
    leftMargin: 96 * unit
    rightMargin: 96 * unit
    model: api.allGames
    keyNavigationEnabled: true
    highlightRangeMode: ListView.ApplyRange
    preferredHighlightBegin: 96 * unit
    preferredHighlightEnd: width / 2
    highlightMoveDuration: 160

    delegate: Item {
        id: tile

        required property var modelData
        readonly property bool current: ListView.isCurrentItem

        width: 300 * shelf.unit
        height: shelf.height
        scale: current ? 1 : 0.88

        Behavior on scale {
            NumberAnimation {
                duration: 140
            }
        }

        Rectangle {
            id: cover
            width: parent.width
            height: 400 * shelf.unit
            radius: 12 * shelf.unit
            color: "#232733"
            border.width: tile.current ? 4 * shelf.unit : 0
            border.color: "#f2f2f2"
            clip: true

            Image {
                anchors.fill: parent
                anchors.margins: parent.border.width
                source: tile.modelData.assets.boxFront
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
            }
        }

        Text {
            anchors.top: cover.bottom
            anchors.topMargin: 16 * shelf.unit
            width: parent.width
            text: tile.modelData.title
            color: tile.current ? "#f2f2f2" : "#9aa0ad"
            font.pixelSize: 26 * shelf.unit
            elide: Text.ElideRight
        }
    }

    Keys.onPressed: function (event) {
        if (api.keys.isAccept(event) && currentIndex >= 0) {
            event.accepted = true;
            api.allGames.get(currentIndex).launch();
        }
    }
}
