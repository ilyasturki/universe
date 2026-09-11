import QtQuick
import "../core"
import "../core/Format.js" as Format

Row {
    id: root

    property var game: null

    Repeater {
        model: {
            if (!root.game)
                return [];
            var out = [];
            if (root.game.playTime > 0)
                out.push({ label: "PLAYTIME", value: Format.playTime(root.game.playTime) });
            out.push({ label: "LAST PLAYED", value: Format.lastPlayed(root.game.lastPlayed) });
            if (root.game.playCount > 0)
                out.push({ label: "SESSIONS", value: root.game.playCount.toString() });
            if (root.game.releaseYear > 0)
                out.push({ label: "RELEASED", value: root.game.releaseYear.toString() });
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
