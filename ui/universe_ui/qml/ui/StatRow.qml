import QtQuick
import "../core"
import "../core/Format.js" as Format

Row {
    id: root

    property var game: null

    readonly property var facts: {
        if (!game)
            return [];
        var out = [];
        if (game.playTime > 0)
            out.push({
                key: "playTime",
                text: Format.playTime(game.playTime) + " played"
            });
        if (game.lastPlayed)
            out.push({
                key: "lastPlayed",
                text: "Last played " + Format.lastPlayed(game.lastPlayed).toLowerCase()
            });
        var status = game.version.status;
        if ((status === "current" || status === "update") && game.installedVersion)
            out.push({
                key: "version",
                text: "Version " + game.installedVersion
            });
        if (status === "current")
            out.push({
                key: "upToDate",
                text: "Up to date"
            });
        return out;
    }
    readonly property string mark: game ? game.updateMark : ""

    spacing: Theme.dp(14)
    visible: facts.length > 0 || mark !== ""

    Text {
        id: line

        anchors.verticalCenter: parent.verticalCenter
        visible: text !== ""
        text: root.facts.map(function (f) {
            return f.text;
        }).join("  ·  ")
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(24)
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: line.visible && markRow.visible
        text: "·"
        color: Theme.textSecondary
        font.family: Theme.sans
        font.pixelSize: Theme.dp(24)
    }

    Row {
        id: markRow

        objectName: "updateMark"
        anchors.verticalCenter: parent.verticalCenter
        visible: root.mark !== ""
        spacing: Theme.dp(10)

        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(10)
            height: width
            radius: width / 2
            color: Theme.accent
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Format.updateMark(root.mark)
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(24)
        }
    }
}
