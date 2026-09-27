import QtQuick
import "../core"

// One labelled row of the Game Hub; the focused card slides to the row's start as the console's do.
Item {
    id: strip

    property string title: ""
    // [{ image, badge, caption, title, body, progress, playIcon, width }]
    property var cards: []
    property int current: 0
    property bool active: false
    property real edge: Theme.dp(Theme.edge)
    property real rightEdge: Theme.dp(Theme.columnRight)

    signal pointed(int index)

    readonly property real cardHeight: Theme.dp(300)
    readonly property real gap: Theme.dp(24)

    function widthOf(i) {
        var c = cards[i];
        return c && c.width ? Theme.dp(c.width) : Theme.dp(504);
    }

    function xOf(i) {
        var x = 0;
        for (var k = 0; k < i; k++)
            x += widthOf(k) + gap;
        return x;
    }

    readonly property real rowWidth: cards.length > 0 ? xOf(cards.length - 1) + widthOf(cards.length - 1) : 0
    readonly property real room: width - edge - rightEdge
    readonly property real scroll: Math.max(0, Math.min(xOf(current), rowWidth - room))

    height: label.height + Theme.dp(16) + cardHeight

    Label {
        id: label
        x: strip.edge
        text: strip.title
        font.pixelSize: Theme.dp(26)
    }

    Item {
        id: row

        x: strip.edge - strip.scroll
        y: label.height + Theme.dp(16)
        width: strip.rowWidth
        height: strip.cardHeight

        Behavior on x {
            NumberAnimation {
                duration: Theme.durMove
                easing.type: Easing.OutCubic
            }
        }

        Repeater {
            model: strip.cards

            HubCard {
                x: strip.xOf(index)
                width: strip.widthOf(index)
                height: strip.cardHeight
                image: modelData.image || ""
                imageFill: modelData.imageFill !== false
                badge: modelData.badge || ""
                caption: modelData.caption || ""
                title: modelData.title || ""
                body: modelData.body || ""
                progress: modelData.progress !== undefined ? modelData.progress : -1
                playIcon: modelData.playIcon === true
                focused: strip.active && index === strip.current
                onPicked: strip.pointed(index)
            }
        }
    }
}
