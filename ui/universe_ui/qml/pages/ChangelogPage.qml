import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "changelogPage"
    focus: true

    property var args: ({})
    readonly property var store: api.screens.changelog
    readonly property bool fresh: args.fresh === true
    readonly property var releases: fresh ? store.pending : store.releases

    signal closeRequested

    readonly property var hints: [
        {
            glyph: "dpad",
            label: "Scroll"
        },
        {
            glyph: "B",
            label: "Back"
        }
    ]

    readonly property real sideMargin: Theme.dp(90)
    readonly property real columnWidth: Math.min(Theme.dp(1200), width - sideMargin * 2)

    Component.onDestruction: if (fresh)
        store.dismiss()

    function maxScroll() {
        return Math.max(0, flick.contentHeight - flick.height);
    }

    function scrollTo(y) {
        var next = Math.max(0, Math.min(maxScroll(), y));
        if (next === flick.contentY) {
            Sound.edge();
            return;
        }
        Sound.tick();
        flick.contentY = next;
    }

    Keys.onPressed: function (event) {
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (event.isAutoRepeat && !vertical && !screen)
            return;
        event.accepted = true;
        if (api.keys.isCancel(event))
            page.closeRequested();
        else if (vertical)
            scrollTo(flick.contentY + (event.key === Qt.Key_Up ? -1 : 1) * Theme.dp(160));
        else if (screen)
            scrollTo(flick.contentY + screen * flick.height * 0.85);
        else if (api.keys.isFirst(event) || api.keys.isLast(event))
            scrollTo(api.keys.isFirst(event) ? 0 : maxScroll());
        else
            event.accepted = false;
    }

    Column {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(56)
        x: page.sideMargin
        spacing: Theme.dp(10)

        CapsLabel {
            text: "UNIVERSE " + (api.universe.version() || "development build").toUpperCase()
            tracking: 0.11
        }

        Text {
            text: page.fresh ? "What's new" : "Changelog"
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
        anchors.bottom: hintBar.top
        anchors.left: parent.left
        anchors.right: parent.right
        contentWidth: width
        contentHeight: notes.height + Theme.dp(60)
        interactive: false
        clip: true

        Behavior on contentY {
            id: notesEase
            Ease {
                duration: Theme.durView
            }
        }

        Wheel {
            ease: notesEase
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
                                    height: entry.height

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
                                        text: modelData
                                        textFormat: Text.RichText
                                        color: Theme.textSecondary
                                        font.family: Theme.sans
                                        font.pixelSize: Theme.dp(24)
                                        lineHeight: 1.4
                                        wrapMode: Text.WordWrap
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
        anchors.rightMargin: Theme.dp(52)
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        flickable: flick
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        hints: page.hints
    }
}
