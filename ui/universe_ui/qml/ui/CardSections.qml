import QtQuick
import "../core"
import "../sound"

// A form's cards as a sidebar beside the picked card's rows: `sections` ([{ name, icon, group }]) names `groups` one for one.
// The cursor sits in one `zone`, "side" or "rows"; A or Right enters the rows, B or Left comes back, LT / RT step the cards.
// Fills the page, so its `cards` sit in page coordinates for an editor or a menu; the columns run from `columnsTop` to `floor`.
FocusScope {
    id: view

    objectName: "cardSections"

    property var rows: []
    property var groups: []
    property var sections: []
    property bool dimmed: false
    property real columnsTop: 0
    property real floor: height
    property real sideMargin: Theme.dp(90)
    property real sideWidth: Theme.dp(300)

    property int section: 0
    property string zone: "side"
    readonly property bool inRows: zone === "rows"
    readonly property var shown: section < groups.length ? [groups[section]] : []
    readonly property var currentRow: cards.currentRow
    readonly property alias cards: cards
    readonly property alias side: side

    signal activated(int index, var row)
    signal cancelled

    readonly property real mainRoom: width - sideMargin * 2 - sideWidth - Theme.dp(56)
    readonly property real mainWidth: Math.min(mainRoom, Theme.dp(1280))
    readonly property real mainX: sideMargin + sideWidth + Theme.dp(56) + (mainRoom - mainWidth) / 2

    function reset() {
        cards.reset();
    }

    // The cursor on row `i` of the flat list: its card picked, the rows zone.
    function landOn(i) {
        var k = groups.findIndex(function (g) {
            return g.rows.indexOf(i) >= 0;
        });
        section = k >= 0 ? k : 0;
        cards.index = i;
        zone = "rows";
    }

    function pick(i) {
        section = i;
        cards.reset();
    }

    function stepSection(d) {
        var n = sections.length;
        if (n === 0)
            return;
        pick((section + d + n) % n);
        Sound.tick();
    }

    // Advanced turned off while on a card that went: the last one that stays.
    onGroupsChanged: if (section >= groups.length)
        section = Math.max(0, groups.length - 1)

    Keys.onPressed: function (event) {
        if (api.keys.isPageUp(event) || api.keys.isPageDown(event))
            event.accepted = true;
    }

    Keys.onReleased: function (event) {
        if (event.isAutoRepeat)
            return;
        var d = api.keys.isPageUp(event) ? -1 : api.keys.isPageDown(event) ? 1 : 0;
        if (!d)
            return;
        event.accepted = true;
        view.stepSection(d);
    }

    SectionList {
        id: side

        x: view.sideMargin
        y: view.columnsTop
        width: view.sideWidth
        focus: view.zone === "side"
        sections: view.sections
        current: view.section
        opacity: view.dimmed ? 0.5 : 1.0

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }

        // The mouse gives the list the focus itself: the zone follows.
        onActiveFocusChanged: if (activeFocus)
            view.zone = "side"
        onRequested: function (i) {
            view.pick(i);
        }
        onEntered: view.zone = "rows"
        onEscapedUp: Sound.edge()
        onCancelled: view.cancelled()
    }

    SettingsCards {
        id: cards

        x: view.mainX
        y: view.columnsTop
        width: view.mainWidth
        height: view.floor - y
        focus: view.zone === "rows"
        columns: 1
        compact: true
        rows: view.rows
        groups: view.shown
        dimmed: view.dimmed

        onPointed: view.zone = "rows"
        onActivated: function (index, row) {
            view.activated(index, row);
        }
        onEscapedUp: Sound.edge()
        onEscapedDown: Sound.edge()
        onEscapedLeft: {
            Sound.panel();
            view.zone = "side";
        }

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                Sound.cancel();
                view.zone = "side";
            }
        }
    }
}
