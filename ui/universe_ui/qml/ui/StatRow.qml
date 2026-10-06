import QtQuick
import "../core"
import "../core/Format.js" as Format

Row {
    id: root

    property var game: null

    visible: stats.count > 0

    Repeater {
        id: stats

        model: {
            if (!root.game)
                return [];
            var out = [];
            if (root.game.playTime > 0)
                out.push({
                    label: "PLAY TIME",
                    value: Format.playTime(root.game.playTime)
                });
            if (root.game.lastPlayed)
                out.push({
                    label: "LAST PLAYED",
                    value: Format.lastPlayed(root.game.lastPlayed)
                });
            if (root.game.playCount > 0)
                out.push({
                    label: "SESSIONS",
                    value: root.game.playCount.toString()
                });
            return out;
        }

        Column {
            spacing: Theme.dp(8)

            CapsLabel {
                text: modelData.label
                tracking: 0.11
            }
            Text {
                text: modelData.value
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(34)
            }
        }
    }
}
