import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: root

    property var tabs: []
    property int currentIndex: 0
    // Runs one past the tabs: the last slot is the search glass, on the pages that have games.
    property int index: 0
    property bool showSearch: true

    readonly property int searchIndex: tabs.length
    readonly property bool onSearch: showSearch && index === searchIndex
    // Past the glass: the running game's badge, while there is one.
    readonly property int badgeIndex: showSearch ? searchIndex + 1 : searchIndex
    readonly property bool onBadge: badge.active && index === badgeIndex
    readonly property int slots: badgeIndex + (badge.active ? 1 : 0)
    readonly property var currentGame: onBadge ? api.allGames.byId(badge.session.id) : null
    readonly property Item menuAnchor: onBadge ? badge : null

    signal tabRequested(int index)
    signal searchRequested()
    signal resumeRequested()
    signal menuRequested(var game, Item anchor)
    signal entered()
    signal dismissed()

    readonly property var hints: onBadge
        ? [ { glyph: "A", label: "Resume" }, { glyph: "≡", label: "Options" }, { glyph: "B", label: "Back" }, { glyph: "LB RB", label: "Tabs" } ]
        : [ { glyph: "A", label: onSearch ? "Search" : "Open" }, { glyph: "B", label: "Back" }, { glyph: "LB RB", label: "Tabs" } ]

    implicitHeight: Theme.dp(Theme.tabBarHeight)

    function enter() {
        index = Math.max(0, Math.min(slots - 1, currentIndex));
        forceActiveFocus();
    }

    // Round trip: the glass follows the last tab, the first tab follows the glass.
    function step(d) {
        var n = slots;
        var next = (index + d + n) % n;
        index = next;
        if (next < searchIndex && next !== currentIndex) {
            Sound.space();
            tabRequested(next);
            return;
        }
        Sound.tick();
    }

    onIndexChanged: frame.retarget()
    onWidthChanged: frame.retarget()
    onCurrentIndexChanged: {
        underline.retarget();
        if (activeFocus && index < searchIndex)
            index = currentIndex;
    }
    // The glass leaves with the page that had it; a cursor on it steps back to the tab.
    onShowSearchChanged: {
        if (!showSearch && index >= searchIndex)
            index = currentIndex;
    }
    onSlotsChanged: {
        if (index >= slots)
            index = currentIndex;
        frame.retarget();
    }

    Keys.onLeftPressed: root.step(-1)
    Keys.onRightPressed: root.step(1)
    Keys.onUpPressed: Sound.edge()
    Keys.onDownPressed: function(event) {
        Sound.panel();
        root.entered();
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (root.onBadge)
                root.resumeRequested();
            else if (root.onSearch)
                root.searchRequested();
            else {
                Sound.panel();
                root.entered();
            }
            return;
        }
        if (api.keys.isMenu(event) && root.onBadge) {
            event.accepted = true;
            if (root.currentGame)
                root.menuRequested(root.currentGame, badge);
            return;
        }
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            Sound.cancel();
            root.dismissed();
            return;
        }
        // Nothing up here owns a game, so the shell must not act on one.
        if (api.keys.isDetails(event) || api.keys.isFilters(event)) {
            event.accepted = true;
            Sound.edge();
            return;
        }
    }

    Item {
        id: frame

        // itemAt() is not a binding source, so the slot is picked by hand — but its
        // geometry stays bound, or the frame keeps whatever the first layout gave it.
        property Item target: null
        readonly property Item pane: root.index < root.searchIndex ? labels : rightSide
        // A circle on the glass, a pill on a label; the badge is a pill of its own.
        readonly property real padX: root.onBadge ? 0 : root.onSearch ? padY : Theme.dp(20)
        readonly property real padY: root.onBadge ? 0 : Theme.dp(10)

        function retarget() {
            target = root.index < root.searchIndex ? tabRepeater.itemAt(root.index) : root.onBadge ? badge : glass;
        }

        x: target ? pane.x + target.x - padX : 0
        y: target ? pane.y + target.y - padY : 0
        width: target ? target.width + padX * 2 : 0
        height: target ? target.height + padY * 2 : 0

        opacity: root.activeFocus ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on x { NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutQuint } }
        Behavior on y { NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutQuint } }
        Behavior on width { NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutQuint } }
        Behavior on height { NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutQuint } }
        Behavior on opacity { NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic } }

        Rectangle {
            anchors.fill: parent
            radius: height / 2
            color: Theme.surface
            visible: !root.onBadge
        }

        FocusRing {
            anchors.fill: parent
            cornerRadius: frame.height / 2
        }
    }

    Row {
        id: labels
        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(Theme.edgeMargin)
        anchors.verticalCenter: parent.verticalCenter
        // The canvas centres the whole tab item — label, 8px gap and 3px bar.
        anchors.verticalCenterOffset: -Math.round(Theme.dp(11) / 2)
        spacing: Theme.dp(44)

        Repeater {
            id: tabRepeater
            model: root.tabs
            onCountChanged: { underline.retarget(); frame.retarget(); }
            onItemAdded: { underline.retarget(); frame.retarget(); }

            Text {
                readonly property bool active: index === root.currentIndex

                text: modelData
                color: active ? Theme.text : Theme.textTab
                font.family: Theme.sans
                font.weight: active ? Font.DemiBold : Font.Medium
                font.pixelSize: Theme.dp(25)

                Behavior on color {
                    ColorAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
                }
            }
        }
    }

    Rectangle {
        id: underline

        // itemAt() is not a binding source, so this is retargeted by hand.
        property Item target: null
        readonly property real baseWidth: Theme.dp(100)

        function retarget() {
            target = (root.currentIndex >= 0 && root.currentIndex < tabRepeater.count)
                     ? tabRepeater.itemAt(root.currentIndex) : null;
        }

        Component.onCompleted: retarget()

        width: baseWidth
        height: Math.max(2, Theme.dp(3))
        color: Theme.text
        antialiasing: true
        // The focus pill stands in for it while it sits on the same tab.
        opacity: root.activeFocus && root.index === root.currentIndex ? 0.0 : 1.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        anchors.top: labels.bottom
        anchors.topMargin: Theme.dp(8)

        x: target ? labels.x + target.x : labels.x
        transform: Scale {
            origin.x: 0
            xScale: underline.target ? underline.target.width / underline.baseWidth : 1

            Behavior on xScale {
                NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutQuint }
            }
        }

        Behavior on x {
            NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutQuint }
        }
    }

    Row {
        id: rightSide
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(Theme.edgeMargin)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(34)

        MenuGlyph {
            id: journalMark

            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(24)
            height: width
            visible: api.screens.pendingJournals.count > 0
            kind: "book"
            tint: Theme.textSecondary

            SequentialAnimation on opacity {
                running: journalMark.visible
                loops: Animation.Infinite
                NumberAnimation { to: 0.35; duration: 900; easing.type: Easing.InOutQuad }
                NumberAnimation { to: 1.0; duration: 900; easing.type: Easing.InOutQuad }
            }
        }

        Canvas {
            id: glass

            readonly property color stroke: root.activeFocus && root.onSearch ? Theme.text : Theme.textTab
            onStrokeChanged: requestPaint()
            onWidthChanged: frame.retarget()

            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(26)
            height: Theme.dp(26)
            visible: opacity > 0.01
            opacity: root.showSearch ? 1.0 : 0.0

            Behavior on opacity {
                NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
            }

            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                var s = width / 24;
                ctx.strokeStyle = stroke;
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

        SessionBadge {
            id: badge
            anchors.verticalCenter: parent.verticalCenter
            focused: root.activeFocus && root.onBadge
            onActiveChanged: frame.retarget()
            onWidthChanged: frame.retarget()
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Theme.clock
            color: Theme.textSecondary
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(22)
        }
    }

    Component.onCompleted: frame.retarget()
}
