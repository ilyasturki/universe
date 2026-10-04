import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: modal

    property bool open: false
    property var callback: null
    property bool carded: true
    property color scrimColor: Theme.scrim
    property real scrimOpacity: 1.0
    readonly property alias card: card
    default property alias content: card.data

    anchors.fill: parent
    visible: scrim.opacity > 0.01
    focus: open

    function present(done) {
        callback = done || null;
        Sound.play("open");
        open = true;
        forceActiveFocus();
    }

    function finish(value, all) {
        var cb = callback;
        callback = null;
        open = false;
        if (cb)
            cb(value, all === true);
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: modal.scrimColor
        opacity: modal.open ? modal.scrimOpacity : 0.0

        Behavior on opacity {
            NumberAnimation {
                duration: modal.open ? Theme.durQuick : 170
                easing.type: Easing.InOutQuad
            }
        }

        Block {
            onTapped: if (modal.carded && modal.open)
                api.keys.press("Cancel")
        }
    }

    Rectangle {
        id: card
        objectName: "modalCard"

        anchors.centerIn: modal.carded ? parent : undefined
        anchors.fill: modal.carded ? undefined : parent
        radius: Theme.dp(16)
        color: modal.carded ? Theme.dialog : "transparent"
        // Fading out, it takes no finger: an A it sent would land on the page.
        enabled: modal.open
        opacity: modal.open ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation {
                duration: modal.open ? Theme.durQuick : 170
                easing.type: Easing.InOutQuad
            }
        }

        Block {}
    }
}
