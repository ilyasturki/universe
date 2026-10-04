import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../core/Format.js" as Format
import "Home.js" as Home

// The console's Information: the game's name and Play over its world, what it is at the left, the facts at the right,
// the store's screenshots under them.
FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    readonly property var game: args && args.gameId ? api.allGames.byId(args.gameId) : null
    readonly property var hints: []
    readonly property bool strip: false

    signal closeRequested

    focus: true

    readonly property var screenshots: game && game.assets.screenshotList ? game.assets.screenshotList : []
    readonly property string description: game ? (game.description || game.summary || "") : ""
    readonly property bool playing: game !== null && game.id === (shell ? shell.playingId : "")
    // "play", "text" (the page scrolls under the cursor), "shots".
    property string zone: "play"
    property int shotIndex: 0
    readonly property alias viewerOpen: viewer.open
    // The core's own record: where the game lives on disk.
    property var record: ({})

    readonly property var sourceNames: ({
            gog: "GOG",
            steam: "Steam",
            epic: "Epic Games",
            itch: "itch.io",
            lutris: "Lutris",
            manual: "Added by hand"
        })

    readonly property var facts: {
        var g = game;
        if (!g)
            return [];
        var out = [];
        var add = function (label, value) {
            if (value !== undefined && value !== null && String(value) !== "")
                out.push({
                    label: label,
                    value: String(value)
                });
        };
        add("Developer", g.developerList.join(", "));
        add("Publisher", g.publisherList.join(", "));
        add("Release", g.releaseYear > 0 ? g.releaseYear : "");
        add("Genre", g.genreList.join(", "));
        add("Players", g.players > 1 ? "Up to " + g.players : g.players === 1 ? "Single player" : "");
        add("Platform", g.platform === "windows" ? "Windows" : g.platform === "linux" ? "Linux" : g.platform);
        add("Source", sourceNames[g.source] || g.source);
        add("Runner", g.runnerName || g.runner);
        add("Play time", Format.playTime(g.playTime));
        add("Sessions", g.playCount > 0 ? g.playCount : "");
        var last = g.lastPlayed ? new Date(g.lastPlayed) : null;
        add("Last played", last && !isNaN(last.getTime()) && last.getFullYear() > 1971 ? Format.lastPlayed(last) : "");
        if (g.extra["metacritic"] !== undefined)
            add("Metacritic", g.extra["metacritic"][0]);
        var hours = function (v) {
            return (Number(v) >= 10 ? Math.round(Number(v)) : Number(v).toFixed(1)) + " h";
        };
        var hltb = [["hltb-main", "Main"], ["hltb-extra", "Extra"], ["hltb-completionist", "100%"]].filter(function (p) {
            return g.extra[p[0]] !== undefined;
        }).map(function (p) {
            return p[1] + " " + hours(g.extra[p[0]][0]);
        });
        add("How long to beat", hltb.join("  ·  "));
        add("Tags", g.tags.join(", "));
        var launch = record && record.launch ? record.launch : {};
        add("Program", launch.exe || "");
        add("Prefix", launch.prefix || "");
        return out;
    }

    function play() {
        if (!game) {
            Sound.play("edge");
            return;
        }
        if (game.installing) {
            Sound.play("ok");
            shell.push("pages/InstallPage.qml", {
                tab: 1
            });
        } else if (playing) {
            shell.resume();
        } else {
            playButton.flash();
            shell.launch(game);
        }
    }

    function openShot(i) {
        if (screenshots.length === 0) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        shotIndex = Math.max(0, Math.min(screenshots.length - 1, i));
        viewer.show(shotIndex);
    }

    function go(z) {
        Sound.play("tick");
        zone = z;
        if (z === "play")
            flick.contentY = 0;
        else if (z === "shots")
            flick.contentY = Math.max(0, Math.min(flick.contentHeight - flick.height, shotsRow.y + shotsRow.height + Theme.dp(40) - flick.height));
    }

    function maxScroll() {
        return Math.max(0, (shots.visible ? shotsRow.y : flick.contentHeight) - flick.height);
    }

    // Down through the text a screenful at a time; past its end, the screenshots.
    function scroll(d) {
        var next = Math.max(0, Math.min(maxScroll(), flick.contentY + d * Theme.dp(260)));
        if (next === flick.contentY) {
            if (d > 0 && screenshots.length > 0)
                go("shots");
            else if (d < 0)
                go("play");
            else
                Sound.play("edge");
            return;
        }
        Sound.play("tick");
        flick.contentY = next;
    }

    function options() {
        if (!game) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        var items = [
            {
                label: playing ? "Resume" : game.playTime > 0 ? "Continue" : "Play",
                glyph: "play",
                act: "play"
            }
        ];
        if (screenshots.length > 0)
            items.push({
                label: "View Screenshots",
                glyph: "image",
                act: "shots"
            });
        if (game.achievementsTotal > 0)
            items.push({
                label: "Trophies",
                glyph: "trophy",
                act: "trophies"
            });
        items.push({
            label: "Game Settings",
            glyph: "sliders",
            act: "settings"
        }, {
            label: "Media Gallery",
            glyph: "gallery",
            act: "gallery"
        }, {
            label: "Play Log",
            glyph: "clock",
            act: "log"
        }, {
            label: "Saved Data and Storage",
            glyph: "storage",
            act: "data"
        }, {
            label: "Artwork",
            glyph: "image",
            act: "artwork"
        });
        var pages = {
            trophies: "pages/AchievementsPage.qml",
            settings: "pages/GameSettingsPage.qml",
            gallery: "pages/MediaGalleryPage.qml",
            log: "pages/PlayLogPage.qml",
            data: "pages/DataPage.qml",
            artwork: "pages/ArtworkPage.qml"
        };
        var id = game.id;
        shell.showMenu({
            items: items
        }, function (i) {
            if (i < 0)
                return;
            var act = items[i].act;
            if (act === "play")
                page.play();
            else if (act === "shots")
                page.openShot(page.shotIndex);
            else
                page.shell.push(pages[act], {
                    gameId: id
                });
        });
    }

    // The derived game is still stale here: read the args themselves.
    onArgsChanged: {
        var g = args && args.gameId ? api.allGames.byId(args.gameId) : null;
        record = g ? (api.universe.game(g.id) || {}) : {};
        var shot = args ? args.shot : undefined;
        if (shot !== undefined && shot !== null && g && g.assets.screenshotList.length > 0) {
            zone = "shots";
            Qt.callLater(function () {
                page.go("shots");
                page.openShot(Number(shot));
            });
        }
    }

    Keys.onPressed: function (event) {
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (event.isAutoRepeat && !arrow && !vertical && !screen)
            return;
        if (screen) {
            event.accepted = true;
            if (zone !== "text")
                zone = "text";
            scroll(screen);
            return;
        }
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (zone === "play")
                play();
            else if (zone === "shots")
                openShot(shotIndex);
            else
                Sound.play("edge");
        } else if (api.keys.isMenu(event)) {
            event.accepted = true;
            options();
        } else if (event.key === Qt.Key_Down) {
            event.accepted = true;
            if (zone === "play")
                maxScroll() > 0 ? go("text") : screenshots.length > 0 ? go("shots") : Sound.play("edge");
            else if (zone === "text")
                scroll(1);
            else
                Sound.play("edge");
        } else if (event.key === Qt.Key_Up) {
            event.accepted = true;
            if (zone === "shots") {
                if (maxScroll() > 0) {
                    Sound.play("tick");
                    zone = "text";
                    flick.contentY = maxScroll();
                } else {
                    go("play");
                }
            } else if (zone === "text") {
                scroll(-1);
            } else {
                Sound.play("edge");
            }
        } else if (arrow) {
            event.accepted = true;
            if (zone === "shots")
                shotIndex = Sound.stepped(shotIndex, event.key === Qt.Key_Left ? -1 : 1, screenshots.length);
            else
                Sound.play("edge");
        }
    }

    ArtBackdrop {
        anchors.fill: parent
        target: Home.art(page.game)
        dim: 0.7
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
        title: "Information"
    }

    Flickable {
        id: flick

        anchors.top: header.bottom
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        contentWidth: width
        contentHeight: body.height + Theme.dp(60)
        interactive: false
        clip: true

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        Item {
            id: body

            readonly property real leftX: Theme.dp(Theme.edge)
            readonly property real inner: flick.width - Theme.dp(Theme.edge + Theme.columnRight)
            readonly property real columnGap: Theme.dp(80)
            readonly property real leftWidth: Math.round((inner - columnGap) * 0.56)

            width: flick.width
            height: shotsRow.y + (shots.visible ? shotsRow.height : 0)

            Column {
                id: about

                x: body.leftX
                y: Theme.dp(20)
                width: body.leftWidth
                spacing: Theme.dp(18)

                Label {
                    width: parent.width
                    text: page.game ? page.game.title : ""
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    lineHeight: 0.95
                    font.weight: Font.Light
                    font.pixelSize: Theme.dp(54)
                }

                Label {
                    width: parent.width
                    visible: text !== ""
                    text: page.game ? Home.tagline(page.game) : ""
                    color: Theme.textSecondary
                    elide: Text.ElideRight
                    font.pixelSize: Theme.dp(Theme.fontBody)
                }

                Item {
                    width: parent.width
                    height: Theme.dp(12)
                }

                PillButton {
                    id: playButton
                    width: Theme.dp(Theme.playWidth)
                    text: page.game && page.game.installing ? "View Download" : page.playing ? "Resume" : "Play Game"
                    focused: page.activeFocus && page.zone === "play" && !viewer.open
                    onPicked: {
                        if (page.zone === "play") {
                            page.play();
                            return;
                        }
                        page.go("play");
                        page.forceActiveFocus();
                    }
                }

                Item {
                    width: parent.width
                    height: Theme.dp(22)
                }

                Label {
                    id: blurb
                    width: parent.width
                    text: page.description !== "" ? page.description : "No description of this game yet."
                    color: page.description !== "" ? (page.zone === "text" ? Theme.text : Qt.rgba(1, 1, 1, 0.84)) : Theme.textMuted
                    wrapMode: Text.WordWrap
                    lineHeight: 1.4

                    Behavior on color {
                        ColorAnimation {
                            duration: Theme.durFocus
                        }
                    }

                    Touch {
                        current: page.zone === "text"
                        onPicked: {
                            page.go("text");
                            page.forceActiveFocus();
                        }
                    }
                }
            }

            Column {
                id: factColumn

                x: body.leftX + body.leftWidth + body.columnGap
                y: Theme.dp(24)
                width: body.inner - body.leftWidth - body.columnGap

                Label {
                    text: "Details"
                    color: Theme.textSecondary
                    bottomPadding: Theme.dp(12)
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }

                Repeater {
                    model: page.facts

                    Item {
                        width: factColumn.width
                        height: Math.max(Theme.dp(60), factValue.height + Theme.dp(26))

                        Label {
                            id: factName
                            y: Theme.dp(13)
                            width: Theme.dp(210)
                            text: modelData.label
                            color: Theme.textMuted
                            elide: Text.ElideRight
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }

                        Label {
                            id: factValue
                            x: Theme.dp(230)
                            y: Theme.dp(11)
                            width: parent.width - x
                            text: modelData.value
                            wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                            maximumLineCount: 3
                            elide: Text.ElideRight
                            font.pixelSize: Theme.dp(26)
                        }

                        Rectangle {
                            anchors.bottom: parent.bottom
                            width: parent.width
                            height: 1
                            color: Theme.hairline
                        }
                    }
                }
            }

            Item {
                id: shotsRow

                y: Math.max(about.y + about.height, factColumn.y + factColumn.height) + Theme.dp(60)
                width: body.width
                height: shotsCaption.height + Theme.dp(16) + Theme.dp(234)

                Label {
                    id: shotsCaption
                    x: body.leftX
                    visible: shots.visible
                    text: "Screenshots"
                    font.pixelSize: Theme.dp(26)
                }

                Item {
                    id: shots

                    readonly property real cardWidth: Theme.dp(416)
                    readonly property real pitch: cardWidth + Theme.dp(24)
                    readonly property real room: body.inner
                    readonly property real rowWidth: page.screenshots.length * pitch - Theme.dp(24)
                    readonly property real scrollX: Math.max(0, Math.min(page.shotIndex * pitch, rowWidth - room))

                    y: shotsCaption.height + Theme.dp(16)
                    width: parent.width
                    height: Theme.dp(234)
                    visible: page.screenshots.length > 0

                    Item {
                        x: body.leftX - shots.scrollX
                        width: shots.rowWidth
                        height: parent.height

                        Behavior on x {
                            NumberAnimation {
                                duration: Theme.durMove
                                easing.type: Easing.OutCubic
                            }
                        }

                        Repeater {
                            model: page.screenshots

                            HubCard {
                                x: index * shots.pitch
                                width: shots.cardWidth
                                height: shots.height
                                image: modelData
                                focused: page.activeFocus && page.zone === "shots" && index === page.shotIndex && !viewer.open
                                onPicked: {
                                    if (page.zone === "shots" && page.shotIndex === index) {
                                        page.openShot(index);
                                        return;
                                    }
                                    Sound.play("tick");
                                    page.zone = "shots";
                                    page.shotIndex = index;
                                    page.forceActiveFocus();
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

    GameShotViewer {
        id: viewer
        z: 10
        images: page.screenshots
        onClosed: {
            page.shotIndex = index;
            page.forceActiveFocus();
        }
    }
}
