import QtQuick
import "../core"
import "../sound"
import "../ui"

// A game's journal: entries on the left, the picked one read on the right.
FocusScope {
    id: page

    focus: true

    property var game: null
    readonly property var store: api.screens.journal
    readonly property var rows: store.rows
    property int index: 0
    // Left/Right hands the d-pad to the reading pane, whose Up/Down then scroll.
    property bool reading: false
    readonly property var current: index >= 0 && index < rows.length ? rows[index] : null

    signal closeRequested()

    readonly property var hints: reading
        ? [ { glyph: "dpad", label: "Scroll" }, { glyph: "B", label: "Back to entries" } ]
        : [ { glyph: "dpad", label: "Navigate" }, { glyph: "A", label: "Read" }, { glyph: "B", label: "Back" } ]

    readonly property real sideMargin: Theme.dp(90)
    readonly property real listWidth: Theme.dp(560)

    onGameChanged: {
        index = 0;
        reading = false;
        if (game)
            store.load(game.id);
    }

    onCurrentChanged: flick.contentY = 0

    function step(d) {
        var next = Math.max(0, Math.min(rows.length - 1, index + d));
        next === index ? Sound.edge() : Sound.tick();
        index = next;
    }

    function scroll(d) {
        var maxY = Math.max(0, flick.contentHeight - flick.height);
        var next = Math.max(0, Math.min(maxY, flick.contentY + d * Theme.dp(260)));
        next === flick.contentY ? Sound.edge() : Sound.tick();
        flick.contentY = next;
    }

    Keys.onUpPressed: reading ? scroll(-1) : step(-1)
    Keys.onDownPressed: reading ? scroll(1) : step(1)
    Keys.onRightPressed: function(event) {
        if (!reading && current) {
            Sound.panel();
            reading = true;
        } else {
            Sound.edge();
        }
    }
    Keys.onLeftPressed: function(event) {
        if (reading) {
            Sound.panel();
            reading = false;
        } else {
            Sound.edge();
        }
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (!reading && current) {
                Sound.panel();
                reading = true;
            }
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            if (reading) {
                Sound.cancel();
                reading = false;
            } else {
                page.closeRequested();
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    Item {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(44)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: title.height + Theme.dp(10) + subtitle.height

        Text {
            id: title
            text: "Journal"
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(46)
        }

        Text {
            id: subtitle
            anchors.top: title.bottom
            anchors.topMargin: Theme.dp(10)
            text: (page.game ? page.game.title : "") + (page.rows.length > 0 ? " · " + page.rows.length + (page.rows.length === 1 ? " entry" : " entries") : "")
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(24)
        }
    }

    Text {
        anchors.centerIn: parent
        visible: page.rows.length === 0
        text: "No journal entries yet — one is written after each session."
        color: Theme.textMuted
        font.family: Theme.sans
        font.pixelSize: Theme.dp(26)
    }

    ListView {
        id: list

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(30)
        anchors.bottom: hintBar.top
        anchors.left: parent.left
        anchors.leftMargin: page.sideMargin
        width: page.listWidth
        model: page.rows
        currentIndex: page.index
        interactive: false
        clip: true
        spacing: Theme.dp(12)
        opacity: page.reading ? 0.55 : 1.0
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: 0
        preferredHighlightEnd: height
        highlightRangeMode: ListView.ApplyRange

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        delegate: Rectangle {
            readonly property bool focused: index === page.index

            width: list.width
            height: Theme.dp(96)
            radius: Theme.dp(16)
            color: focused && !page.reading ? Theme.text : Theme.surface

            Behavior on color {
                ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
            }

            Column {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.margins: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                spacing: Theme.dp(6)

                Text {
                    width: parent.width
                    text: modelData.title
                    color: focused && !page.reading ? Theme.onLight : Theme.text
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Theme.dp(24)
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: modelData.dateText
                    color: focused && !page.reading ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(20)
                    elide: Text.ElideRight
                }
            }
        }
    }

    Flickable {
        id: flick

        anchors.top: list.top
        anchors.bottom: hintBar.top
        anchors.left: list.right
        anchors.leftMargin: Theme.dp(60)
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        contentWidth: width
        contentHeight: article.height + Theme.dp(60)
        interactive: false
        clip: true
        visible: page.current !== null

        Behavior on contentY {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
        }

        Column {
            id: article

            width: flick.width
            spacing: Theme.dp(28)

            Text {
                width: parent.width
                text: page.current ? page.current.title : ""
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(38)
                wrapMode: Text.WordWrap
            }

            CapsLabel {
                text: page.current ? (page.current.dateText + (page.current.provider ? "  ·  " + page.current.provider.toUpperCase() : "")) : ""
                tracking: 0.11
            }

            Repeater {
                model: page.current ? page.current.paragraphs : []

                Text {
                    width: article.width
                    text: modelData
                    color: page.reading ? Theme.text : Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(24)
                    lineHeight: 1.5
                    wrapMode: Text.WordWrap

                    Behavior on color {
                        ColorAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
                    }
                }
            }

            Column {
                width: parent.width
                spacing: Theme.dp(10)
                visible: page.current && page.current.next_up !== ""

                CapsLabel {
                    text: "NEXT UP"
                    tracking: 0.11
                }

                Text {
                    width: parent.width
                    text: page.current ? page.current.next_up : ""
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Medium
                    font.pixelSize: Theme.dp(24)
                    lineHeight: 1.4
                    wrapMode: Text.WordWrap
                }
            }

            Flow {
                width: parent.width
                spacing: Theme.dp(16)

                Repeater {
                    model: page.current ? page.current.images : []

                    Rectangle {
                        width: Theme.dp(336)
                        height: Theme.dp(189)
                        radius: Theme.dp(10)
                        color: Theme.cardBase
                        clip: true

                        Image {
                            anchors.fill: parent
                            source: modelData
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                        }
                    }
                }
            }
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        showClock: true
        hints: page.hints
    }
}
