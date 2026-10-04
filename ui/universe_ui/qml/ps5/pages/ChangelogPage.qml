import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "changelogPage"

    property var shell: null
    property var args: ({})
    readonly property var hints: []
    signal closeRequested
    focus: true

    readonly property var store: api.screens.changelog
    readonly property bool fresh: args.fresh === true
    readonly property var releases: fresh ? store.pending : store.releases

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

    ArtBackdrop {
        anchors.fill: parent
        target: ({
                scene: "system"
            })
        dim: 0.62
    }

    PageTitle {
        anchors.left: parent.left
        anchors.right: parent.right
        title: page.fresh ? "What's New" : "Changelog"
        trailing: "Universe " + (api.universe.version() || "development build")
    }

    Flickable {
        id: flick

        readonly property real room: Theme.dp(12)

        x: Theme.dp(Theme.edge) - room
        y: Theme.dp(170)
        width: Math.min(Theme.dp(1260), parent.width - Theme.dp(Theme.edge) - Theme.dp(Theme.columnRight)) + room * 2
        height: parent.height - y
        contentWidth: width
        contentHeight: notes.height + Theme.dp(120)
        interactive: false
        clip: true

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        Column {
            id: notes

            objectName: "changelogNotes"
            x: flick.room
            y: Theme.dp(20)
            width: flick.width - flick.room * 2
            spacing: Theme.dp(60)

            Repeater {
                model: page.releases

                Column {
                    width: notes.width
                    spacing: Theme.dp(22)

                    Label {
                        text: modelData.version
                        font.weight: Font.Light
                        font.pixelSize: Theme.dp(56)
                    }

                    Label {
                        text: modelData.dateText
                        color: Theme.textSecondary
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }

                    Repeater {
                        model: modelData.sections

                        Column {
                            width: notes.width
                            spacing: Theme.dp(12)

                            Label {
                                text: modelData.title
                                color: Theme.textSecondary
                                font.weight: Font.DemiBold
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
                                        font.pixelSize: Theme.dp(Theme.fontBody)
                                    }

                                    Label {
                                        id: entry

                                        x: Theme.dp(30)
                                        width: parent.width - x
                                        text: modelData
                                        textFormat: Text.RichText
                                        wrapMode: Text.WordWrap
                                        lineHeight: 1.4
                                        font.pixelSize: Theme.dp(Theme.fontBody)
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
        anchors.left: flick.right
        anchors.leftMargin: Theme.dp(24)
        anchors.top: flick.top
        anchors.bottom: flick.bottom
        flickable: flick
    }
}
