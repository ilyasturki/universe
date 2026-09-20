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
    readonly property int badgeIndex: showSearch ? searchIndex + 1 : searchIndex
    readonly property bool onBadge: badge.active && index === badgeIndex
    readonly property int slots: badgeIndex + (badge.active ? 1 : 0)
    readonly property var currentGame: onBadge ? api.allGames.byId(badge.session.id) : null
    readonly property Item menuAnchor: onBadge ? badge : null

    signal tabRequested(int index)
    signal searchRequested
    signal resumeRequested
    signal menuRequested(var game, Item anchor)
    signal entered
    signal dismissed

    readonly property var hints: onBadge ? [
        {
            glyph: "A",
            label: "Resume"
        },
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "≡",
            label: "Options"
        },
        {
            glyph: "LB RB",
            label: "Tabs"
        }
    ] : [
        {
            glyph: "A",
            label: onSearch ? "Search" : "Open"
        },
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "LB RB",
            label: "Tabs"
        }
    ]

    implicitHeight: Theme.dp(Theme.tabBarHeight)

    function enter() {
        index = Math.max(0, Math.min(slots - 1, currentIndex));
        forceActiveFocus();
    }

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
        if (index < searchIndex)
            index = currentIndex;
    }
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
    Keys.onDownPressed: function (event) {
        Sound.panel();
        root.entered();
    }

    Keys.onPressed: function (event) {
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
                // The lit tab may stand for a page past the bar (the Library): A lands on the tab itself.
                root.tabRequested(root.index);
                root.entered();
            }
        } else if (api.keys.isMenu(event) && root.onBadge) {
            event.accepted = true;
            if (root.currentGame)
                root.menuRequested(root.currentGame, badge);
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            Sound.cancel();
            root.dismissed();
        } else if (api.keys.isDetails(event) || api.keys.isFilters(event)) {
            event.accepted = true;
            Sound.edge();
        }
    }

    Item {
        id: frame

        // itemAt() is not a binding source, so the slot is picked by hand.
        property Item target: null
        readonly property Item pane: root.index < root.searchIndex ? labels : rightSide
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

        // Unseen moves (a tab switched from the page) land at once, or they'd play on the way in.
        Behavior on x {
            enabled: frame.visible
            Ease {
                duration: Theme.durNudge
                easing.type: Easing.OutQuint
            }
        }
        Behavior on y {
            enabled: frame.visible
            Ease {
                duration: Theme.durNudge
                easing.type: Easing.OutQuint
            }
        }
        Behavior on width {
            enabled: frame.visible
            Ease {
                duration: Theme.durNudge
                easing.type: Easing.OutQuint
            }
        }
        Behavior on height {
            enabled: frame.visible
            Ease {
                duration: Theme.durNudge
                easing.type: Easing.OutQuint
            }
        }
        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }

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
            onCountChanged: {
                underline.retarget();
                frame.retarget();
            }
            onItemAdded: {
                underline.retarget();
                frame.retarget();
            }

            Text {
                readonly property bool active: index === root.currentIndex

                text: modelData
                color: active ? Theme.text : Theme.textTab
                font.family: Theme.sans
                font.weight: active ? Font.DemiBold : Font.Medium
                font.pixelSize: Theme.dp(25)

                Behavior on color {
                    ColorEase {
                        duration: Theme.durBase
                    }
                }
            }
        }
    }

    Rectangle {
        id: underline

        property Item target: null
        readonly property real baseWidth: Theme.dp(100)

        function retarget() {
            target = (root.currentIndex >= 0 && root.currentIndex < tabRepeater.count) ? tabRepeater.itemAt(root.currentIndex) : null;
        }

        Component.onCompleted: retarget()

        width: baseWidth
        height: Math.max(2, Theme.dp(3))
        color: Theme.text
        antialiasing: true
        opacity: root.activeFocus && root.index === root.currentIndex ? 0.0 : 1.0

        Behavior on opacity {
            Ease {
                duration: Theme.durQuick
            }
        }

        anchors.top: labels.bottom
        anchors.topMargin: Theme.dp(8)

        x: target ? labels.x + target.x : labels.x
        transform: Scale {
            origin.x: 0
            xScale: underline.target ? underline.target.width / underline.baseWidth : 1

            Behavior on xScale {
                Ease {
                    duration: Theme.durNudge
                    easing.type: Easing.OutQuint
                }
            }
        }

        Behavior on x {
            Ease {
                duration: Theme.durNudge
                easing.type: Easing.OutQuint
            }
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
                running: journalMark.visible && !Theme.covered
                loops: Animation.Infinite
                NumberAnimation {
                    to: 0.35
                    duration: 900
                    easing.type: Easing.InOutQuad
                }
                NumberAnimation {
                    to: 1.0
                    duration: 900
                    easing.type: Easing.InOutQuad
                }
            }
        }

        MenuGlyph {
            id: glass

            onWidthChanged: frame.retarget()

            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(26)
            height: Theme.dp(26)
            visible: opacity > 0.01
            opacity: root.showSearch ? 1.0 : 0.0
            kind: "search"
            tint: root.activeFocus && root.onSearch ? Theme.text : Theme.textTab

            Behavior on opacity {
                Ease {
                    duration: Theme.durView
                }
            }
        }

        SessionBadge {
            id: badge
            anchors.verticalCenter: parent.verticalCenter
            focused: root.activeFocus && root.onBadge
            onActiveChanged: frame.retarget()
            onWidthChanged: frame.retarget()
        }

        PowerBadge {
            anchors.verticalCenter: parent.verticalCenter
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
