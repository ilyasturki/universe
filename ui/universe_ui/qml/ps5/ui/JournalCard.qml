import QtQuick
import "../core"
import "../../ui" as Base
import "../pages/Home.js" as Home

// One session of the Journal: its picture, the game, the entry's title and first lines,
// or what stands in for them while it is written, put off, failed or never asked for.
Item {
    id: card

    // A row of api.screens.news.
    property var row: null
    property bool focused: false
    // Now, for a pending entry's time so far.
    property double now: Date.now()

    signal picked

    readonly property var game: row ? api.allGames.byId(row.gameId) : null
    readonly property string phase: row ? row.state : ""
    readonly property bool pending: phase === "pending"
    readonly property bool blank: phase === "none"
    readonly property string picture: row && row.images && row.images.length > 0 ? row.images[0] : ""
    readonly property string fallback: game ? String(Home.art(game).source) : ""

    function elapsed(startedAt) {
        var s = Math.round((now - Date.parse(startedAt)) / 1000);
        if (isNaN(s))
            return "";
        s = Math.max(0, s);
        if (s < 60)
            return s + " s";
        if (s < 3600)
            return Math.floor(s / 60) + " min";
        return Math.floor(s / 3600) + " h " + ("0" + Math.floor((s % 3600) / 60)).slice(-2);
    }

    readonly property string heading: !row ? "" : pending ? "Writing the entry…" : blank ? "No entry yet" : row.title
    readonly property string line: {
        if (!row)
            return "";
        if (pending)
            return "The journal module is writing it: " + elapsed(row.started_at) + " so far.";
        if (blank)
            return "A session nobody wrote about. " + Theme.buttonName("A") + " writes its entry.";
        if (phase === "deferred")
            return row.reason + (row.retryText !== "" ? " · another try " + row.retryText : "");
        if (phase === "failed")
            return row.reason;
        return (row.paragraphs || []).join(" ");
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(Theme.radiusCard)
        color: card.focused ? Qt.rgba(0.14, 0.15, 0.19, 0.92) : Theme.glass
        border.width: 1
        border.color: Theme.glassEdge

        Behavior on color {
            ColorAnimation {
                duration: Theme.durFocus
            }
        }
    }

    Base.RoundedMask {
        id: art
        x: Theme.dp(16)
        y: Theme.dp(16)
        height: parent.height - Theme.dp(32)
        width: Math.round(height * 16 / 9)
        radius: Theme.dp(6)

        Rectangle {
            anchors.fill: parent
            radius: Theme.software ? Theme.dp(6) : 0
            color: Theme.artShade
        }

        Image {
            anchors.fill: parent
            source: card.picture !== "" ? card.picture : card.fallback
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            sourceSize.width: 480
            opacity: status === Image.Ready ? (card.picture !== "" ? 1.0 : 0.5) : 0.0
        }
    }

    Column {
        anchors.left: art.right
        anchors.leftMargin: Theme.dp(28)
        anchors.right: marks.left
        anchors.rightMargin: Theme.dp(24)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(6)

        Row {
            spacing: Theme.dp(12)

            TileArt {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(30)
                height: width
                radius: Theme.dp(5)
                game: card.game
                titleSize: Theme.dp(7)
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: card.row ? card.row.gameTitle + "  ·  " + card.row.dateText + (card.row.durationText ? "  ·  " + card.row.durationText : "") : ""
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontTiny)
            }
        }

        Label {
            width: parent.width
            text: card.heading
            color: card.pending || card.blank ? Theme.textSecondary : Theme.text
            elide: Text.ElideRight
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(29)
        }

        Label {
            width: parent.width
            visible: text !== ""
            text: card.line
            color: card.phase === "failed" ? Theme.danger : Theme.textMuted
            elide: Text.ElideRight
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }

    Row {
        id: marks

        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(28)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(16)

        Rectangle {
            id: dot
            anchors.verticalCenter: parent.verticalCenter
            visible: card.pending
            width: Theme.dp(12)
            height: width
            radius: width / 2
            color: Theme.text

            SequentialAnimation on opacity {
                running: card.pending && card.visible && !Theme.covered
                loops: Animation.Infinite
                NumberAnimation {
                    to: 0.25
                    duration: 700
                    easing.type: Easing.InOutQuad
                }
                NumberAnimation {
                    to: 1.0
                    duration: 700
                    easing.type: Easing.InOutQuad
                }
            }
        }

        Glyph {
            anchors.verticalCenter: parent.verticalCenter
            visible: card.row !== null && card.row.hasRecording
            width: Theme.dp(30)
            height: width
            kind: "film"
            tint: Theme.textSecondary
        }
    }

    FocusFrame {
        target: card
        shown: card.focused
        radius: Theme.dp(Theme.radiusCard)
        gap: Theme.dp(3)
        line: Theme.dp(3)
    }

    Touch {
        current: card.focused
        menu: true
        onPicked: card.picked()
    }
}
