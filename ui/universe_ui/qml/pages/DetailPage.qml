import QtQuick
import "../core"
import "../sound"
import "../core/Format.js" as Format
import "../ui"

FocusScope {
    id: page

    objectName: "detailPage"
    focus: true

    property var game: null
    // The game's screenshots, recordings and journal entries as the game menu lists them; set by the theme.
    property var media: []
    // A menu or a question over the page: its hints take the bar.
    property var overlayHints: null

    signal launchRequested(var game)
    signal closeRequested
    signal menuRequested(var game, Item anchor)
    signal settingsRequested(var game)
    signal mediaRequested(var game, Item anchor)
    signal achievementsRequested(var game)

    readonly property string updateAction: game ? game.updateAction : ""
    readonly property var pills: ["play", "favourite", "settings"].concat(media.length > 0 ? ["media"] : []).concat(game && game.achievementsTotal > 0 ? ["achievements"] : []).concat(updateAction ? [updateAction] : []).concat(["more"])
    readonly property string action: pills[Math.max(0, Math.min(actionIndex, pills.length - 1))]
    readonly property var actionLabels: ({
            settings: "Game settings",
            media: "Media",
            achievements: "Achievements",
            update: "Update",
            "mark-current": "Mark as current",
            more: "More"
        })
    readonly property var session: api.universe.currentSession
    readonly property string playLabel: Format.playLabel(game, session && session.id !== undefined ? session.id : "")

    readonly property real scrollY: flick.contentY

    readonly property var screenshots: game && game.assets.screenshotList ? game.assets.screenshotList : []
    readonly property string description: game ? (game.description || game.summary || "") : ""

    // 0 hero actions, 1 about, 2 screenshots
    property int section: 0
    property int actionIndex: 0
    property int shotIndex: 0
    property bool lightbox: false

    readonly property real sideMargin: Theme.dp(90)
    readonly property real heroHeight: Theme.dp(Theme.heroDetail)
    readonly property real ledge: Theme.dp(150)

    readonly property bool hasAbout: description !== ""
    readonly property bool hasShots: screenshots.length > 0

    readonly property bool onHeart: section === 0 && action === "favourite"
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
            label: section === 2 ? "View" : section === 1 ? playLabel : onHeart ? (game && game.favorite ? "Remove from favourites" : "Add to favourites") : actionLabels[action] || playLabel
        }
    ].concat(onHeart ? [] : [
        {
            glyph: "Y",
            label: game && game.favorite ? "Unfavourite" : "Favourite"
        }
    ]).concat([
        {
            glyph: "Start",
            label: "More"
        },
        {
            glyph: "B",
            label: "Back"
        }
    ])

    onGameChanged: reset()

    function reset() {
        if (game) {
            api.screens.achievements.prime(game.id, game.source, game.achievementsTotal);
            api.universe.checkVersions(game.id);
        }
        section = 0;
        actionIndex = 0;
        shotIndex = 0;
        lightbox = false;
        pageScroll.place(0);
    }

    function sectionTop(which) {
        return which === 1 ? body.y + about.y - page.ledge : which === 2 ? body.y + shots.y - page.ledge : 0;
    }

    function scrollTo(y) {
        pageScroll.to(y);
    }

    function goTo(which) {
        section = which;
        scrollTo(sectionTop(which));
    }

    function stepDown(held) {
        if (section === 0) {
            if (held)
                return "";
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
            var viewBottom = pageScroll.heading() + flick.height - hintBar.height;
            if (aboutBottom > viewBottom + 1) {
                scrollTo(Math.min(pageScroll.heading() + Theme.dp(320), aboutBottom - flick.height + hintBar.height));
                return "scroll";
            }
            if (hasShots && !held) {
                goTo(2);
                return "section";
            }
        }
        return "";
    }

    function stepUp(held) {
        if (held && section !== 1)
            return "";
        if (section === 2) {
            goTo(hasAbout ? 1 : 0);
            return "section";
        }
        if (section === 1) {
            if (pageScroll.heading() > sectionTop(1) + 1) {
                scrollTo(Math.max(sectionTop(1), pageScroll.heading() - Theme.dp(320)));
                return "scroll";
            }
            if (held)
                return "";
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

    Flickable {
        id: flick

        anchors.fill: parent
        contentWidth: width
        contentHeight: content.height
        interactive: false
        boundsBehavior: Flickable.StopAtBounds

        Scroller {
            id: pageScroll
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

                HeroLogo {
                    id: heroLogo
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
                        label: page.playLabel
                        focused: actions.active && page.action === "play"
                        dimmed: actions.active && page.action !== "play"
                        onPicked: page.pointToAction("play")
                    }

                    RoundButton {
                        icon: page.game && page.game.favorite ? "heart" : "heart-outline"
                        focused: actions.active && page.action === "favourite"
                        dimmed: actions.active && page.action !== "favourite"
                        onPicked: page.pointToAction("favourite")
                    }

                    PillButton {
                        ghost: true
                        icon: "sliders"
                        label: page.actionLabels.settings
                        focused: actions.active && page.action === "settings"
                        dimmed: actions.active && page.action !== "settings"
                        onPicked: page.pointToAction("settings")
                    }

                    PillButton {
                        id: mediaButton
                        visible: page.pills.indexOf("media") >= 0
                        ghost: true
                        icon: "photos"
                        label: page.actionLabels.media
                        focused: actions.active && page.action === "media"
                        dimmed: actions.active && page.action !== "media"
                        onPicked: page.pointToAction("media")
                    }

                    PillButton {
                        visible: page.pills.indexOf("achievements") >= 0
                        ghost: true
                        icon: "trophy"
                        label: page.actionLabels.achievements
                        focused: actions.active && page.action === "achievements"
                        dimmed: actions.active && page.action !== "achievements"
                        onPicked: page.pointToAction("achievements")
                    }

                    PillButton {
                        visible: page.updateAction !== ""
                        ghost: true
                        icon: page.updateAction === "update" ? "download" : "check"
                        label: page.actionLabels[page.updateAction] || ""
                        focused: actions.active && visible && page.action === page.updateAction
                        dimmed: actions.active && !focused
                        onPicked: page.pointToAction(page.updateAction)
                    }

                    RoundButton {
                        id: moreButton
                        icon: "more"
                        focused: actions.active && page.action === "more"
                        dimmed: actions.active && page.action !== "more"
                        onPicked: page.pointToAction("more")
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
                    objectName: "stats"
                    spacing: Theme.dp(74)
                    game: page.game
                }

                Text {
                    id: about

                    readonly property bool focused: page.section === 1 && !page.lightbox

                    width: Theme.dp(1080)
                    visible: page.hasAbout
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

                    Pointer {
                        accept: false
                        wash: 0
                        onPicked: page.section = 1
                    }
                }

                ScreenshotStrip {
                    id: shots

                    label: ""
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
        anchors.topMargin: page.ledge
        anchors.bottom: hintBar.top
        flickable: flick
        shown: page.section === 1
        from: page.sectionTop(1)
        to: body.y + about.y + about.height
        window: flick.height - hintBar.height
    }

    HintBar {
        id: hintBar
        objectName: "hintBar"

        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        hints: page.overlayHints || page.hints
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
            } else if (page.section !== 0 || page.action === "play") {
                page.launchRequested(page.game);
            } else if (page.action === "favourite") {
                page.toggleFavourite();
            } else if (page.action === "settings") {
                page.settingsRequested(page.game);
            } else if (page.action === "media") {
                page.mediaRequested(page.game, mediaButton);
            } else if (page.action === "achievements") {
                page.achievementsRequested(page.game);
            } else if (page.action === page.updateAction) {
                Sound.enter();
                api.updateGame(page.game.id);
            } else {
                page.menuRequested(page.game, moreButton);
            }
        } else if (api.keys.isFilters(event)) {
            if (page.game)
                page.toggleFavourite();
        } else if (api.keys.isCancel(event)) {
            page.closeRequested();
        } else if (event.key === Qt.Key_Down) {
            page.stepSound(page.stepDown(event.isAutoRepeat));
        } else if (event.key === Qt.Key_Up) {
            page.stepSound(page.stepUp(event.isAutoRepeat));
        } else if (arrow) {
            var step = event.key === Qt.Key_Left ? -1 : 1;
            if (page.section === 0)
                page.actionIndex = Sound.stepped(page.actionIndex, step, page.pills.length);
            else if (page.section === 2)
                page.stepShot(step);
            else
                Sound.edge();
        } else if (api.keys.isMenu(event)) {
            if (!page.game)
                return;
            // The menu sits beside the More button: a scrolled page snaps back under it.
            if (pageScroll.heading() > 0) {
                page.section = 0;
                pageScroll.place(0);
            }
            page.menuRequested(page.game, moreButton);
        }
    }
}
