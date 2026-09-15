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

    function finish(value) {
        var cb = callback;
        callback = null;
        open = false;
        if (cb)
            cb(value);
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: modal.scrimColor
        opacity: modal.open ? modal.scrimOpacity : 0.0

        Behavior on opacity { Ease { duration: Theme.durQuick } }
    }

    Rectangle {
        id: card

        anchors.centerIn: modal.carded ? parent : undefined
        anchors.fill: modal.carded ? undefined : parent
        radius: Theme.dp(6)
        color: modal.carded ? Theme.card : "transparent"
        opacity: modal.open ? 1.0 : 0.0
        scale: modal.open || !modal.carded ? 1.0 : 0.98

        Behavior on opacity { Ease { duration: Theme.durQuick } }
        Behavior on scale { Ease { duration: Theme.durQuick } }
    }
}
