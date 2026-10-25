import QtQuick
import "../core"
import "../sound"

// What's new over the screen it opens on: `fresh` lists the releases since the last version run and marks them seen as it closes.
FocusScope {
    id: dialog

    property bool open: false
    property bool fresh: false
    readonly property var store: api.screens.changelog

    signal closed

    readonly property var hints: [
        {
            glyph: "dpad",
            label: "Scroll"
        },
        {
            glyph: "B",
            label: "Close"
        }
    ]

    focus: open
    visible: scrim.opacity > 0.01

    function show(isFresh) {
        Sound.panel();
        fresh = isFresh === true;
        open = true;
        forceActiveFocus();
    }

    function hide() {
        Sound.cancel();
        if (fresh)
            store.dismiss();
        open = false;
        focus = false;
        closed();
    }

    Keys.onPressed: function (event) {
        event.accepted = true;
    }
    Keys.onReleased: function (event) {
        event.accepted = true;
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Qt.rgba(0.02, 0.02, 0.03, 1)
        opacity: dialog.open ? 0.62 : 0.0

        Behavior on opacity {
            Ease {}
        }

        // Keeps the mouse off the screen beneath, its right click too: a click outside the card is B.
        HoverHandler {}
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
            onClicked: api.keys.press("Cancel")
        }
    }

    Rectangle {
        id: card

        width: Math.min(Theme.dp(1200), dialog.width - Theme.dp(160))
        height: Math.min(Theme.dp(800), dialog.height - Theme.dp(Theme.tabBarHeight + Theme.hintBarHeight))
        x: (dialog.width - width) / 2
        y: (dialog.height - height) / 2 + (dialog.open ? 0 : Theme.dp(24))
        radius: Theme.dp(24)
        color: "#1b1d24"
        border.width: 1
        border.color: Theme.surfaceBorder
        clip: true
        opacity: scrim.opacity / 0.62

        Behavior on y {
            Ease {}
        }

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
        }

        Loader {
            anchors.fill: parent
            active: dialog.open || scrim.opacity > 0.01
            focus: true
            sourceComponent: FocusScope {
                id: page

                objectName: "changelogPage"
                focus: true

                readonly property var store: dialog.store
                readonly property bool fresh: dialog.fresh
                readonly property var releases: fresh ? store.pending : store.releases

                readonly property real sideMargin: Theme.dp(56)
                readonly property real columnWidth: width - sideMargin * 2

                function scrollTo(y) {
                    notesScroll.to(y) ? Sound.tick() : Sound.edge();
                }

                Keys.onPressed: function (event) {
                    var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
                    var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
                    event.accepted = true;
                    if (event.isAutoRepeat && !vertical && !screen)
                        return;
                    if (api.keys.isCancel(event))
                        dialog.hide();
                    else if (vertical)
                        scrollTo(notesScroll.heading() + (event.key === Qt.Key_Up ? -1 : 1) * Theme.dp(160));
                    else if (screen)
                        scrollTo(notesScroll.heading() + screen * flick.height * 0.85);
                    else if (api.keys.isFirst(event) || api.keys.isLast(event))
                        scrollTo(api.keys.isFirst(event) ? notesScroll.low() : notesScroll.high());
                }

                Column {
                    id: header

                    anchors.top: parent.top
                    anchors.topMargin: Theme.dp(44)
                    x: page.sideMargin
                    spacing: Theme.dp(10)

                    CapsLabel {
                        text: "UNIVERSE " + (api.universe.version() || "development build").toUpperCase()
                        tracking: 0.11
                    }

                    Text {
                        text: "What's new"
                        color: Theme.text
                        font.family: Theme.sans
                        font.weight: Font.Bold
                        font.pixelSize: Theme.dp(44)
                    }
                }

                Flickable {
                    id: flick

                    anchors.top: header.bottom
                    anchors.topMargin: Theme.dp(24)
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: Theme.dp(36)
                    anchors.left: parent.left
                    anchors.right: parent.right
                    contentWidth: width
                    contentHeight: notes.height + Theme.dp(60)
                    interactive: false
                    clip: true

                    Scroller {
                        id: notesScroll
                    }

                    Column {
                        id: notes

                        objectName: "changelogNotes"
                        x: page.sideMargin
                        width: page.columnWidth
                        spacing: Theme.dp(44)

                        Repeater {
                            model: page.releases

                            Column {
                                width: notes.width
                                spacing: Theme.dp(18)

                                Row {
                                    spacing: Theme.dp(18)

                                    Text {
                                        id: version

                                        text: modelData.version
                                        color: Theme.text
                                        font.family: Theme.sans
                                        font.weight: Font.Bold
                                        font.pixelSize: Theme.dp(34)
                                    }

                                    CapsLabel {
                                        anchors.baseline: version.baseline
                                        text: modelData.dateText.toUpperCase()
                                        tracking: 0.11
                                    }
                                }

                                Repeater {
                                    model: modelData.sections

                                    Column {
                                        width: notes.width
                                        spacing: Theme.dp(10)

                                        CapsLabel {
                                            text: modelData.title.toUpperCase()
                                            color: Theme.accent
                                            tracking: 0.11
                                        }

                                        Repeater {
                                            model: modelData.items

                                            Item {
                                                width: notes.width
                                                height: entry.height + (shot.height > 0 ? Theme.dp(16) + shot.height : 0)

                                                Text {
                                                    text: "•"
                                                    color: Theme.textMuted
                                                    font.family: Theme.sans
                                                    font.pixelSize: Theme.dp(24)
                                                }

                                                Text {
                                                    id: entry

                                                    x: Theme.dp(28)
                                                    width: parent.width - x
                                                    text: modelData.text
                                                    textFormat: Text.RichText
                                                    color: Theme.textSecondary
                                                    font.family: Theme.sans
                                                    font.pixelSize: Theme.dp(24)
                                                    lineHeight: 1.4
                                                    wrapMode: Text.WordWrap
                                                }

                                                Loader {
                                                    id: shot

                                                    x: entry.x
                                                    y: entry.height + Theme.dp(16)
                                                    width: Math.min(entry.width, Theme.dp(960))
                                                    active: !!modelData.media
                                                    sourceComponent: ReleaseMedia {
                                                        media: modelData.media
                                                        store: page.store
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Scrollbar {
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.dp(20)
                    anchors.top: flick.top
                    anchors.bottom: flick.bottom
                    flickable: flick
                }
            }
        }
    }
}
