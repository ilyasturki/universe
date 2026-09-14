import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    property var game: null

    signal launchRequested(var game)
    signal closeRequested()
    signal menuRequested(var game, Item anchor)
    signal recordingsRequested(var game)
    signal journalRequested(var game)

    readonly property int recordingCount: game && filed >= 0 ? (api.universe.recordings(game.id) || []).length : 0
    readonly property int entryCount: game && filed >= 0 ? (api.universe.journal(game.id) || []).length : 0
    property int filed: 0
    readonly property var pills: [ "play", "favourite" ].concat(recordingCount > 0 ? [ "recordings" ] : [], entryCount > 0 ? [ "journal" ] : [])
    readonly property string action: pills[Math.max(0, Math.min(actionIndex, pills.length - 1))] || "play"
    readonly property string acceptLabel: action === "favourite" ? favouriteLabel
        : action === "recordings" ? "Recordings" : action === "journal" ? "Journal"
        : game && game.playTime > 0 ? "Continue" : "Play"

    // The shell's hero reads this to pull its art up and dim it as the page scrolls.
    readonly property real scrollY: flick.contentY

    readonly property var screenshots: game && game.assets.screenshotList ? game.assets.screenshotList : []
    readonly property string description: game ? (game.description || game.summary || "") : ""
    readonly property var facts: {
        if (!game)
            return [];
        var out = [];
        if (game.developerList.length > 0)
            out.push({ label: "DEVELOPER", value: game.developerList.join(", ") });
        if (game.publisherList.length > 0)
            out.push({ label: "PUBLISHER", value: game.publisherList.join(", ") });
        if (game.extra["metacritic"] !== undefined)
            out.push({ label: "METACRITIC", value: String(game.extra["metacritic"][0]) });
        var hltb = [];
        var hours = function(v) { return (Number(v) >= 10 ? Math.round(Number(v)) : Number(v).toFixed(1)) + " h"; };
        if (game.extra["hltb-main"] !== undefined)
            hltb.push("Main " + hours(game.extra["hltb-main"][0]));
        if (game.extra["hltb-extra"] !== undefined)
            hltb.push("Extra " + hours(game.extra["hltb-extra"][0]));
        if (game.extra["hltb-completionist"] !== undefined)
            hltb.push("100% " + hours(game.extra["hltb-completionist"][0]));
        if (hltb.length > 0)
            out.push({ label: "HOW LONG TO BEAT", value: hltb.join("  ·  ") });
        return out;
    }

    // 0 hero actions, 1 about, 2 screenshots
    property int section: 0
    property int actionIndex: 0
    property int shotIndex: 0
    property bool lightbox: false

    readonly property real sideMargin: Theme.dp(90)
    readonly property real heroHeight: Theme.dp(Theme.heroDetail)
    // What stays of the hero above a section once it has been scrolled to.
    readonly property real ledge: Theme.dp(150)

    readonly property bool hasAbout: description !== "" || facts.length > 0
    readonly property bool hasShots: screenshots.length > 0

    readonly property var hints: lightbox
        ? [ { glyph: "dpad", label: "Previous / next" }, { glyph: "B", label: "Close" } ]
        : [ { glyph: "A", label: section === 2 ? "View" : section === 1 ? "Play" : acceptLabel },
            { glyph: "Y", label: favouriteLabel },
            { glyph: "Start", label: "More" },
            { glyph: "B", label: "Back" } ]

    readonly property string favouriteLabel: game && game.favorite ? "Remove from favourites" : "Add to favourites"

    onGameChanged: reset()

    Connections {
        target: api.universe
        function onRecordingFiled(session, id, path) { page.filed++; }
        function onEntryWritten(session, id) { page.filed++; }
    }

    function reset() {
        section = 0;
        actionIndex = 0;
        shotIndex = 0;
        lightbox = false;
        flick.contentY = 0;
    }

    function sectionTop(which) {
        if (which === 1)
            return body.y + about.y - page.ledge;
        if (which === 2)
            return body.y + shots.y - page.ledge;
        return 0;
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

    // Both return what moved: "section", "scroll" or "" — the sound follows it.
    function stepDown() {
        if (section === 0) {
            if (hasAbout) { goTo(1); return "section"; }
            if (hasShots) { goTo(2); return "section"; }
            return "";
        }
        if (section === 1) {
            // A long description is read before the page moves on past it.
            var aboutBottom = body.y + about.y + about.height + Theme.dp(40);
            var viewBottom = flick.contentY + flick.height - hintBar.height;
            if (aboutBottom > viewBottom + 1) {
                scrollTo(Math.min(flick.contentY + Theme.dp(320), aboutBottom - flick.height + hintBar.height));
                return "scroll";
            }
            if (hasShots) { goTo(2); return "section"; }
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
        if (moved === "section") Sound.panel();
        else if (moved === "scroll") Sound.tick();
        else Sound.edge();
    }

    function toggleFavourite() {
        game.favorite = !game.favorite;
        Sound.favourite(game.favorite);
    }

    function stepShot(step) {
        var next = Math.max(0, Math.min(screenshots.length - 1, shotIndex + step));
        next === shotIndex ? Sound.edge() : Sound.tick();
        shotIndex = next;
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
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
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
                // Leaves with the hero rather than lingering over the collapsed art.
                opacity: Math.max(0, 1 - flick.contentY / Theme.dp(300))

                Row {
                    id: chips
                    spacing: Theme.dp(14)

                    Rectangle {
                        visible: page.game !== null && page.game.collections.count > 0
                        height: Theme.dp(41)
                        width: platformIcon.width + Theme.dp(36)
                        radius: height / 2
                        color: Qt.rgba(1, 1, 1, 0.10)
                        border.width: 1
                        border.color: Theme.surfaceBorder

                        PlatformIcon {
                            id: platformIcon
                            anchors.centerIn: parent
                            game: page.game
                            size: Theme.dp(24)
                            labelSize: Theme.dp(20)
                        }
                    }

                    Rectangle {
                        visible: page.game !== null && page.game.runner !== ""
                        height: Theme.dp(41)
                        width: runnerBadge.width + Theme.dp(36)
                        radius: height / 2
                        color: Qt.rgba(1, 1, 1, 0.10)
                        border.width: 1
                        border.color: Theme.surfaceBorder

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
                        model: page.game
                            ? page.game.genreList.slice(0, 3).concat(
                                  page.game.players > 1 ? [ page.game.players + " players" ] : [])
                            : []

                        Rectangle {
                            height: Theme.dp(41)
                            width: chipText.width + Theme.dp(40)
                            radius: height / 2
                            color: Qt.rgba(1, 1, 1, 0.10)
                            border.width: 1
                            border.color: Theme.surfaceBorder

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
                            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                        }
                        Behavior on scale {
                            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
                        }

                        Loader {
                            anchors.fill: parent
                            active: heart.focused
                            sourceComponent: FocusRing {
                                cornerRadius: heart.radius
                                gapWidth: Theme.dp(6)
                            }
                        }

                        Canvas {
                            anchors.centerIn: parent
                            width: Theme.dp(28)
                            height: Theme.dp(28)

                            readonly property bool filled: page.game ? page.game.favorite : false
                            onFilledChanged: requestPaint()

                            onPaint: {
                                var ctx = getContext("2d");
                                ctx.reset();
                                var s = width / 24;
                                ctx.strokeStyle = "#f2f3f5";
                                ctx.fillStyle = "#f2f3f5";
                                ctx.lineWidth = 2 * s;
                                ctx.lineJoin = "round";
                                ctx.beginPath();
                                ctx.moveTo(12 * s, 20 * s);
                                ctx.bezierCurveTo(12 * s, 20 * s, 4.5 * s, 15.3 * s, 4.5 * s, 10.4 * s);
                                ctx.bezierCurveTo(4.5 * s, 7.2 * s, 9.2 * s, 5.4 * s, 12 * s, 7.6 * s);
                                ctx.bezierCurveTo(14.8 * s, 5.4 * s, 19.5 * s, 7.2 * s, 19.5 * s, 10.4 * s);
                                ctx.bezierCurveTo(19.5 * s, 15.3 * s, 12 * s, 20 * s, 12 * s, 20 * s);
                                ctx.closePath();
                                if (filled)
                                    ctx.fill();
                                else
                                    ctx.stroke();
                            }
                        }
                    }

                    PillButton {
                        visible: page.recordingCount > 0
                        label: "Recordings"
                        icon: "film"
                        ghost: true
                        focused: actions.active && page.action === "recordings"
                        dimmed: actions.active && page.action !== "recordings"
                    }

                    PillButton {
                        visible: page.entryCount > 0
                        label: "Journal"
                        icon: "book"
                        ghost: true
                        focused: actions.active && page.action === "journal"
                        dimmed: actions.active && page.action !== "journal"
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
                                ColorAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
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

                Column {
                    id: shots

                    width: parent.width
                    spacing: Theme.dp(18)
                    visible: page.hasShots

                    readonly property bool focused: page.section === 2 && !page.lightbox
                    readonly property real shotWidth: Theme.dp(336)
                    readonly property real shotHeight: Theme.dp(189)

                    CapsLabel {
                        text: "SCREENSHOTS"
                        tracking: 0.11
                        color: shots.focused ? Theme.textSecondary : Theme.textMuted
                    }

                    ListView {
                        id: shotStrip

                        // Room for the focus ring's halo inside the clip on every side.
                        readonly property real inset: Theme.dp(24)

                        x: -inset
                        width: parent.width + page.sideMargin + inset
                        height: shots.shotHeight + inset * 2
                        leftMargin: inset
                        orientation: ListView.Horizontal
                        spacing: Theme.dp(20)
                        model: page.screenshots
                        interactive: false
                        clip: true
                        currentIndex: page.shotIndex
                        boundsBehavior: Flickable.StopAtBounds
                        highlightFollowsCurrentItem: false

                        onCurrentIndexChanged: slide()
                        onWidthChanged: slide()

                        function slide() {
                            var pitch = shots.shotWidth + spacing;
                            var target = currentIndex * pitch - (width - page.sideMargin) * 0.5 + shots.shotWidth * 0.5;
                            contentX = Math.max(-inset, Math.min(target, Math.max(-inset, contentWidth - width + inset)));
                        }

                        Behavior on contentX {
                            NumberAnimation { duration: Theme.durNudge; easing.type: Easing.OutQuint }
                        }

                        delegate: Item {
                            width: shots.shotWidth
                            height: shotStrip.height

                            readonly property bool current: index === page.shotIndex && shots.focused

                            Rectangle {
                                id: shotCard
                                width: shots.shotWidth
                                height: shots.shotHeight
                                anchors.verticalCenter: parent.verticalCenter
                                radius: Theme.dp(10)
                                color: Theme.cardBase
                                clip: true
                                opacity: shots.focused && !current ? 0.6 : 1.0
                                scale: current ? 1.03 : 1.0

                                Behavior on opacity {
                                    NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                                }
                                Behavior on scale {
                                    NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
                                }

                                Image {
                                    anchors.fill: parent
                                    source: modelData
                                    fillMode: Image.PreserveAspectCrop
                                    asynchronous: true
                                    mipmap: true
                                }
                            }

                            Loader {
                                anchors.fill: shotCard
                                active: current
                                sourceComponent: FocusRing {
                                    cornerRadius: shotCard.radius
                                    gapWidth: Theme.dp(4)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Scrolled content runs out under the hint bar rather than into it.
    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        height: hintBar.height + Theme.dp(70)
        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(0.055, 0.059, 0.075, 0.0) }
            GradientStop { position: 0.45; color: Qt.rgba(0.055, 0.059, 0.075, 0.92) }
            GradientStop { position: 1.0; color: Theme.ground }
        }
    }

    HintBar {
        id: hintBar

        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        sideMargin: page.sideMargin
        showClock: true
        hints: page.hints
    }

    Lightbox {
        anchors.fill: parent
        images: page.screenshots
        index: page.shotIndex
        open: page.lightbox
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat && !(event.key === Qt.Key_Up || event.key === Qt.Key_Down
                                    || event.key === Qt.Key_Left || event.key === Qt.Key_Right))
            return;

        if (page.lightbox) {
            event.accepted = true;
            if (api.keys.isCancel(event) || api.keys.isAccept(event)) {
                Sound.cancel();
                page.lightbox = false;
            } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
                page.stepShot(event.key === Qt.Key_Left ? -1 : 1);
            }
            return;
        }

        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (!page.game)
                return;
            if (page.section === 2) {
                Sound.enter();
                page.lightbox = true;
            } else if (page.section === 0 && page.action === "favourite") {
                page.toggleFavourite();
            } else if (page.section === 0 && page.action === "recordings") {
                page.recordingsRequested(page.game);
            } else if (page.section === 0 && page.action === "journal") {
                page.journalRequested(page.game);
            } else {
                page.launchRequested(page.game);
            }
            return;
        }
        if (api.keys.isFilters(event)) {
            event.accepted = true;
            if (page.game)
                page.toggleFavourite();
            return;
        }
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            page.closeRequested();
            return;
        }
        if (event.key === Qt.Key_Down) {
            event.accepted = true;
            page.stepSound(page.stepDown());
            return;
        }
        if (event.key === Qt.Key_Up) {
            event.accepted = true;
            page.stepSound(page.stepUp());
            return;
        }
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            event.accepted = true;
            var step = event.key === Qt.Key_Left ? -1 : 1;
            if (page.section === 0) {
                var next = Math.max(0, Math.min(page.pills.length - 1, page.actionIndex + step));
                next === page.actionIndex ? Sound.edge() : Sound.tick();
                page.actionIndex = next;
            } else if (page.section === 2) {
                page.stepShot(step);
            } else {
                Sound.edge();
            }
            return;
        }
        if (api.keys.isMenu(event)) {
            event.accepted = true;
            if (page.game)
                page.menuRequested(page.game, heroLogo);
            return;
        }
        event.accepted = true;
    }
}
