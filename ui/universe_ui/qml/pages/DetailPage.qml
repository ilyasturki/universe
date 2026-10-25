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
    // A menu or a question over the page: its hints take the bar.
    property var overlayHints: null

    signal launchRequested(var game)
    signal closeRequested
    signal menuRequested(var game, Item anchor)

    readonly property bool updatable: game !== null && game.updateAction === "update"
    readonly property var pills: ["play", "favourite"].concat(updatable ? ["update"] : []).concat(["more"])
    readonly property string action: pills[Math.max(0, Math.min(actionIndex, pills.length - 1))]
    readonly property var actionLabels: ({
            update: "Update",
            more: "More"
        })
    readonly property string updateMark: game ? game.updateMark : ""
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

    function bodyTop() {
        return body.y - page.ledge;
    }

    function viewBottom() {
        return pageScroll.heading() + flick.height - hintBar.height;
    }

    function scrollTo(y) {
        pageScroll.to(y);
    }

    function aboutScroll() {
        var end = Math.max(bodyTop(), body.y + about.height + Theme.dp(40) - flick.height + hintBar.height);
        return Math.max(bodyTop(), Math.min(end, pageScroll.heading()));
    }

    function revealShot() {
        var card = shotCards.itemAt(shotIndex);
        if (!card)
            return;
        var top = body.y + shots.y + card.y, bottom = top + card.height;
        if (top < pageScroll.heading() + page.ledge)
            scrollTo(top - page.ledge);
        else if (bottom + Theme.dp(40) > viewBottom())
            scrollTo(bottom + Theme.dp(40) - flick.height + hintBar.height);
    }

    // Into the column the shot the page already shows: entering it moves nothing on screen.
    function enterShots() {
        var top = pageScroll.heading() + page.ledge - body.y - shots.y;
        var landing = 0;
        while (landing < shotCards.count - 1 && shotCards.itemAt(landing).y + shotCards.itemAt(landing).height <= top)
            landing++;
        section = 2;
        shotIndex = landing;
        revealShot();
    }

    function stepDown(held) {
        if (section === 0) {
            if (held)
                return "";
            if (hasAbout) {
                section = 1;
                scrollTo(bodyTop());
                return "section";
            }
            if (hasShots) {
                enterShots();
                return "section";
            }
            return "";
        }
        if (section === 1) {
            var aboutBottom = body.y + about.height + Theme.dp(40);
            if (aboutBottom > viewBottom() + 1) {
                scrollTo(Math.min(pageScroll.heading() + Theme.dp(320), aboutBottom - flick.height + hintBar.height));
                return "scroll";
            }
            return "";
        }
        if (section === 2 && shotIndex < screenshots.length - 1) {
            shotIndex++;
            revealShot();
            return "shot";
        }
        return "";
    }

    function stepUp(held) {
        if (section === 2) {
            if (shotIndex > 0) {
                shotIndex--;
                revealShot();
                return "shot";
            }
            if (held)
                return "";
            section = 0;
            scrollTo(0);
            return "section";
        }
        if (section === 1) {
            if (pageScroll.heading() > bodyTop() + 1) {
                scrollTo(Math.max(bodyTop(), pageScroll.heading() - Theme.dp(320)));
                return "scroll";
            }
            if (held)
                return "";
            section = 0;
            scrollTo(0);
            return "section";
        }
        return "";
    }

    function stepAcross(step) {
        if (section === 0) {
            actionIndex = Sound.stepped(actionIndex, step, pills.length);
        } else if (section === 1 && step > 0 && hasShots) {
            enterShots();
            Sound.panel();
        } else if (section === 2 && step < 0 && hasAbout) {
            section = 1;
            scrollTo(aboutScroll());
            Sound.panel();
        } else {
            Sound.edge();
        }
    }

    function stepSound(moved) {
        if (moved === "section")
            Sound.panel();
        else if (moved === "scroll" || moved === "shot")
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

                StatRow {
                    id: stats
                    objectName: "stats"
                    anchors.top: heroLogo.bottom
                    anchors.topMargin: Theme.dp(22)
                    game: page.game
                }

                Row {
                    id: actions
                    anchors.top: stats.visible ? stats.bottom : heroLogo.bottom
                    anchors.topMargin: Theme.dp(stats.visible ? 30 : 34)
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
                        visible: page.updatable
                        ghost: true
                        icon: "download"
                        label: page.actionLabels.update
                        focused: actions.active && page.action === "update"
                        dimmed: actions.active && page.action !== "update"
                        onPicked: page.pointToAction("update")
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

            Item {
                id: body

                readonly property real shotsWidth: Math.round(width * 0.32)

                x: page.sideMargin
                y: page.heroHeight + Theme.dp(44)
                width: parent.width - page.sideMargin * 2
                height: Math.max(about.visible ? about.height : 0, shots.visible ? shots.height : 0)

                Text {
                    id: about

                    readonly property bool focused: page.section === 1 && !page.lightbox

                    width: body.width - body.shotsWidth - Theme.dp(100)
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

                // Sized by bindings, not a Column's layout: a key pressed before the first frame scrolls a page of its full height.
                Item {
                    id: shots

                    readonly property bool active: page.section === 2 && !page.lightbox
                    readonly property real cardHeight: Math.round(width * 9 / 16)
                    readonly property real spacing: Theme.dp(24)

                    objectName: "shots"
                    x: body.width - width
                    width: body.shotsWidth
                    height: Math.max(0, page.screenshots.length * (cardHeight + spacing) - spacing)
                    visible: page.hasShots

                    Repeater {
                        id: shotCards

                        model: page.screenshots

                        ShotCard {
                            y: index * (shots.cardHeight + shots.spacing)
                            width: shots.width
                            height: shots.cardHeight
                            source: modelData
                            focused: shots.active && index === page.shotIndex
                            dimmed: shots.active && index !== page.shotIndex

                            Pointer {
                                current: shots.active && index === page.shotIndex
                                radius: Theme.dp(14)
                                onPicked: {
                                    page.section = 2;
                                    page.shotIndex = index;
                                }
                            }
                        }
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
        shown: page.section !== 0
        from: page.bodyTop()
        to: body.y + (page.section === 2 ? shots.height : about.height)
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
            } else if (page.action === "update") {
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
            if (event.isAutoRepeat && page.section !== 0)
                return;
            page.stepAcross(event.key === Qt.Key_Left ? -1 : 1);
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
