import QtQuick
import "../core"
import "../core/Format.js" as Format

Rectangle {
    id: badge

    readonly property var session: api.universe.currentSession
    readonly property bool active: session != null && session.title !== undefined
    property bool focused: false
    property int elapsed: 0

    visible: active
    height: Theme.dp(40)
    width: row.width + Theme.dp(32)
    radius: height / 2
    color: focused ? Theme.text : Qt.rgba(1, 1, 1, 0.10)
    border.width: 1
    border.color: Theme.surfaceBorder

    Behavior on color {
        ColorEase {}
    }

    function sync() {
        var started = session && session.started_at ? Date.parse(session.started_at) : NaN;
        elapsed = isNaN(started) ? 0 : Math.max(0, Math.round((Date.now() - started) / 1000));
    }

    onSessionChanged: sync()

    // The count holds while the game is on screen and catches up when the launcher is back.
    Connections {
        target: Theme
        function onCoveredChanged() {
            if (!Theme.covered)
                badge.sync();
        }
    }

    Timer {
        interval: 1000
        running: badge.active && !Theme.covered
        repeat: true
        onTriggered: badge.elapsed += 1
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.dp(10)

        PulseDot {
            anchors.verticalCenter: parent.verticalCenter
            running: badge.active
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: (badge.session && badge.session.title ? badge.session.title : "") + " · " + Format.clockTime(badge.elapsed)
            color: badge.focused ? Theme.onLight : Theme.text
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(20)
        }
    }
}
