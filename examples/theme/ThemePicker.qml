import QtQuick

FocusScope {
    id: picker

    property real unit: 1

    signal closed

    function open() {
        var ids = api.theme.themes.map(function (t) {
            return t.id;
        });
        list.currentIndex = Math.max(0, ids.indexOf(api.theme.current));
        visible = true;
        list.forceActiveFocus();
    }

    function close() {
        visible = false;
        closed();
    }

    visible: false

    Rectangle {
        anchors.fill: parent
        color: "#cc000000"
    }

    Rectangle {
        anchors.centerIn: parent
        width: 760 * picker.unit
        height: list.contentHeight + 64 * picker.unit
        radius: 20 * picker.unit
        color: "#1e212b"

        ListView {
            id: list
            objectName: "looks"
            anchors.fill: parent
            anchors.margins: 32 * picker.unit
            model: api.theme.themes
            interactive: false
            keyNavigationEnabled: true

            delegate: Rectangle {
                id: look

                required property var modelData
                readonly property bool current: ListView.isCurrentItem

                width: list.width
                height: 72 * picker.unit
                radius: 12 * picker.unit
                color: current ? "#f2f2f2" : "transparent"

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    x: 24 * picker.unit
                    text: look.modelData.name + (look.modelData.id === api.theme.current ? "  ✓" : "")
                    color: look.current ? "#14161c" : look.modelData.unavailable ? "#5d6270" : "#f2f2f2"
                    font.pixelSize: 30 * picker.unit
                }
            }

            Keys.onPressed: function (event) {
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    var look = api.theme.themes[currentIndex];
                    if (!look || look.unavailable || look.id === api.theme.current)
                        picker.close();
                    else
                        // The new look replaces this tree: the key is done with it first.
                        Qt.callLater(api.theme.set, look.id);
                } else if (api.keys.isCancel(event) || api.keys.isMenu(event)) {
                    event.accepted = true;
                    picker.close();
                }
            }
        }
    }
}
