import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: root

    property var tabs: []
    property int currentIndex: 0
    property string title: ""
    // Runs past the tabs: the search glass, the power button, then the running game's badge.
    property int index: 0

    readonly property int searchIndex: tabs.length
    readonly property bool onSearch: index === searchIndex
    readonly property int powerIndex: searchIndex + 1
    readonly property bool onPower: index === powerIndex
    readonly property int badgeIndex: powerIndex + 1
    readonly property bool onBadge: badge.active && index === badgeIndex
    readonly property int slots: badgeIndex + (badge.active ? 1 : 0)
    readonly property var currentGame: onBadge ? api.allGames.byId(badge.session.id) : null
    readonly property Item menuAnchor: onBadge ? badge : null

    signal tabRequested(int index)
    signal searchRequested
    signal powerRequested
    signal resumeRequested
    signal menuRequested(var game, Item anchor)
    signal entered
    signal dismissed
    // The mouse is on a slot: the frame lands there, no page switch until a click.
    signal pointed

    function pointTo(slot) {
        index = slot;
        pointed();
        forceActiveFocus();
    }

    readonly property var hints: onBadge ? [
        {
            glyph: "A",
            label: "Resume"
        },
        {
            glyph: "Start",
            label: "More"
        },
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "LB RB",
            label: "Tabs"
        }
    ] : [
        {
            glyph: "A",
            label: onSearch ? "Search" : onPower ? "Power" : "Open"
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
            else if (root.onPower)
                root.powerRequested();
            else {
                Sound.panel();
                // A tab the mouse pointed to is not open yet.
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

    // A glyph that says what it is while the frame is on it.
    component GlyphSlot: Item {
        id: slot

        property string kind: ""
        property string label: ""
        property bool lit: false

        signal picked

        anchors.verticalCenter: parent.verticalCenter
        width: content.width
        height: content.height
        onWidthChanged: frame.retarget()

        Row {
            id: content

            spacing: slot.lit ? Theme.dp(12) : 0

            MenuGlyph {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(26)
                height: Theme.dp(26)
                kind: slot.kind
                tint: slot.lit ? Theme.text : Theme.textTab
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: slot.lit
                text: slot.label
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(22)
            }
        }

        Pointer {
            anchors.margins: -Theme.dp(12)
            direct: true
            radius: height / 2
            onPicked: slot.picked()
        }
    }

    Item {
        id: frame

        // itemAt() is not a binding source, so the slot is picked by hand.
        property Item target: null
        readonly property Item pane: root.index < root.searchIndex ? labels : rightSide
        readonly property real padX: root.onBadge ? 0 : root.index >= root.searchIndex ? padY : Theme.dp(20)
        readonly property real padY: root.onBadge ? 0 : Theme.dp(10)

        function retarget() {
            target = root.index < root.searchIndex ? tabRepeater.itemAt(root.index) : root.onBadge ? badge : root.onPower ? power : search;
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

            Item {
                id: tab

                readonly property bool active: index === root.currentIndex
                property color tint: active ? Theme.text : Theme.textTab

                width: tabContent.width
                height: tabContent.height

                Behavior on tint {
                    ColorEase {
                        duration: Theme.durBase
                    }
                }

                Row {
                    id: tabContent

                    spacing: Theme.dp(10)

                    MenuGlyph {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(24)
                        height: Theme.dp(24)
                        kind: modelData.icon
                        tint: tab.tint
                    }

                    Text {
                        text: modelData.name
                        color: tab.tint
                        font.family: Theme.sans
                        font.weight: tab.active ? Font.DemiBold : Font.Medium
                        font.pixelSize: Theme.dp(25)
                    }
                }

                Pointer {
                    anchors.margins: -Theme.dp(12)
                    direct: true
                    radius: height / 2
                    onPicked: root.pointTo(index)
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

    Text {
        id: titleText
        objectName: "barTitle"

        readonly property real roomLeft: labels.x + labels.width + Theme.dp(48)
        readonly property real roomRight: rightSide.x - Theme.dp(48)

        anchors.verticalCenter: labels.verticalCenter
        x: Math.max(roomLeft, Math.min((root.width - width) / 2, roomRight - width))
        width: Math.min(implicitWidth, Math.max(0, roomRight - roomLeft))
        text: root.title
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(30)
        elide: Text.ElideRight
    }

    Row {
        id: rightSide
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(Theme.edgeMargin)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(34)

        JournalMark {
            anchors.verticalCenter: parent.verticalCenter
        }

        GlyphSlot {
            id: search
            kind: "search"
            label: "Search"
            lit: root.activeFocus && root.onSearch
            onPicked: root.pointTo(root.searchIndex)
        }

        GlyphSlot {
            id: power
            objectName: "powerButton"
            kind: "power"
            label: "Power"
            lit: root.activeFocus && root.onPower
            onPicked: root.pointTo(root.powerIndex)
        }

        SessionBadge {
            id: badge
            anchors.verticalCenter: parent.verticalCenter
            focused: root.activeFocus && root.onBadge
            onActiveChanged: frame.retarget()
            onWidthChanged: frame.retarget()

            Pointer {
                enabled: badge.active
                direct: true
                radius: height / 2
                onPicked: root.pointTo(root.badgeIndex)
            }
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
