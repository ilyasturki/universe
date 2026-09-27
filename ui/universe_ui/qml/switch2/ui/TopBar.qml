import QtQuick
import "../core"

Item {
    id: bar

    readonly property var battery: {
        var sources = api.power.sources;
        for (var i = 0; i < sources.length; i++)
            if (sources[i].kind === "system")
                return sources[i];
        return null;
    }

    implicitHeight: Theme.dp(170)

    Rectangle {
        id: avatar

        x: Theme.dp(75)
        y: Theme.dp(69)
        width: Theme.dp(90)
        height: width
        radius: width / 2
        color: Theme.disc
        border.width: Theme.dp(3)
        border.color: Theme.discEdge

        Image {
            anchors.fill: parent
            anchors.margins: Theme.dp(14)
            source: Qt.resolvedUrl("../../../../icons/hicolor/scalable/apps/universe-ui.svg")
            fillMode: Image.PreserveAspectFit
            sourceSize.width: 128
            sourceSize.height: 128
            smooth: true
            mipmap: true
        }
    }

    Row {
        id: status

        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(72)
        y: Theme.dp(83)
        height: clockLabel.height
        spacing: Theme.dp(28)

        Label {
            id: clockLabel
            text: Theme.clock
            font.family: Theme.clockSans
            font.pixelSize: Theme.dp(Theme.fontClock)
            font.letterSpacing: Theme.dp(2)
        }

        Glyph {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(36)
            height: width
            visible: api.network.kind !== ""
            kind: api.network.kind
            level: api.network.bars
            stroke: 2.2
            tint: Theme.text
        }

        Battery {
            anchors.verticalCenter: parent.verticalCenter
            visible: bar.battery !== null
            percent: bar.battery ? bar.battery.percent : 0
            charging: bar.battery ? bar.battery.charging : false
        }
    }
}
