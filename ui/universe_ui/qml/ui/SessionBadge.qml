import QtQuick
import "../core"

// What is running right now, from Session1's Current property.
Rectangle {
    id: badge

    readonly property var session: api.universe.currentSession
    readonly property bool active: session !== null && session !== undefined && session.title !== undefined
    property int elapsed: 0

    visible: active
    height: Theme.dp(40)
    width: row.width + Theme.dp(32)
    radius: height / 2
    color: Qt.rgba(1, 1, 1, 0.10)
    border.width: 1
    border.color: Theme.surfaceBorder

    onSessionChanged: {
        var started = session && session.started_at ? Date.parse(session.started_at) : NaN;
        elapsed = isNaN(started) ? 0 : Math.max(0, Math.round((Date.now() - started) / 1000));
    }

    Timer {
        interval: 1000
        running: badge.active
        repeat: true
        onTriggered: badge.elapsed += 1
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.dp(10)

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(10)
            height: width
            radius: width / 2
            color: "#5fd48a"

            SequentialAnimation on opacity {
                running: badge.active
                loops: Animation.Infinite
                NumberAnimation { to: 0.3; duration: 900; easing.type: Easing.InOutQuad }
                NumberAnimation { to: 1.0; duration: 900; easing.type: Easing.InOutQuad }
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: (badge.session && badge.session.title ? badge.session.title : "")
                  + " · " + Math.floor(badge.elapsed / 60) + ":" + ("0" + (badge.elapsed % 60)).slice(-2)
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(20)
        }
    }
}
