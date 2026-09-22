import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    property var game: null

    signal launchRequested(var game)
    signal closeRequested
    signal menuRequested(var game, Item anchor)
    signal recordingsRequested(var game)
    signal journalRequested(var game)
    signal screenshotsRequested(var game)
    signal sessionsRequested(var game)

    readonly property int recordingCount: game && filed >= 0 ? (api.universe.recordings(game.id) || []).length : 0
    readonly property int entryCount: game && filed >= 0 ? (api.universe.journal(game.id) || []).length : 0
    readonly property int shotCount: game && filed >= 0 ? (api.universe.screenshots(game.id) || []).length : 0
    property int filed: 0
    readonly property bool played: game !== null && game.playCount > 0
    readonly property var pills: ["play", "favourite"].concat(shotCount > 0 ? ["shots"] : [], recordingCount > 0 ? ["recordings"] : [], entryCount > 0 ? ["journal"] : [], played ? ["sessions"] : [])
    readonly property string action: pills[Math.max(0, Math.min(actionIndex, pills.length - 1))] || "play"
    readonly property string acceptLabel: action === "favourite" ? favouriteLabel : action === "shots" ? "Screenshots" : action === "recordings" ? "Recordings" : action === "journal" ? "Journal" : action === "sessions" ? "Sessions" : game && game.playTime > 0 ? "Continue" : "Play"

    readonly property real scrollY: flick.contentY

    readonly property var screenshots: game && game.assets.screenshotList ? game.assets.screenshotList : []
    readonly property string description: game ? (game.description || game.summary || "") : ""
    readonly property var facts: {
        if (!game)
            return [];
        var out = [];
        if (game.developerList.length > 0)
            out.push({
                label: "DEVELOPER",
                value: game.developerList.join(", ")
            });
        if (game.publisherList.length > 0)
            out.push({
                label: "PUBLISHER",
                value: game.publisherList.join(", ")
            });
        if (game.extra["metacritic"] !== undefined)
            out.push({
                label: "METACRITIC",
                value: String(game.extra["metacritic"][0])
            });
        var hours = function (v) {
            return (Number(v) >= 10 ? Math.round(Number(v)) : Number(v).toFixed(1)) + " h";
        };
        var hltb = [["hltb-main", "Main"], ["hltb-extra", "Extra"], ["hltb-completionist", "100%"]].filter(function (k) {
            return game.extra[k[0]] !== undefined;
        }).map(function (k) {
            return k[1] + " " + hours(game.extra[k[0]][0]);
        });
        if (hltb.length > 0)
            out.push({
                label: "HOW LONG TO BEAT",
                value: hltb.join("  ·  ")
            });
        return out;
    }

    // 0 hero actions, 1 about, 2 screenshots
    property int section: 0
    property int actionIndex: 0
    property int shotIndex: 0
    property bool lightbox: false

    readonly property real sideMargin: Theme.dp(90)
    readonly property real heroHeight: Theme.dp(Theme.heroDetail)
    readonly property real ledge: Theme.dp(150)

    readonly property bool hasAbout: description !== "" || facts.length > 0
    readonly property bool hasShots: screenshots.length > 0

    readonly property var hints: lightbox ? [
        {
            glyph: "dpad",
            label: "Previous / next"
        },
        {
            glyph: "B",
            label: "Close"
        }
    ] : [
        {
            glyph: "A",
            label: section === 2 ? "View" : section === 1 ? "Play" : acceptLabel
        },
        {
            glyph: "Y",
            label: favouriteLabel
        },
        {
            glyph: "Start",
            label: "More"
        },
        {
            glyph: "B",
            label: "Back"
        }
    ]

    readonly property string favouriteLabel: game && game.favorite ? "Remove from favourites" : "Add to favourites"

    onGameChanged: reset()

    Connections {
        target: api.universe
        function onRecordingFiled(session, id, path) {
            page.filed++;
        }
        function onEntryWritten(session, id) {
            page.filed++;
        }
        function onLibraryChanged(ids) {
            if (page.game && (ids.length === 0 || ids.indexOf(page.game.id) >= 0))
                page.filed++;
        }
    }

    function reset() {
        section = 0;
        actionIndex = 0;
        shotIndex = 0;
        lightbox = false;
        flick.contentY = 0;
    }

    function sectionTop(which) {
        return which === 1 ? body.y + about.y - page.ledge : which === 2 ? body.y + shots.y - page.ledge : 0;
    }

    function maxScroll() {
        return Math.max(0, flick.contentHeight - flick.height);
    }

    function scrollTo(y) {
        flick.contentY = Math.max(0, Math.min(y, maxScroll()));
    }

    function goTo(which) {
        section = which;
        scrollTo(sectionTop(which));
    }

    function stepDown() {
        if (section === 0) {
            if (hasAbout) {
                goTo(1);
                return "section";
            }
            if (hasShots) {
                goTo(2);
                return "section";
            }
            return "";
        }
        if (section === 1) {
            var aboutBottom = body.y + about.y + about.height + Theme.dp(40);
            var viewBottom = flick.contentY + flick.height - hintBar.height;
            if (aboutBottom > viewBottom + 1) {
                scrollTo(Math.min(flick.contentY + Theme.dp(320), aboutBottom - flick.height + hintBar.height));
                return "scroll";
            }
            if (hasShots) {
                goTo(2);
                return "section";
            }
        }
        return "";
    }

    function stepUp() {
        if (section === 2) {
            goTo(hasAbout ? 1 : 0);
            return "section";
        }
        if (section === 1) {
            if (flick.contentY > sectionTop(1) + 1) {
                scrollTo(Math.max(sectionTop(1), flick.contentY - Theme.dp(320)));
                return "scroll";
            }
            goTo(0);
            return "section";
        }
        return "";
    }

    function stepSound(moved) {
        if (moved === "section")
            Sound.panel();
        else if (moved === "scroll")
            Sound.tick();
        else
            Sound.edge();
    }

    function toggleFavourite() {
        game.favorite = !game.favorite;
        Sound.favourite(game.favorite);
    }

    function stepShot(step) {
        shotIndex = Sound.stepped(shotIndex, step, screenshots.length);
    }

    // The mouse lands a section without the scroll or the sound a step brings.
    function pointToAction(name) {
        section = 0;
        actionIndex = Math.max(0, pills.indexOf(name));
    }

    component InfoPill: Rectangle {
        height: Theme.dp(41)
        radius: height / 2
        color: Qt.rgba(1, 1, 1, 0.10)
        border.width: 1
        border.color: Theme.surfaceBorder
    }

    Flickable {
        id: flick

        anchors.fill: parent
        contentWidth: width
        contentHeight: content.height
        interactive: false
        // The default overshoot fixup fights the contentY Behavior.
        boundsBehavior: Flickable.StopAtBounds

        Behavior on contentY {
            id: pageEase
            Ease {
                duration: Theme.durView
            }
        }

        Wheel {
            ease: pageEase
        }

        Item {
            id: content

            width: flick.width
            height: body.y + body.height + hintBar.height + Theme.dp(50)

            Item {
                id: heroContent

                anchors.left: parent.left
                anchors.leftMargin: page.sideMargin
                y: page.heroHeight - Theme.dp(52) - height
                width: Theme.dp(1000)
                height: actions.y + actions.height
                opacity: Math.max(0, 1 - flick.contentY / Theme.dp(300))

                Row {
                    id: chips
                    spacing: Theme.dp(14)

                    InfoPill {
                        visible: page.game !== null && page.game.collections.count > 0
                        width: platformIcon.width + Theme.dp(36)

                        PlatformIcon {
                            id: platformIcon
                            anchors.centerIn: parent
                            game: page.game
                            size: Theme.dp(24)
                            labelSize: Theme.dp(20)
                        }
                    }

                    InfoPill {
                        visible: page.game !== null && page.game.runner !== ""
                        width: runnerBadge.width + Theme.dp(36)

                        RunnerBadge {
                            id: runnerBadge
                            anchors.centerIn: parent
                            runner: page.game ? page.game.runner : ""
                            name: page.game ? page.game.runnerName : ""
                            size: Theme.dp(24)
                            labelSize: Theme.dp(20)
                        }
                    }

                    Repeater {
                        model: page.game ? page.game.genreList.slice(0, 3).concat(page.game.players > 1 ? [page.game.players + " players"] : []) : []

                        InfoPill {
                            width: chipText.width + Theme.dp(40)

                            Text {
                                id: chipText
                                anchors.centerIn: parent
                                text: modelData
                                color: Theme.text
                                font.family: Theme.sans
                                font.weight: Font.Medium
                                font.pixelSize: Theme.dp(20)
                            }
                        }
                    }
                }

                HeroLogo {
                    id: heroLogo
                    anchors.top: chips.bottom
                    anchors.topMargin: Theme.dp(24)
                    game: page.game
                    logoWidth: Theme.dp(460)
                    logoHeight: Theme.dp(160)
                    titleWidth: parent.width
                }

                Row {
                    id: actions
                    anchors.top: heroLogo.bottom
                    anchors.topMargin: Theme.dp(34)
                    // Clears the focused pill's ring, which reaches 20 past its edge.
                    spacing: Theme.dp(36)

                    readonly property bool active: page.section === 0 && !page.lightbox

                    PillButton {
                        label: page.game && page.game.playTime > 0 ? "Continue" : "Play"
                        focused: actions.active && page.action === "play"
                        dimmed: actions.active && page.action !== "play"
                        onPicked: page.pointToAction("play")
                    }

                    Rectangle {
                        id: heart

                        readonly property bool focused: actions.active && page.action === "favourite"

                        width: Theme.dp(78)
                        height: Theme.dp(78)
                        radius: width / 2
                        color: Theme.surface
                        border.width: Math.max(1, Theme.dp(2))
                        border.color: Theme.surfaceBorder
                        opacity: actions.active && !focused ? 0.5 : 1.0
                        scale: focused ? 1.04 : 1.0

                        Behavior on opacity {
                            Ease {
                                duration: Theme.durQuick
                            }
                        }
                        Behavior on scale {
                            Ease {
                                easing.type: Easing.OutQuint
                            }
                        }

                        Loader {
                            anchors.fill: parent
                            active: heart.focused
                            sourceComponent: FocusRing {
                                cornerRadius: heart.radius
                                gapWidth: Theme.dp(Theme.ringGap)
                            }
                        }

                        MenuGlyph {
                            anchors.centerIn: parent
                            width: Theme.dp(28)
                            height: Theme.dp(28)
                            kind: page.game && page.game.favorite ? "heart" : "heart-outline"
                            tint: "#f2f3f5"
                        }

                        Pointer {
                            direct: true
                            radius: heart.radius
                            onPicked: page.pointToAction("favourite")
                        }
                    }

                    PillButton {
                        visible: page.shotCount > 0
                        label: "Screenshots"
                        icon: "camera"
                        ghost: true
                        focused: actions.active && page.action === "shots"
                        dimmed: actions.active && page.action !== "shots"
                        onPicked: page.pointToAction("shots")
                    }

                    PillButton {
                        visible: page.recordingCount > 0
                        label: "Recordings"
                        icon: "film"
                        ghost: true
                        focused: actions.active && page.action === "recordings"
                        dimmed: actions.active && page.action !== "recordings"
                        onPicked: page.pointToAction("recordings")
                    }

                    PillButton {
                        visible: page.entryCount > 0
                        label: "Journal"
                        icon: "book"
                        ghost: true
                        focused: actions.active && page.action === "journal"
                        dimmed: actions.active && page.action !== "journal"
                        onPicked: page.pointToAction("journal")
                    }

                    PillButton {
                        visible: page.played
                        label: "Sessions"
                        icon: "terminal"
                        ghost: true
                        focused: actions.active && page.action === "sessions"
                        dimmed: actions.active && page.action !== "sessions"
                        onPicked: page.pointToAction("sessions")
                    }
                }
            }

            Column {
                id: body

                anchors.left: parent.left
                anchors.right: parent.right
                anchors.leftMargin: page.sideMargin
                anchors.rightMargin: page.sideMargin
                y: page.heroHeight + Theme.dp(44)
                spacing: Theme.dp(48)

                StatRow {
                    spacing: Theme.dp(74)
                    game: page.game
                }

                Item {
                    id: about

                    width: parent.width
                    height: Math.max(aboutText.height, factsColumn.height)
                    visible: page.hasAbout

                    readonly property bool focused: page.section === 1 && !page.lightbox

                    Pointer {
                        accept: false
                        wash: 0
                        onPicked: page.section = 1
                    }

                    Column {
                        id: aboutText
                        width: Theme.dp(1080)
                        spacing: Theme.dp(16)
                        visible: page.description !== ""

                        CapsLabel {
                            text: "ABOUT"
                            tracking: 0.11
                            color: about.focused ? Theme.textSecondary : Theme.textMuted
                        }

                        Text {
                            width: parent.width
                            text: page.description
                            color: about.focused ? Theme.text : Theme.textSecondary
                            font.family: Theme.sans
                            font.pixelSize: Theme.dp(24)
                            lineHeight: 1.5
                            wrapMode: Text.WordWrap

                            Behavior on color {
                                ColorEase {
                                    duration: Theme.durBase
                                }
                            }
                        }
                    }

                    Column {
                        id: factsColumn
                        anchors.right: parent.right
                        width: Theme.dp(520)
                        spacing: Theme.dp(28)

                        Repeater {
                            model: page.facts

                            Column {
                                width: factsColumn.width
                                spacing: Theme.dp(8)

                                CapsLabel {
                                    text: modelData.label
                                    tracking: 0.11
                                }
                                Text {
                                    width: parent.width
                                    text: modelData.value
                                    color: Theme.text
                                    font.family: Theme.sans
                                    font.weight: Font.DemiBold
                                    font.pixelSize: Theme.dp(26)
                                    wrapMode: Text.WordWrap
                                }
                            }
                        }
                    }
                }

                ScreenshotStrip {
                    id: shots

                    width: parent.width
                    images: page.screenshots
                    index: page.shotIndex
                    focused: page.section === 2 && !page.lightbox
                    sideMargin: page.sideMargin
                    onPointed: function (i) {
                        page.section = 2;
                        page.shotIndex = i;
                    }
                }
            }
        }
    }

    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        height: hintBar.height + Theme.dp(70)
        gradient: Gradient {
            GradientStop {
                position: 0.0
                color: Qt.rgba(0.055, 0.059, 0.075, 0.0)
            }
            GradientStop {
                position: 0.45
                color: Qt.rgba(0.055, 0.059, 0.075, 0.92)
            }
            GradientStop {
                position: 1.0
                color: Theme.ground
            }
        }
    }

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(52)
        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.bottom: hintBar.top
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

    Lightbox {
        anchors.fill: parent
        images: page.screenshots
        index: page.shotIndex
        open: page.lightbox
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat && !(event.key === Qt.Key_Up || event.key === Qt.Key_Down || event.key === Qt.Key_Left || event.key === Qt.Key_Right))
            return;

        event.accepted = true;
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        if (page.lightbox) {
            if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                Sound.cancel();
                page.lightbox = false;
            } else if (arrow) {
                page.stepShot(event.key === Qt.Key_Left ? -1 : 1);
            }
        } else if (api.keys.isAccept(event)) {
            if (!page.game)
                return;
            if (page.section === 2) {
                Sound.enter();
                page.lightbox = true;
            } else if (page.section === 0 && page.action === "favourite") {
                page.toggleFavourite();
            } else if (page.section === 0 && page.action === "shots") {
                page.screenshotsRequested(page.game);
            } else if (page.section === 0 && page.action === "recordings") {
                page.recordingsRequested(page.game);
            } else if (page.section === 0 && page.action === "journal") {
                page.journalRequested(page.game);
            } else if (page.section === 0 && page.action === "sessions") {
                page.sessionsRequested(page.game);
            } else {
                page.launchRequested(page.game);
            }
        } else if (api.keys.isFilters(event)) {
            if (page.game)
                page.toggleFavourite();
        } else if (api.keys.isCancel(event)) {
            page.closeRequested();
        } else if (event.key === Qt.Key_Down) {
            page.stepSound(page.stepDown());
        } else if (event.key === Qt.Key_Up) {
            page.stepSound(page.stepUp());
        } else if (arrow) {
            var step = event.key === Qt.Key_Left ? -1 : 1;
            if (page.section === 0)
                page.actionIndex = Sound.stepped(page.actionIndex, step, page.pills.length);
            else if (page.section === 2)
                page.stepShot(step);
            else
                Sound.edge();
        } else if (api.keys.isMenu(event)) {
            if (page.game)
                page.menuRequested(page.game, heroLogo);
        }
    }
}
