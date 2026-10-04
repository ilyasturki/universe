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
    property string openSound: "open"
    readonly property alias card: card
    default property alias content: card.data

    anchors.fill: parent
    visible: scrim.opacity > 0.01
    focus: open

    function present(done) {
        callback = done || null;
        if (openSound !== "")
            Sound.play(openSound);
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
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }

        Block {
            onTapped: if (modal.open)
                api.keys.press("Cancel")
        }
    }

    Rectangle {
        id: card

        anchors.centerIn: modal.carded ? parent : undefined
        anchors.fill: modal.carded ? undefined : parent
        radius: Theme.dp(Theme.radiusCard + 2)
        color: modal.carded ? Theme.glassHigh : "transparent"
        border.width: modal.carded ? 1 : 0
        border.color: Theme.glassEdge
        // Fading out, it takes no finger: an A it sent would land on the page.
        enabled: modal.open
        opacity: modal.open ? 1.0 : 0.0
        scale: modal.open || !modal.carded ? 1.0 : 0.97

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }
        Behavior on scale {
            NumberAnimation {
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }

        Block {}
    }
}
