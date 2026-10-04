import QtQuick
import Universe
import "../core"
import "../sound"
import "../ui"

// The console's search: Games and Settings as tabs, the query in a bar across the top, the keyboard under it,
// the hits beside the keyboard while typing and over the whole width once it is put away.
FocusScope {
    id: page

    objectName: "searchPage"
    property var shell: null
    property var args: ({})
    signal closeRequested
    focus: true

    property int tab: 0
    property bool typing: true
    property string query: ""
    property int index: 0

    readonly property var settings: api.screens.search
    readonly property var gameHits: {
        var out = [];
        for (var i = 0; i < games.count && out.length < 60; i++) {
            var g = games.get(i);
            if (g)
                out.push(g);
        }
        return out;
    }
    readonly property var settingHits: settings.results
    readonly property int count: tab === 0 ? gameHits.length : settingHits.length
    readonly property bool strip: true
    readonly property var hints: typing ? [
        {
            glyph: "Y",
            label: "Space"
        },
        {
            glyph: "X",
            label: "Clear"
        },
        {
            glyph: "B",
            label: query === "" ? "Back" : "Delete"
        },
        {
            glyph: "Start",
            label: "Results",
            dim: count === 0
        },
        {
            glyph: "LB RB",
            label: tab === 0 ? "Settings" : "Games"
        }
    ] : [
        {
            glyph: "B",
            label: "Keyboard"
        },
        {
            glyph: "A",
            label: "Open"
        },
        {
            glyph: "LB RB",
            label: tab === 0 ? "Settings" : "Games"
        }
    ]

    SearchGames {
        id: games
        sourceModel: api.allGames
        query: page.query
    }

    onQueryChanged: {
        settings.query = query;
        index = 0;
    }
    onIndexChanged: settingRows.index = index

    Component.onCompleted: {
        settings.load();
        settings.query = "";
        panel.built = true;
        panel.reset();
        if (args && args.tab === "settings")
            tab = 1;
    }

    function switchTab(t) {
        if (t === tab) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        tab = t;
        index = 0;
        if (!typing && count === 0)
            toKeys();
    }

    function toResults() {
        if (count === 0) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        typing = false;
        index = 0;
    }

    function toKeys() {
        Sound.play("back");
        typing = true;
    }

    function open(i) {
        if (tab === 0) {
            var g = gameHits[i];
            if (g)
                shell.openGame(g.id);
            return;
        }
        var row = settingHits[i];
        if (!row)
            return;
        if (row.kind === "gamekey") {
            Sound.play("select");
            settings.expand(i);
            return;
        }
        var target = row.target;
        if (!target) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        if (target.page === "runner")
            shell.push("pages/FormPage.qml", {
                runner: target.id,
                key: target.key
            });
        else if (target.page === "module")
            shell.push("pages/FormPage.qml", {
                module: target.id,
                key: target.key
            });
        else if (target.page === "source")
            shell.push("pages/FormPage.qml", {
                source: target.id,
                key: target.key
            });
        else if (target.page === "game")
            shell.push("pages/GameSettingsPage.qml", {
                gameId: target.id,
                key: target.key,
                settingModule: target.module
            });
        else if (target.page === "controller" && target.key)
            shell.push("pages/ControllersPage.qml", {
                key: target.key
            });
        else {
            // The settings page lands the hit once it is up: the shell is held, not read from this page, which it replaces.
            var held = shell;
            held.pop();
            held.push("pages/SettingsPage.qml", {});
            Qt.callLater(function () {
                if (held.topPage && held.topPage.land)
                    held.topPage.land(target);
            });
        }
    }

    Keys.onPressed: function (event) {
        var horizontal = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        if (event.isAutoRepeat && !horizontal && !vertical)
            return;
        if (api.keys.isPrevPage(event) || api.keys.isNextPage(event)) {
            event.accepted = true;
            switchTab(api.keys.isPrevPage(event) ? 0 : 1);
            return;
        }
        if (typing) {
            event.accepted = true;
            if (horizontal)
                panel.move(0, event.key === Qt.Key_Left ? -1 : 1);
            else if (vertical)
                panel.move(event.key === Qt.Key_Up ? -1 : 1, 0);
            else if (api.keys.isAccept(event))
                panel.press();
            else if (api.keys.isCancel(event)) {
                if (query === "") {
                    event.accepted = false;
                    return;
                }
                Sound.play("type");
                query = query.slice(0, -1);
            } else if (api.keys.isDetails(event)) {
                Sound.play(query === "" ? "edge" : "type");
                query = "";
            } else if (api.keys.isFilters(event)) {
                Sound.play("type");
                query += " ";
            } else if (api.keys.isMenu(event))
                toResults();
            else
                event.accepted = false;
            return;
        }
        event.accepted = true;
        var d = event.key === Qt.Key_Left || event.key === Qt.Key_Up ? -1 : 1;
        if (tab === 0 && horizontal)
            index = Sound.stepped(index, d, count);
        else if (tab === 0 && vertical) {
            var next = index + d * gameGrid.columns;
            if (next >= 0 && next < count) {
                Sound.play("tick");
                index = next;
            } else if (d < 0)
                toKeys();
            else
                Sound.play("edge");
        } else if (tab === 1 && vertical) {
            if (d < 0 && index === 0)
                toKeys();
            else
                index = Sound.stepped(index, d, count);
        } else if (api.keys.isAccept(event))
            open(index);
        else if (api.keys.isCancel(event))
            toKeys();
        else
            event.accepted = false;
    }

    Backdrop {
        anchors.fill: parent
    }

    Row {
        id: tabs
        x: Theme.dp(173)
        y: Theme.dp(65) - height / 2
        spacing: Theme.dp(66)

        Repeater {
            model: ["Games", "Settings"]

            Label {
                text: modelData
                color: index === page.tab ? Theme.text : Theme.textMuted
                font.weight: index === page.tab ? Font.Normal : Font.Light
                font.pixelSize: Theme.dp(Theme.fontTitle)

                Behavior on color {
                    ColorAnimation {
                        duration: Theme.durFocus
                    }
                }

                Touch {
                    anchors.margins: -Theme.dp(12)
                    direct: true
                    action: ""
                    onPicked: page.switchTab(index)
                }
            }
        }
    }

    Rectangle {
        id: field

        x: Theme.dp(173)
        y: Theme.dp(118)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: Theme.dp(70)
        color: Qt.rgba(0, 0, 0, 0.4)

        Glyph {
            id: lens
            x: Theme.dp(22)
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(32)
            height: width
            kind: "search"
        }

        Label {
            id: queryText
            x: lens.x + lens.width + Theme.dp(16)
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - x - Theme.dp(40)
            text: page.query !== "" ? page.query : page.tab === 0 ? "Search your games" : "Search every setting"
            color: page.query !== "" ? Theme.text : Theme.textMuted
            elide: Text.ElideLeft
            font.weight: page.query !== "" ? Font.DemiBold : Font.Light
            font.pixelSize: Theme.dp(Theme.fontTitle)
        }

        Rectangle {
            x: queryText.x + (page.query !== "" ? Math.min(queryText.implicitWidth, queryText.width) + Theme.dp(2) : 0)
            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(2)
            height: Theme.dp(38)
            color: Theme.text
            visible: caret.on && page.typing
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: Theme.dp(2)
            color: page.typing ? Theme.text : Theme.hairline
        }
    }

    Timer {
        id: caret
        property bool on: true
        interval: 560
        running: page.typing
        repeat: true
        onTriggered: on = !on
    }

    KeyPanel {
        id: panel

        x: field.x
        y: field.y + field.height + Theme.dp(66) + (page.typing ? 0 : Theme.dp(30))
        opacity: page.typing ? 1.0 : 0.0
        visible: opacity > 0.01
        cursorShown: page.typing && page.activeFocus

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
            }
        }
        Behavior on y {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }

        onTyped: function (value) {
            if (page.query.length < 80)
                page.query += value;
            else
                Sound.play("edge");
        }
        onBackspaced: page.query = page.query.slice(0, -1)
        onAccepted: page.toResults()
        onEscapedUp: Sound.play("edge")
    }

    Item {
        id: results

        readonly property real leftTyping: panel.x + panel.width + Theme.dp(60)
        readonly property real leftFull: field.x

        x: page.typing ? leftTyping : leftFull
        y: field.y + field.height + Theme.dp(50)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: parent.height - y - Theme.dp(90)
        clip: true

        Behavior on x {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }

        Label {
            visible: page.count === 0
            text: page.query === "" ? (page.tab === 0 ? "Type a title." : "Type a setting's name, what it does, or its value.") : "Nothing matches “" + page.query + "”."
            color: Theme.textMuted
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        GridView {
            id: gameGrid

            readonly property int columns: Math.max(2, Math.floor(width / Theme.dp(page.typing ? 230 : 250)))

            anchors.fill: parent
            visible: page.tab === 0
            model: page.tab === 0 ? page.gameHits : []
            cellWidth: width / columns
            cellHeight: cellWidth + Theme.dp(56)
            currentIndex: page.index
            interactive: false
            clip: true
            highlightRangeMode: GridView.ApplyRange
            preferredHighlightBegin: 0
            preferredHighlightEnd: height
            highlightMoveDuration: Theme.durScroll

            delegate: Item {
                id: cell

                readonly property bool focused: !page.typing && index === page.index && page.activeFocus

                width: gameGrid.cellWidth
                height: gameGrid.cellHeight

                TileArt {
                    id: art
                    x: Theme.dp(10)
                    y: Theme.dp(10)
                    width: parent.width - Theme.dp(20)
                    height: width
                    game: modelData
                    radius: Theme.dp(10)

                    FocusFrame {
                        shown: cell.focused
                        radius: Theme.dp(10)
                    }
                }

                Label {
                    anchors.top: art.bottom
                    anchors.topMargin: Theme.dp(12)
                    x: art.x
                    width: art.width
                    text: modelData.title
                    elide: Text.ElideRight
                    color: cell.focused ? Theme.text : Theme.textSecondary
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }

                Touch {
                    current: cell.focused
                    onPicked: {
                        Sound.play("tick");
                        page.typing = false;
                        page.index = index;
                    }
                }
            }
        }

        SettingsRows {
            id: settingRows

            anchors.fill: parent
            anchors.leftMargin: Theme.dp(16)
            anchors.rightMargin: Theme.dp(16)
            visible: page.tab === 1
            shell: page.shell
            focus: false
            model: page.tab === 1 ? page.settingHits.map(function (r) {
                var out = Object.assign({}, r);
                out.icon = r.image || (r.kind === "section" ? "settings" : "");
                out.iconSlot = r.kind === "gamerow" || r.kind === "game";
                if (r.advanced)
                    out.detail = "Advanced" + (r.detail ? " · " + r.detail : "");
                return out;
            }) : []
            cursorShown: !page.typing && page.activeFocus
            onPointed: {
                page.typing = false;
                page.index = settingRows.index;
                page.forceActiveFocus();
            }
        }
    }
}
