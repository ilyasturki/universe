import QtQuick
import Universe
import "../core"
import "../sound"

FocusScope {
    id: overlay

    clip: true

    property bool open: false
    property string query: ""
    property bool resultsFocused: false

    signal closeRequested

    readonly property bool typing: !resultsFocused

    readonly property var currentGame: !typing && results.currentIndex >= 0 && matches.count > 0 ? matches.get(results.currentIndex) : null
    readonly property Item menuAnchor: typing || !results.currentItem ? null : results.currentItem.artItem

    readonly property var hints: typing ? [
        {
            glyph: "A",
            label: "Type"
        },
        {
            glyph: "X",
            label: "Backspace"
        },
        {
            glyph: "B",
            label: "Close"
        }
    ] : [
        {
            glyph: "A",
            label: "Launch"
        },
        {
            glyph: "X",
            label: "Details"
        },
        {
            glyph: "B",
            label: "Close"
        }
    ]

    readonly property real sheetInner: Math.min(Theme.dp(880), width - Theme.dp(280))
    readonly property real sheetPad: Theme.dp(28)
    readonly property real fieldHeight: Theme.dp(66)
    readonly property real cardHeight: Math.max(Theme.dp(150), Math.min(Theme.dp(300), resultsArea.height - Theme.dp(96)))
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
        results.currentIndex = Sound.stepped(results.currentIndex, d, matches.count);
    }

    function kbMove(dRow, dCol) {
        keyboard.move(dRow, dCol) ? Sound.kbtick() : Sound.edge();
    }

    SearchGames {
        id: matches
        sourceModel: api.allGames
        query: overlay.query
    }

    onOpenChanged: if (open)
        resultsFocused = false

    onQueryChanged: {
        results.currentIndex = 0;
        results.contentX = -results.leftMargin;
        if (matches.count === 0)
            resultsFocused = false;
    }

    Keys.onLeftPressed: overlay.typing ? overlay.kbMove(0, -1) : overlay.moveResult(-1)
    Keys.onRightPressed: overlay.typing ? overlay.kbMove(0, 1) : overlay.moveResult(1)

    Keys.onUpPressed: !overlay.typing ? Sound.edge() : keyboard.rowIndex === 0 ? overlay.toResults() : overlay.kbMove(-1, 0)
    Keys.onDownPressed: overlay.typing ? overlay.kbMove(1, 0) : overlay.toKeyboard()

    Keys.onPressed: function (event) {
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            if (!event.isAutoRepeat) {
                Sound.cancel();
                overlay.closeRequested();
            }
        } else if (api.keys.isPrevPage(event) || api.keys.isNextPage(event)) {
            event.accepted = true;
            if (!event.isAutoRepeat)
                Sound.edge();
        } else if (api.keys.isAccept(event) && overlay.typing) {
            event.accepted = true;
            keyboard.press();
        } else if (api.keys.isDetails(event) && overlay.typing) {
            event.accepted = true;
            Sound.backspace();
            overlay.query = overlay.query.slice(0, -1);
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(0.055, 0.059, 0.075, 0.91)
        opacity: overlay.open ? 1.0 : 0.0

        Behavior on opacity {
            Ease {
                duration: Theme.durView
            }
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
            Ease {
                duration: Theme.durView
            }
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
            cacheBuffer: overlay.cardWidth * 3
            highlightRangeMode: ListView.ApplyRange
            preferredHighlightBegin: (width - overlay.cardWidth) / 2
            preferredHighlightEnd: preferredHighlightBegin + overlay.cardWidth
            highlightMoveDuration: Theme.durView

            leftMargin: Math.max(Theme.dp(80), (width - (matches.count * overlay.cardWidth + Math.max(0, matches.count - 1) * spacing)) / 2)
            rightMargin: Theme.dp(80)

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
            Ease {
                duration: Theme.durView
                easing.type: Easing.OutQuint
            }
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

            MenuGlyph {
                id: glass
                anchors.left: parent.left
                anchors.leftMargin: Theme.dp(26)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(26)
                height: Theme.dp(26)
                kind: "search"
                tint: "#f2f3f5"
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
                Ease {
                    duration: Theme.durQuick
                }
            }

            onCharEntered: function (value) {
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
