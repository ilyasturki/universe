import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../ui" as Base

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null

    readonly property var screenshots: game && game.assets.screenshotList ? game.assets.screenshotList : []
    readonly property url fallbackArt: !game ? "" : String(game.assets.banner) !== "" ? game.assets.banner : game.assets.boxFront
    readonly property string description: game ? (game.description || game.summary || "") : ""
    property string zone: "button"
    property int shotIndex: 0

    readonly property var hints: zone === "shots" && screenshots.length > 1
        ? [ { glyph: "dpad", label: "Screenshots" }, { glyph: "B", label: "Back" } ]
        : zone === "text"
        ? [ { glyph: "B", label: "Back" } ]
        : [ { glyph: "B", label: "Back" }, { glyph: "A", label: "Confirm" } ]

    readonly property var facts: {
        if (!game)
            return [];
        var out = [];
        if (game.developerList.length > 0)
            out.push({ label: "Developer", value: game.developerList.join(", ") });
        if (game.genreList.length > 0)
            out.push({ label: "Genre", value: game.genreList.join(", ") });
        if (game.releaseYear > 0)
            out.push({ label: "Release", value: String(game.releaseYear) });
        if (game.players > 1)
            out.push({ label: "Players", value: String(game.players) });
        if (game.extra["metacritic"] !== undefined)
            out.push({ label: "Metacritic", value: String(game.extra["metacritic"][0]) });
        var hours = function(v) { return (Number(v) >= 10 ? Math.round(Number(v)) : Number(v).toFixed(1)) + " h"; };
        var hltb = [["hltb-main", "Main"], ["hltb-extra", "Extra"], ["hltb-completionist", "100%"]]
            .filter(function(p) { return game.extra[p[0]] !== undefined; })
            .map(function(p) { return p[1] + " " + hours(game.extra[p[0]][0]); });
        if (hltb.length > 0)
            out.push({ label: "How long to beat", value: hltb.join("  ·  ") });
        return out;
    }

    signal closeRequested()

    focus: true

    function stepShot(d) { shotIndex = Sound.stepped(shotIndex, d, screenshots.length); }

    function go(z) {
        Sound.play("tick");
        zone = z;
    }

    function scroll(d) {
        var max = Math.max(0, flick.contentHeight - flick.height);
        var next = Math.max(0, Math.min(max, flick.contentY + d * Theme.dp(220)));
        next === flick.contentY ? Sound.play("edge") : Sound.play("tick");
        flick.contentY = next;
    }

    Keys.onPressed: function(event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        if (event.isAutoRepeat && !(zone === "shots" && arrow) && !(zone === "text" && (event.key === Qt.Key_Up || event.key === Qt.Key_Down)))
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (zone === "button")
                shell.launch(page.game);
            else if (zone === "shots")
                stepShot(1);
            else
                Sound.play("edge");
        } else if (event.key === Qt.Key_Up) {
            event.accepted = true;
            if (zone === "button") go("shots"); else if (zone === "text") scroll(-1); else Sound.play("edge");
        } else if (event.key === Qt.Key_Down) {
            event.accepted = true;
            if (zone === "shots") go("button"); else if (zone === "text") scroll(1); else Sound.play("edge");
        } else if (event.key === Qt.Key_Right) {
            event.accepted = true;
            if (zone === "shots" && shotIndex < screenshots.length - 1) stepShot(1); else if (zone !== "text") go("text"); else Sound.play("edge");
        } else if (event.key === Qt.Key_Left) {
            event.accepted = true;
            if (zone === "shots") stepShot(-1); else if (zone === "text") go("button"); else Sound.play("edge");
        }
    }

    Label {
        x: Theme.dp(108)
        y: Theme.dp(78)
        text: page.game && page.game.publisherList.length > 0 ? page.game.publisherList.join(", ")
            : page.game && page.game.developerList.length > 0 ? page.game.developerList.join(", ") : ""
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    Label {
        x: Theme.dp(108)
        y: Theme.dp(116)
        width: Theme.dp(1500)
        text: page.game ? page.game.title : ""
        elide: Text.ElideRight
        font.pixelSize: Theme.dp(Theme.fontTitle)
    }

    Tile {
        x: parent.width - Theme.dp(108) - width
        y: Theme.dp(66)
        width: Theme.dp(96)
        height: Theme.dp(96)
        cornerRadius: Theme.dp(6)
        game: page.game
        outlineShown: false
    }

    Rectangle {
        x: Theme.dp(Theme.edgeMargin)
        y: Theme.dp(190)
        width: parent.width - x * 2
        height: 1
        color: Theme.hairline
    }

    Item {
        id: pane

        x: Theme.dp(136)
        y: Theme.dp(250)
        width: Theme.dp(720)
        height: Math.round(width * 9 / 16)

        Base.RoundedMask {
            id: paneBody
            anchors.fill: parent
            radius: Theme.dp(8)

            Rectangle {
                anchors.fill: parent
                radius: Theme.software ? Theme.dp(8) : 0
                color: "#1e1e1e"
            }

            Image {
                anchors.fill: parent
                source: page.screenshots.length > 0 ? page.screenshots[page.shotIndex] : ""
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                mipmap: true
                sourceSize.width: 1280
                visible: page.screenshots.length > 0
            }

            Image {
                anchors.fill: parent
                source: page.screenshots.length === 0 ? page.fallbackArt : ""
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                mipmap: true
                sourceSize.height: 720
                visible: page.screenshots.length === 0 && status === Image.Ready
            }

            Label {
                anchors.centerIn: parent
                visible: page.screenshots.length === 0 && String(page.fallbackArt) === ""
                text: "No screenshots"
                color: "#8a8a8a"
            }
        }

        FocusOutline {
            target: paneBody
            cornerRadius: Theme.dp(8)
            shown: page.zone === "shots" && page.activeFocus
        }
    }

    Row {
        anchors.top: pane.bottom
        anchors.topMargin: Theme.dp(18)
        anchors.horizontalCenter: pane.horizontalCenter
        spacing: Theme.dp(12)
        visible: page.screenshots.length > 1

        Repeater {
            model: page.screenshots.length

            Rectangle {
                width: Theme.dp(12)
                height: width
                radius: width / 2
                color: index === page.shotIndex ? Theme.text : Theme.hairline
            }
        }
    }

    Item {
        id: startButton

        x: pane.x
        y: pane.y + pane.height + Theme.dp(60)
        width: pane.width
        height: Theme.dp(96)

        FocusPill {
            anchors.fill: parent
            color: page.zone === "button" ? Theme.focusFill : "transparent"
            border.width: Theme.dp(2)
            border.color: page.zone === "button" ? "transparent" : Theme.hairline
            visible: true
            focused: page.zone === "button" && page.activeFocus
        }

        Label {
            anchors.centerIn: parent
            text: page.game && page.game.playTime > 0 ? "Continue Software" : "Start Software"
        }
    }

    Item {
        id: textPane

        x: Theme.dp(990)
        y: Theme.dp(250)
        width: Theme.dp(790)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(30)

        Flickable {
            id: flick

            anchors.fill: parent
            contentWidth: width
            contentHeight: article.height + Theme.dp(20)
            interactive: false
            clip: true

            Behavior on contentY { Ease {} }

            Column {
                id: article

                width: flick.width - Theme.dp(20)
                spacing: Theme.dp(30)

                Label {
                    width: parent.width
                    visible: page.description !== ""
                    text: page.description
                    color: page.zone === "text" ? Theme.text : Theme.artInk
                    wrapMode: Text.WordWrap
                    lineHeight: 1.4
                }

                Label {
                    width: parent.width
                    visible: page.description === "" && page.facts.length === 0
                    text: "No information about this software yet."
                    color: Theme.textSecondary
                    wrapMode: Text.WordWrap
                }

                Column {
                    width: parent.width
                    spacing: Theme.dp(8)

                    Repeater {
                        model: page.facts

                        Row {
                            width: parent.width
                            spacing: Theme.dp(20)

                            Label {
                                width: Theme.dp(260)
                                text: modelData.label
                                color: Theme.textSecondary
                                elide: Text.ElideRight
                                font.pixelSize: Theme.dp(Theme.fontSmall)
                            }

                            Label {
                                width: parent.width - Theme.dp(280)
                                text: modelData.value
                                wrapMode: Text.WordWrap
                                font.pixelSize: Theme.dp(Theme.fontSmall)
                            }
                        }
                    }
                }
            }
        }

        Scrollbar {
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            flickable: flick
        }
    }
}
