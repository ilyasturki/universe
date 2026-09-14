import QtQuick
import Universe
import "../core"
import "../sound"

// The global search: a sheet that rises over whatever page is behind it, with
// the field and the keyboard on it and the matches in a row above.
FocusScope {
    id: overlay

    // The sheet parks below the bottom edge when closed.
    clip: true

    property bool open: false
    property string query: ""
    property bool resultsFocused: false

    signal closeRequested()

    readonly property bool typing: !resultsFocused
    // Read by tools/shot: with the keyboard up, A types instead of launching.
    readonly property bool ownsAccept: typing

    readonly property var currentGame: !typing && results.currentIndex >= 0 && matches.count > 0
                                       ? matches.get(results.currentIndex) : null
    readonly property Item menuAnchor: typing || !results.currentItem ? null : results.currentItem.artItem

    readonly property var hints: typing
        ? [ { glyph: "A", label: "Type" },
            { glyph: "X", label: "Backspace" },
            { glyph: "B", label: "Close" } ]
        : [ { glyph: "A", label: "Launch" },
            { glyph: "X", label: "Details" },
            { glyph: "B", label: "Close" } ]

    readonly property real sheetInner: Math.min(Theme.dp(880), width - Theme.dp(280))
    readonly property real sheetPad: Theme.dp(28)
    readonly property real fieldHeight: Theme.dp(66)
    // The cover, its title and the gaps around them fill the band over the sheet.
    readonly property real cardHeight: Math.max(Theme.dp(150),
                                                Math.min(Theme.dp(300), resultsArea.height - Theme.dp(96)))
    readonly property real cardWidth: cardHeight / 1.5

    function toResults() {
        if (matches.count === 0) {
            Sound.edge();
            return;
        }
        Sound.panel();
        resultsFocused = true;
    }

    function toKeyboard() {
        Sound.panel();
        resultsFocused = false;
    }

    function moveResult(d) {
        var next = results.currentIndex + d;
        if (next < 0 || next >= matches.count) {
            Sound.edge();
            return;
        }
        Sound.tick();
        results.currentIndex = next;
    }

    function kbMove(dRow, dCol) {
        keyboard.move(dRow, dCol) ? Sound.kbtick() : Sound.edge();
    }

    SearchGames {
        id: matches
        sourceModel: api.allGames
        query: overlay.query
    }

    // The query and the key under the cursor keep, but search always opens typing.
    onOpenChanged: if (open) resultsFocused = false

    onQueryChanged: {
        results.currentIndex = 0;
        results.contentX = -results.leftMargin;
        if (matches.count === 0)
            resultsFocused = false;
    }

    Keys.onLeftPressed: overlay.typing ? overlay.kbMove(0, -1) : overlay.moveResult(-1)
    Keys.onRightPressed: overlay.typing ? overlay.kbMove(0, 1) : overlay.moveResult(1)

    Keys.onUpPressed: function(event) {
        if (!overlay.typing) {
            Sound.edge();
            return;
        }
        if (keyboard.rowIndex === 0) {
            overlay.toResults();
            return;
        }
        overlay.kbMove(-1, 0);
    }

    Keys.onDownPressed: function(event) {
        if (overlay.typing) {
            overlay.kbMove(1, 0);
            return;
        }
        overlay.toKeyboard();
    }

    Keys.onPressed: function(event) {
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            if (event.isAutoRepeat)
                return;
            Sound.cancel();
            overlay.closeRequested();
            return;
        }
        // Search sits over the page it was opened from; the tabs stay put.
        if (api.keys.isPrevPage(event) || api.keys.isNextPage(event)) {
            event.accepted = true;
            if (!event.isAutoRepeat)
                Sound.edge();
            return;
        }
        if (!overlay.typing)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            keyboard.press();
            return;
        }
        if (api.keys.isDetails(event)) {
            event.accepted = true;
            Sound.backspace();
            overlay.query = overlay.query.slice(0, -1);
            return;
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(0.055, 0.059, 0.075, 0.91)
        opacity: overlay.open ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
        }
    }

    Item {
        id: resultsArea

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(Theme.tabBarHeight + 16)
        anchors.bottom: sheet.top
        anchors.left: parent.left
        anchors.right: parent.right
        opacity: overlay.open ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
        }

        Text {
            anchors.centerIn: parent
            visible: matches.count === 0
            text: overlay.query === "" ? "" : "No games match that."
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(26)
        }

        ListView {
            id: results

            anchors.fill: parent
            visible: matches.count > 0

            model: matches
            orientation: ListView.Horizontal
            spacing: Theme.dp(30)
            interactive: false
            keyNavigationEnabled: false
            highlightFollowsCurrentItem: false
            cacheBuffer: overlay.cardWidth * 3

            leftMargin: Math.max(Theme.dp(80),
                                 (width - (matches.count * overlay.cardWidth
                                           + Math.max(0, matches.count - 1) * spacing)) / 2)
            rightMargin: Theme.dp(80)

            function slideToCurrent() {
                if (width <= 0 || contentWidth <= width)
                    return;
                var step = overlay.cardWidth + spacing;
                var target = currentIndex * step - (width - overlay.cardWidth) / 2;
                var maxX = Math.max(0, contentWidth + rightMargin - width);
                contentX = Math.max(-leftMargin, Math.min(target, maxX));
            }

            onCurrentIndexChanged: slideToCurrent()

            Behavior on contentX {
                NumberAnimation { duration: Theme.durView; easing.type: Easing.OutQuint }
            }

            delegate: Item {
                id: card

                readonly property bool selected: ListView.isCurrentItem && !overlay.typing
                property alias artItem: art

                width: overlay.cardWidth
                height: results.height

                Column {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.dp(16)

                    CoverCard {
                        id: art
                        width: overlay.cardWidth
                        height: overlay.cardHeight
                        game: model
                        selected: card.selected
                        selectedScale: 1.0
                        idleScale: 0.94
                        cornerRadius: Theme.dp(14)
                        ringOpacity: overlay.typing ? Theme.ringIdle : 1.0
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: overlay.cardWidth
                        horizontalAlignment: Text.AlignHCenter
                        text: model.title
                        color: card.selected ? Theme.text : Theme.textSecondary
                        font.family: Theme.sans
                        font.weight: card.selected ? Font.DemiBold : Font.Medium
                        font.pixelSize: Theme.dp(23)
                        elide: Text.ElideRight
                    }
                }
            }
        }
    }

    Item {
        id: sheet

        anchors.left: parent.left
        anchors.right: parent.right
        height: overlay.sheetPad * 2 + overlay.fieldHeight + Theme.dp(22) + keyboard.height
        y: overlay.open ? parent.height - height : parent.height

        Behavior on y {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutQuint }
        }

        Rectangle {
            anchors.fill: parent
            anchors.bottomMargin: -Theme.dp(30)
            radius: Theme.dp(30)
            color: Qt.rgba(0.071, 0.075, 0.094, 1.0)
            border.width: 1
            border.color: Theme.surfaceBorder
        }

        Item {
            id: field

            anchors.top: parent.top
            anchors.topMargin: overlay.sheetPad
            anchors.horizontalCenter: parent.horizontalCenter
            width: overlay.sheetInner
            height: overlay.fieldHeight

            Rectangle {
                anchors.fill: parent
                radius: Theme.dp(16)
                color: Theme.surface
            }

            Canvas {
                id: glass
                anchors.left: parent.left
                anchors.leftMargin: Theme.dp(26)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(26)
                height: Theme.dp(26)

                onPaint: {
                    var ctx = getContext("2d");
                    ctx.reset();
                    var s = width / 24;
                    ctx.strokeStyle = "#f2f3f5";
                    ctx.lineWidth = 2 * s;
                    ctx.lineCap = "round";
                    ctx.beginPath();
                    ctx.arc(11 * s, 11 * s, 7 * s, 0, Math.PI * 2);
                    ctx.stroke();
                    ctx.beginPath();
                    ctx.moveTo(16.5 * s, 16.5 * s);
                    ctx.lineTo(21 * s, 21 * s);
                    ctx.stroke();
                }
            }

            Text {
                id: queryText
                anchors.left: glass.right
                anchors.leftMargin: Theme.dp(18)
                anchors.verticalCenter: parent.verticalCenter
                text: overlay.query
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(27)
            }

            Text {
                visible: overlay.query === ""
                anchors.left: glass.right
                anchors.leftMargin: Theme.dp(18)
                anchors.verticalCenter: parent.verticalCenter
                text: "Search your library"
                color: Theme.textMuted
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(27)
            }

            Rectangle {
                anchors.left: queryText.right
                anchors.leftMargin: Theme.dp(14)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(3)
                height: Theme.dp(30)
                color: Theme.text
                visible: overlay.open && overlay.typing && caret.on
            }

            Text {
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(26)
                anchors.verticalCenter: parent.verticalCenter
                text: matches.count + " of " + api.allGames.count
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
            }
        }

        Timer {
            id: caret
            property bool on: true

            interval: 560
            running: overlay.open && overlay.typing
            repeat: true
            onTriggered: on = !on
        }

        VirtualKeyboard {
            id: keyboard

            anchors.top: field.bottom
            anchors.topMargin: Theme.dp(22)
            anchors.horizontalCenter: parent.horizontalCenter
            width: overlay.sheetInner
            height: implicitHeight
            keyHeight: Theme.dp(52)
            keyGap: Theme.dp(9)
            opacity: overlay.typing ? 1.0 : 0.55

            Behavior on opacity {
                NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
            }

            onCharEntered: function(value) {
                Sound.type();
                overlay.query += value;
            }
            onBackspaced: {
                Sound.backspace();
                overlay.query = overlay.query.slice(0, -1);
            }
            onCleared: {
                Sound.backspace();
                overlay.query = "";
            }
        }
    }
}
