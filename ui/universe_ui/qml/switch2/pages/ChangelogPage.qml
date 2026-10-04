import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "changelogPage"

    property var shell: null
    property var args: ({})

    readonly property var store: api.screens.changelog
    readonly property bool fresh: args.fresh === true
    readonly property var releases: fresh ? store.pending : store.releases

    readonly property var hints: [
        {
            glyph: "B",
            label: "Back"
        }
    ]

    signal closeRequested

    readonly property real columnX: Theme.dp(253)
    readonly property real columnWidth: Theme.dp(1667 - 253)

    focus: true

    Component.onDestruction: if (fresh)
        store.dismiss()

    function maxScroll() {
        return Math.max(0, flick.contentHeight - flick.height);
    }

    function scroll(d) {
        var next = Math.max(0, Math.min(maxScroll(), flick.contentY + d * Theme.dp(260)));
        if (next === flick.contentY) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        flick.contentY = next;
    }

    Keys.onPressed: function (event) {
        if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            event.accepted = true;
            scroll(event.key === Qt.Key_Up ? -1 : 1);
        } else if ((event.key === Qt.Key_Left || event.key === Qt.Key_Right) && !event.isAutoRepeat) {
            event.accepted = true;
            Sound.play("edge");
        }
    }

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "news"
        title: page.fresh ? "What's New" : "Changelog"
        trailing: "Universe " + (api.universe.version() || "development build")
    }

    Flickable {
        id: flick

        readonly property real room: Theme.dp(Theme.ringRoom)

        x: page.columnX - room
        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight)
        width: page.columnWidth + room * 2
        contentWidth: width
        contentHeight: notes.height + Theme.dp(120)
        interactive: false
        clip: true

        Behavior on contentY {
            id: scrollEase
            Ease {}
        }

        Column {
            id: notes

            objectName: "changelogNotes"
            x: flick.room
            y: Theme.dp(48)
            width: page.columnWidth
            spacing: Theme.dp(56)

            Repeater {
                model: page.releases

                Column {
                    width: notes.width
                    spacing: Theme.dp(20)

                    Label {
                        text: "Version " + modelData.version
                        font.pixelSize: Theme.dp(Theme.fontTitle)
                    }

                    Label {
                        text: modelData.dateText
                        color: Theme.textSecondary
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }

                    Rectangle {
                        width: parent.width
                        height: 1
                        color: Theme.hairline
                    }

                    Repeater {
                        model: modelData.sections

                        Column {
                            width: notes.width
                            spacing: Theme.dp(12)

                            Label {
                                text: modelData.title
                                color: Theme.accent
                                font.pixelSize: Theme.dp(Theme.fontSmall)
                            }

                            Repeater {
                                model: modelData.items

                                Item {
                                    width: notes.width
                                    height: entry.height

                                    Label {
                                        text: "•"
                                        color: Theme.textSecondary
                                    }

                                    Label {
                                        id: entry

                                        x: Theme.dp(32)
                                        width: parent.width - x
                                        text: modelData
                                        textFormat: Text.RichText
                                        wrapMode: Text.WordWrap
                                        lineHeight: 1.4
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Swipe {
        flickable: flick
        ease: scrollEase
    }

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(60)
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        flickable: flick
    }
}
