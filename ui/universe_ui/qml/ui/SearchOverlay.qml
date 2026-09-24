import QtQuick
import Universe
import "../core"
import "../core/Format.js" as Format
import "../sound"
import "Sections.js" as Sections

// The tab bar's search: the games as covers, the settings that match under them, the keyboard at the bottom while typing.
FocusScope {
    id: overlay

    clip: true

    property bool open: false
    property string query: ""
    // "keys" | "games" | "settings"
    property string zone: "keys"

    signal closeRequested
    signal settingRequested(var target)

    readonly property var search: api.screens.search
    readonly property bool typing: zone === "keys"
    readonly property bool hasGames: matches.count > 0
    // The games are covers already: the index's own game entries stay out, and a game's title alone lists no settings.
    readonly property var settingRows: {
        var out = [], all = search.results;
        for (var i = 0; i < all.length; i++)
            if (all[i].kind !== "game")
                out.push(i);
        return out;
    }
    readonly property bool hasSettings: query.trim() !== "" && !search.titleOnly && settingRows.length > 0
    // While typing the covers keep the room and the settings are one line under them; ▲ lowers the keyboard and lists them.
    readonly property bool previewing: typing && hasGames && hasSettings
    readonly property string preview: {
        var seen = [], all = search.results, more = false;
        for (var i = 0; i < all.length; i++) {
            if (all[i].kind === "game" || seen.indexOf(all[i].label) >= 0)
                continue;
            if (seen.length === 3) {
                more = true;
                break;
            }
            seen.push(all[i].label);
        }
        return "Settings: " + seen.join(", ") + (more ? "…" : "");
    }

    readonly property var session: api.universe.currentSession
    readonly property var currentGame: zone === "games" && results.currentIndex >= 0 && matches.count > 0 ? matches.get(results.currentIndex) : null
    readonly property Item menuAnchor: zone !== "games" || !results.currentItem ? null : results.currentItem.artItem

    readonly property var hints: typing ? (api.keys.mode === "keyboard" ? [
            {
                glyph: "A",
                label: "To the results"
            },
            {
                glyph: "B",
                label: "Close"
            }
        ] : [
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
        ]) : zone === "settings" ? [
        {
            glyph: "A",
            label: settingsList.currentRow && settingsList.currentRow.kind === "gamekey" ? (settingsList.currentRow.expanded ? "Collapse" : "Expand") : "Open"
        },
        {
            glyph: "B",
            label: "Close"
        }
    ] : [
        {
            glyph: "A",
            label: Format.playLabel(currentGame, session && session.id !== undefined ? session.id : "")
        },
        {
            glyph: "Start",
            label: "More"
        },
        {
            glyph: "B",
            label: "Close"
        }
    ]

    readonly property real sheetInner: Math.min(Theme.dp(880), width - Theme.dp(280))
    readonly property real sheetPad: Theme.dp(28)
    readonly property real fieldHeight: Theme.dp(66)
    readonly property real cardHeight: Math.max(Theme.dp(150), Math.min(Theme.dp(300), gamesZone.height - Theme.dp(96)))
    readonly property real cardWidth: cardHeight / 1.5

    Component.onCompleted: search.sections = Sections.list.map(function (s) {
        return {
            id: s.id,
            label: s.name,
            icon: s.icon
        };
    })

    function goTo(next) {
        Sound.panel();
        zone = next;
        claim();
    }

    // The overlay keeps the keys for the keyboard and the covers; the settings list takes them while it has the cursor.
    function claim() {
        if (zone === "settings") {
            settingsList.forceActiveFocus();
            return;
        }
        settingsList.focus = false;
        overlay.forceActiveFocus();
    }

    // Up from the keys reaches the nearest list: the settings sit between the covers and the keyboard.
    function toResults() {
        if (hasSettings)
            goTo("settings");
        else if (hasGames)
            goTo("games");
        else
            Sound.edge();
    }

    function toKeyboard() {
        goTo("keys");
    }

    function moveResult(d) {
        results.currentIndex = Sound.stepped(results.currentIndex, d, matches.count);
    }

    function kbMove(dRow, dCol) {
        keyboard.move(dRow, dCol) ? Sound.kbtick() : Sound.edge();
    }

    function activate(index, row) {
        if (row.kind === "gamekey") {
            Sound.panel();
            search.expand(index);
            return;
        }
        Sound.enter();
        overlay.settingRequested(row.target);
    }

    SearchGames {
        id: matches
        sourceModel: api.allGames
        query: overlay.query
    }

    onActiveFocusChanged: if (activeFocus)
        claim()

    onOpenChanged: {
        if (!open)
            return;
        zone = "keys";
        search.load();
    }

    onQueryChanged: {
        search.query = query;
        results.currentIndex = 0;
        results.contentX = -results.leftMargin;
        if (zone !== "keys" && (zone === "games" ? !hasGames : !hasSettings))
            goTo("keys");
    }

    Keys.onLeftPressed: overlay.typing ? overlay.kbMove(0, -1) : overlay.moveResult(-1)
    Keys.onRightPressed: overlay.typing ? overlay.kbMove(0, 1) : overlay.moveResult(1)

    Keys.onUpPressed: !overlay.typing ? Sound.edge() : keyboard.rowIndex === 0 ? overlay.toResults() : overlay.kbMove(-1, 0)
    Keys.onDownPressed: overlay.typing ? overlay.kbMove(1, 0) : overlay.hasSettings ? overlay.goTo("settings") : overlay.toKeyboard()

    // The mouse on a card or a key: the focus goes to that part of the overlay, without the pad's panel sound.
    function pointToResult(index) {
        zone = "games";
        claim();
        results.currentIndex = index;
    }

    Keys.onPressed: function (event) {
        if (overlay.typing && keyboard.typed(event)) {
            event.accepted = true;
        } else if (api.keys.isCancel(event)) {
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

        // Keeps the mouse off the page beneath.
        HoverHandler {}
        TapHandler {}
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
            visible: !overlay.hasGames && !overlay.hasSettings && overlay.query.trim() !== ""
            text: "Nothing matches that."
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(26)
        }

        Item {
            id: gamesZone

            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: !overlay.hasGames ? 0 : overlay.previewing ? parent.height - Theme.dp(48) : overlay.hasSettings ? parent.height * 0.52 : parent.height

            Behavior on height {
                Ease {
                    duration: Theme.durView
                }
            }

            ListView {
                id: results

                anchors.fill: parent
                visible: overlay.hasGames

                Wheel {
                    horizontal: true
                    step: overlay.cardWidth + results.spacing
                }

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

                    readonly property bool selected: ListView.isCurrentItem && overlay.zone === "games"
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
                            ringOpacity: overlay.zone !== "games" ? Theme.ringIdle : 1.0
                            pointable: true
                            current: card.selected
                            onPicked: overlay.pointToResult(index)
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

        Text {
            anchors.top: gamesZone.bottom
            anchors.horizontalCenter: parent.horizontalCenter
            width: overlay.sheetInner
            horizontalAlignment: Text.AlignHCenter
            visible: overlay.previewing
            text: overlay.preview
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(22)
            elide: Text.ElideRight
        }

        SettingsCards {
            id: settingsList

            anchors.top: gamesZone.bottom
            anchors.topMargin: overlay.hasGames ? Theme.dp(12) : 0
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.dp(20)
            anchors.horizontalCenter: parent.horizontalCenter
            width: overlay.sheetInner
            columns: 1
            compact: true
            rows: overlay.search.results
            groups: overlay.hasSettings ? [
                {
                    title: "",
                    rows: overlay.settingRows
                }
            ] : []
            opacity: overlay.hasSettings && !overlay.previewing ? 1.0 : 0.0
            visible: opacity > 0.01

            Behavior on opacity {
                Ease {
                    duration: Theme.durQuick
                }
            }

            onActivated: function (index, row) {
                overlay.activate(index, row);
            }
            onPointed: overlay.zone = "settings"
            onEscapedUp: overlay.hasGames ? overlay.goTo("games") : Sound.edge()
            onEscapedDown: overlay.toKeyboard()
            onEscapedLeft: Sound.edge()
        }
    }

    // Down while typing; out of the way while a list has the cursor, so the lists get the whole height.
    Item {
        id: sheet

        anchors.left: parent.left
        anchors.right: parent.right
        height: overlay.sheetPad * 2 + overlay.fieldHeight + Theme.dp(22) + keyboard.height
        y: !overlay.open ? parent.height : overlay.typing ? parent.height - height : parent.height - overlay.sheetPad - overlay.fieldHeight - Theme.dp(16)

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
                text: "Search games and settings"
                color: Theme.textMuted
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(27)
            }

            Rectangle {
                anchors.left: queryText.right
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(3)
                height: Theme.dp(30)
                color: Theme.text
                visible: overlay.open && overlay.typing && caret.on
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
            onDone: overlay.toResults()
            onPointed: {
                overlay.zone = "keys";
                overlay.claim();
            }
        }
    }
}
