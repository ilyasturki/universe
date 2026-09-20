import QtQuick
import "core"
import "sound"
import "ui"

FocusScope {
    id: root

    focus: true

    Component.onCompleted: {
        Sound.preload();
        if (api.theme.landing === "themes")
            tabIndex = settingsTab;
    }

    readonly property var tabs: [
        { name: "Home", source: "pages/HomePage.qml" },
        { name: "Library", source: "pages/LibraryPage.qml" },
        { name: "Favourites", source: "pages/FavouritesPage.qml" },
        { name: "Media", source: "pages/MediaPage.qml" },
        { name: "Settings", source: "pages/SettingsPage.qml" }
    ]
    readonly property int settingsTab: 4
    property int tabIndex: 0
    property bool detailOpen: false
    property var detailGame: null
    property bool searchOpen: false
    property bool subOpen: false
    property string subSource: ""
    property var subArgs: ({})
    property var subReturn: null
    property bool subSwapping: false
    // "page" | "chrome" | "search": one owner, so no two focus bindings race.
    property string focusOwner: "page"

    // Repeater.itemAt() is not a binding source; the active loader publishes itself.
    property var activePage: null
    readonly property var focusTarget: searchOpen && searchLoader.item ? searchLoader.item : activePage
    readonly property var focusedGame: subOpen ? (subArgs.game || null)
                                               : detailOpen ? detailGame
                                               : (activePage ? activePage.currentGame : null)
    readonly property bool menuOpen: gameMenu.open
    readonly property var session: api.universe.currentSession
    readonly property bool sessionRunning: session != null && session.session_id !== undefined
    readonly property string playingId: sessionRunning ? session.id : ""
    property var pendingLaunch: null

    Binding {
        target: Theme
        property: "vscale"
        value: root.height / 1080
    }

    Binding {
        target: Theme
        property: "software"
        value: root.GraphicsInfo.api === GraphicsInfo.Software
    }

    function goToTab(index) {
        tabIndex = (index + tabs.length) % tabs.length;
    }

    property string settingsLanding: ""

    function openSettings(section) {
        settingsLanding = section;
        goToTab(settingsTab);
        deliverLanding();
    }

    function deliverLanding() {
        if (settingsLanding === "" || tabIndex !== settingsTab || !activePage || !activePage.land)
            return;
        var section = settingsLanding;
        settingsLanding = "";
        activePage.land(section);
    }

    onActivePageChanged: deliverLanding()

    function openAdd() {
        openSub("pages/AddGamePage.qml", { add: true });
    }

    function restoreFocus() {
        if (subOpen && subLoader.item)
            subLoader.item.forceActiveFocus();
        else if (detailOpen && detailLoader.item)
            detailLoader.item.forceActiveFocus();
        else if (focusOwner === "search" && searchLoader.item)
            searchLoader.item.forceActiveFocus();
        else if (focusOwner === "chrome")
            tabBar.forceActiveFocus();
        else if (activePage)
            activePage.forceActiveFocus();
    }

    function focusChrome() {
        Sound.panel();
        focusOwner = "chrome";
        tabBar.enter();
    }

    function focusPage() {
        focusOwner = "page";
        if (activePage)
            activePage.forceActiveFocus();
    }

    function openSearch() {
        Sound.enter();
        tabBar.index = tabBar.searchIndex;
        searchOpen = true;
        focusOwner = "search";
        restoreFocus();
    }

    function closeSearch() {
        searchOpen = false;
        focusOwner = "chrome";
        tabBar.forceActiveFocus();
    }

    function openDetail(game) {
        if (!game)
            return;
        Sound.enter();
        detailGame = game;
        detailOpen = true;
    }

    function closeDetail() {
        Sound.cancel();
        if (detailLoader.item)
            detailLoader.item.reset();
        detailOpen = false;
        // Qt clears the loader's focus from C++ without re-evaluating the binding.
        restoreFocus();
    }

    function launchGame(game) {
        if (!game || launchOverlay.running || confirm.open)
            return;
        if (sessionRunning) {
            if (game.id === playingId) {
                resumeSession();
                return;
            }
            var running = session.title;
            confirm.ask({ message: "Quit " + running + " and start " + game.title + "?",
                          detail: "Unsaved progress in " + running + " will be lost.", yes: "Quit and start", no: "Keep playing" },
                        function(yes) {
                            if (!yes)
                                return;
                            root.pendingLaunch = game;
                            root.stopSession();
                        });
            return;
        }
        launching = true;
        Sound.enter();
        Sound.launch();
        launchOverlay.begin(game);
    }

    function resumeSession() {
        if (!sessionRunning || flip.growing)
            return;
        Sound.enter();
        if (api.home.frame === "") {
            api.home.toGame();
            return;
        }
        var page = root.activePage;
        var rect = root.tabIndex === 0 && !root.detailOpen && !root.subOpen && !root.searchOpen && page && page.playingTileRect
                 ? page.playingTileRect(flip) : null;
        flip.fromTile(rect, function() { api.home.toGame(); });
    }

    function homePressed() {
        if (root.launching || launchOverlay.running)
            return;
        if (!sessionRunning) {
            if (root.subOpen || confirm.open || root.menuOpen)
                return;
            var t = root.focusTarget;
            if (t && t.currentGame && t.menuAnchor)
                openMenu(t.currentGame, t.menuAnchor);
            return;
        }
        if (api.home.shown !== "game")
            resumeSession();
        else if (api.home.open)
            api.home.closeDock();
        else
            api.home.openDock();
    }

    function openSub(source, args) {
        if (!args.game && !args.runner && !args.module && !args.source && !args.add)
            return;
        Sound.enter();
        subArgs = args;
        subSource = source;
        subReturn = null;
        subOpen = true;
        restoreFocus();
    }

    function jumpSub(source, session) {
        if (!subOpen || subSwapping || source === subSource)
            return;
        Sound.enter();
        subReturn = subReturn && subReturn.source === source ? null
                  : { source: subSource, session: subLoader.item && subLoader.item.currentSession ? subLoader.item.currentSession : "" };
        subSwapping = true;
        subSwap.target = { source: source, session: session };
        subSwap.restart();
    }

    readonly property Timer subSwap: Timer {
        property var target: null
        interval: Theme.durQuick
        onTriggered: {
            root.subArgs = target.args !== undefined ? target.args : { game: root.subArgs.game, session: target.session };
            root.subSource = target.source;
            root.subSwapping = false;
            root.restoreFocus();
        }
    }

    function pushSub(source, args) {
        if (!subOpen || subSwapping)
            return;
        Sound.enter();
        subReturn = { source: subSource, args: subArgs };
        subSwapping = true;
        subSwap.target = { source: source, args: args };
        subSwap.restart();
    }

    function closeSub() {
        Sound.cancel();
        if (subReturn) {
            var back = subReturn;
            subReturn = null;
            subSwapping = true;
            subSwap.target = back;
            subSwap.restart();
            return;
        }
        subOpen = false;
        restoreFocus();
    }

    function stopSession() {
        if (!sessionRunning)
            return;
        Sound.cancel();
        api.home.stop();
    }

    function mediaItems(game) {
        var out = [];
        var shots = (api.universe.screenshots(game.id) || []).length;
        var recordings = (api.universe.recordings(game.id) || []).length;
        var entries = (api.universe.journal(game.id) || []).length;
        if (shots > 0)
            out.push({ icon: "camera", label: "Screenshots", action: "screenshots", detail: String(shots) });
        if (recordings > 0)
            out.push({ icon: "film", label: "Recordings", action: "recordings", detail: String(recordings) });
        if (entries > 0)
            out.push({ icon: "book", label: "Journal", action: "journal", detail: String(entries) });
        return out;
    }

    function openMenu(game, anchor) {
        if (!game || !anchor)
            return;
        Sound.panel();
        var media = mediaItems(game);
        var play = sessionRunning && game.id === playingId
            ? [ { icon: "play", label: "Resume", action: "resume" },
                { icon: "stop", label: "Quit " + session.title, action: "stop" } ]
            : [ { icon: "play", label: game.playTime > 0 ? "Continue" : "Play", action: "play" } ];
        var look = [];
        if (!(detailOpen && detailGame === game))
            look.push({ icon: "info", label: "Details", action: "details" });
        look.push({ icon: game.favorite ? "heart" : "heart-outline",
                    label: game.favorite ? "Remove from favourites" : "Add to favourites", action: "favourite" });
        if (media.length > 0)
            look.push({ icon: "photos", label: "Media", action: "media", more: true });
        var manage = [ { icon: "sliders", label: "Game settings", action: "settings" },
                       { icon: "image", label: "Artwork", action: "artwork" },
                       { icon: "eye-off", label: "Remove from library…", action: "remove", danger: true } ];
        var items = [];
        [ play, look, manage ].forEach(function(group) {
            group[0].gap = items.length > 0;
            items = items.concat(group);
        });
        var pages = { settings: "FormPage", artwork: "ArtworkPage", screenshots: "ScreenshotsPage", recordings: "RecordingsPage", journal: "JournalPage" };
        gameMenu.show(items, anchor, Qt.rect(0, 0, anchor.width, anchor.height), "", function(action) {
            if (action === "media") {
                Sound.enter();
                gameMenu.push(media, "Media", function(page) {
                    root.restoreFocus();
                    root.openSub("pages/" + pages[page] + ".qml", { game: game });
                });
                return;
            }
            if (action === "remove") {
                Sound.panel();
                gameMenu.push([ { icon: "", label: "Keep it", action: "" },
                                { icon: "eye-off", label: "Remove from the library", action: "yes", danger: true } ],
                              "Remove " + game.title + "?", function(answer) {
                    if (answer === "yes")
                        root.removeGame(game);
                    root.restoreFocus();
                });
                return;
            }
            root.restoreFocus();
            if (action === "play")
                root.launchGame(game);
            else if (action === "details")
                root.openDetail(game);
            else if (action === "favourite")
                root.toggleFavourite(game);
            else if (action === "stop")
                root.stopSession();
            else if (action === "resume")
                root.resumeSession();
            else
                root.openSub("pages/" + pages[action] + ".qml", { game: game });
        });
    }

    // The install folder, the hours and the journal stay on disk; the watcher drops the game from the library.
    function removeGame(game) {
        var title = game.title;
        if (detailOpen && detailGame === game)
            closeDetail();
        Sound.enter();
        if (api.universe.remove(game.id, false))
            toast.show("Removed " + title + " from the library");
    }

    function toggleFavourite(game) {
        if (!searchOpen && activePage && activePage.toggleFavourite) {
            activePage.toggleFavourite();
            return;
        }
        game.favorite = !game.favorite;
        Sound.favourite(game.favorite);
    }

    // A launches on release; a hold this long opens the game menu instead.
    property bool acceptHeld: false
    readonly property Timer holdTimer: Timer {
        interval: 450
        onTriggered: {
            var p = root.focusTarget;
            if (!p || !p.menuAnchor)
                return;
            root.acceptHeld = false;
            root.openMenu(p.currentGame, p.menuAnchor);
        }
    }

    property bool launching: false
    readonly property bool pageOwnsBackdrop: activePage ? activePage.ownsBackdrop : false

    readonly property bool heroActive: detailOpen || pageOwnsBackdrop
    readonly property real heroHeight: detailOpen ? Theme.dp(Theme.heroDetail)
                                                  : Theme.dp(Theme.tabBarHeight + Theme.heroBand)
    readonly property real detailScroll: detailLoader.item ? detailLoader.item.scrollY : 0

    property real scrimTop: activePage ? activePage.scrimTop : 0
    property real scrimMid: activePage ? activePage.scrimMid : 0
    property real scrimBottom: activePage ? activePage.scrimBottom : 0

    Behavior on scrimTop { Ease { duration: Theme.durView } }
    Behavior on scrimMid { Ease { duration: Theme.durView } }
    Behavior on scrimBottom { Ease { duration: Theme.durView } }

    component SceneFade: NumberAnimation {
        duration: root.launching ? Theme.durLaunch : Theme.durScene
        easing.type: root.launching ? Easing.InOutQuad : Easing.OutCubic
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    BackgroundStage {
        anchors.fill: parent
        game: root.focusedGame
        blurRadius: art.cropped ? 56 : (root.activePage ? root.activePage.backdropBlur : 18)
        opacity: (root.launching || root.detailOpen || root.pageOwnsBackdrop) ? 0.0 : 1.0
        visible: opacity > 0.01

        Behavior on blurRadius { Ease { duration: Theme.durView } }
        Behavior on opacity { SceneFade {} }
    }

    Item {
        id: heroFrame

        y: -root.detailScroll * 0.45
        width: parent.width
        height: root.heroHeight
        clip: true
        opacity: root.heroActive && !root.launching ? 1.0 : 0.0
        visible: opacity > 0.01

        property real detail: root.detailOpen ? 1.0 : 0.0

        Behavior on height { Ease { duration: Theme.durScene } }
        Behavior on opacity { SceneFade {} }
        Behavior on detail { Ease { duration: Theme.durScene } }

        BackgroundStage {
            anchors.fill: parent
            game: root.focusedGame
            blurRadius: art.cropped ? 56 : 0
            zoomEnabled: false
            overscan: 1.06
        }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0.00; color: Qt.rgba(0.055, 0.059, 0.075, 0.92 - 0.20 * heroFrame.detail) }
                GradientStop { position: 0.38; color: Qt.rgba(0.055, 0.059, 0.075, 0.66 - 0.30 * heroFrame.detail) }
                GradientStop { position: 0.74; color: Qt.rgba(0.055, 0.059, 0.075, 0.10) }
                GradientStop { position: 1.00; color: Qt.rgba(0.055, 0.059, 0.075, 0.00) }
            }
        }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.00; color: Qt.rgba(0.055, 0.059, 0.075, 1.00 - 0.70 * heroFrame.detail) }
                GradientStop { position: 0.14; color: Qt.rgba(0.055, 0.059, 0.075, 0.62 - 0.50 * heroFrame.detail) }
                GradientStop { position: 0.36 - 0.06 * heroFrame.detail; color: Qt.rgba(0.055, 0.059, 0.075, 0.00) }
                GradientStop { position: 0.80; color: Qt.rgba(0.055, 0.059, 0.075, 0.55 + 0.25 * heroFrame.detail) }
                GradientStop { position: 1.00; color: Theme.ground }
            }
        }

        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0.055, 0.059, 0.075, Math.min(0.88, root.detailScroll / Theme.dp(520)))
        }
    }

    Item {
        anchors.fill: parent
        opacity: (root.launching || root.detailOpen) ? 0.0 : 1.0
        transform: Translate { y: root.detailOpen ? -Theme.dp(36) : 0
                               Behavior on y { Ease { duration: Theme.durScene } } }
        visible: opacity > 0.01

        Behavior on opacity { SceneFade {} }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.00; color: Qt.rgba(0.055, 0.059, 0.075, root.scrimTop) }
                GradientStop { position: 0.42; color: Qt.rgba(0.055, 0.059, 0.075, root.scrimMid) }
                GradientStop { position: 1.00; color: Qt.rgba(0.055, 0.059, 0.075, root.scrimBottom) }
            }
        }

        TabBar {
            id: tabBar
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            tabs: root.tabs.map(function(t) { return t.name; })
            currentIndex: root.tabIndex
            showSearch: root.tabs[root.tabIndex].name !== "Settings"
            focus: root.focusOwner === "chrome"

            onTabRequested: function(index) { root.goToTab(index); }
            onSearchRequested: root.openSearch()
            onResumeRequested: root.resumeSession()
            onMenuRequested: function(game, anchor) { root.openMenu(game, anchor); }
            onEntered: root.focusPage()
            onDismissed: root.focusPage()
        }

        Item {
            id: pageArea

            anchors.top: tabBar.bottom
            anchors.bottom: hintBar.top
            anchors.left: parent.left
            anchors.right: parent.right

            Repeater {
                model: root.tabs

                Loader {
                    id: pageLoader

                    readonly property bool isActive: index === root.tabIndex
                    property bool activated: false

                    width: pageArea.width
                    height: pageArea.height
                    source: modelData.source
                    active: isActive || activated
                    focus: isActive && root.focusOwner === "page"
                    opacity: isActive ? 1.0 : 0.0
                    visible: opacity > 0.01
                    x: isActive ? 0 : (index > root.tabIndex ? 1 : -1) * Theme.dp(40)

                    Behavior on opacity { Ease { duration: Theme.durView } }
                    Behavior on x { Ease { duration: Theme.durView } }

                    function publish() {
                        if (isActive && item)
                            root.activePage = item;
                    }
                    onIsActiveChanged: {
                        publish();
                        if (!isActive && item && item.leave)
                            item.leave();
                    }
                    onLoaded: {
                        activated = true;
                        publish();
                        if ("menuOpen" in item)
                            item.menuOpen = Qt.binding(function() { return root.menuOpen; });
                    }

                    Connections {
                        target: pageLoader.item
                        ignoreUnknownSignals: true
                        function onDetailRequested(game) { root.openDetail(game); }
                        function onSettingsRequested(game) { root.openSub("pages/FormPage.qml", { game: game }); }
                        function onRunnerRequested(runner) { root.openSub("pages/FormPage.qml", { runner: runner }); }
                        function onModuleRequested(module) { root.openSub("pages/FormPage.qml", { module: module }); }
                        function onSourceRequested(source) { root.openSub("pages/FormPage.qml", { source: source }); }
                        function onFormRequested(args) { root.openSub("pages/FormPage.qml", args); }
                        function onArtworkRequested(game, slot) { root.openSub("pages/ArtworkPage.qml", { game: game, slot: slot }); }
                        function onScreenshotsRequested(game, name) { root.openSub("pages/ScreenshotsPage.qml", { game: game, name: name || "" }); }
                        function onRecordingsRequested(game, session) { root.openSub("pages/RecordingsPage.qml", { game: game, session: session || "" }); }
                        function onJournalRequested(game, session) { root.openSub("pages/JournalPage.qml", { game: game, session: session || "" }); }
                        function onTabRequested(index) {
                            Sound.enter();
                            root.goToTab(index);
                        }
                        function onChromeRequested() { root.focusChrome(); }
                        function onAddRequested() { root.openAdd(); }
                        function onMessage(text) { toast.show(text); }
                    }
                }
            }
        }

        HintBar {
            id: hintBar
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            hints: confirm.open ? confirm.hints
                 : root.menuOpen ? gameMenu.hints
                 : root.focusOwner === "chrome" ? tabBar.hints
                 : root.focusOwner === "search" ? (searchLoader.item ? searchLoader.item.hints : [])
                 : !root.activePage ? []
                 : root.activePage.modal ? root.activePage.hints
                 : root.activePage.hints.concat([ { glyph: "LB RB", label: "Tabs" } ])
        }
    }

    Loader {
        id: searchLoader

        property bool activated: false

        anchors.fill: parent
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight)
        active: root.searchOpen || activated
        focus: root.focusOwner === "search"
        source: "ui/SearchOverlay.qml"
        opacity: (root.detailOpen || root.launching) ? 0.0 : 1.0
        visible: opacity > 0.01

        Behavior on opacity { SceneFade {} }

        onLoaded: {
            activated = true;
            item.open = Qt.binding(function() { return root.searchOpen; });
            if (root.focusOwner === "search")
                item.forceActiveFocus();
        }

        Connections {
            target: searchLoader.item
            ignoreUnknownSignals: true
            function onCloseRequested() { root.closeSearch(); }
        }
    }

    Loader {
        id: detailLoader

        anchors.fill: parent
        active: root.detailOpen || detailLoader.opacity > 0.01
        source: "pages/DetailPage.qml"
        focus: root.detailOpen && !root.subOpen
        opacity: root.detailOpen && !root.launching && !root.subOpen ? 1.0 : 0.0
        transform: Translate { y: root.detailOpen ? 0 : Theme.dp(48)
                               Behavior on y { Ease { duration: Theme.durScene } } }

        onLoaded: {
            item.game = Qt.binding(function() { return root.detailGame; });
            item.forceActiveFocus();
        }

        Connections {
            target: detailLoader.item
            ignoreUnknownSignals: true
            function onLaunchRequested(game) { root.launchGame(game); }
            function onCloseRequested() { root.closeDetail(); }
            function onMenuRequested(game, anchor) { root.openMenu(game, anchor); }
            function onRecordingsRequested(game) { root.openSub("pages/RecordingsPage.qml", { game: game }); }
            function onJournalRequested(game) { root.openSub("pages/JournalPage.qml", { game: game }); }
            function onScreenshotsRequested(game) { root.openSub("pages/ScreenshotsPage.qml", { game: game }); }
        }

        Behavior on opacity { SceneFade {} }
    }

    // A sub-page jump fades over this, not over the tabs.
    Rectangle {
        anchors.fill: parent
        color: Theme.ground
        opacity: root.subOpen && !root.launching ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity { Ease { duration: Theme.durScene } }
    }

    Loader {
        id: subLoader

        anchors.fill: parent
        active: root.subOpen || subLoader.opacity > 0.01
        source: root.subSource
        focus: root.subOpen
        opacity: root.subOpen && !root.launching && !root.subSwapping ? 1.0 : 0.0
        visible: opacity > 0.01
        transform: Translate { y: root.subOpen ? 0 : Theme.dp(48)
                               Behavior on y { Ease { duration: Theme.durScene } } }

        onLoaded: {
            item.args = Qt.binding(function() { return root.subArgs; });
            item.forceActiveFocus();
        }

        Connections {
            target: subLoader.item
            ignoreUnknownSignals: true
            function onCloseRequested() { root.closeSub(); }
            function onJumpRequested(source, session) { root.jumpSub(source, session); }
            function onInstallRequested(source, section) {
                root.closeSub();
                root.openSettings(section);
            }
            function onSettingsRequested(game) { root.pushSub("pages/FormPage.qml", { game: game }); }
            function onMessage(text) { toast.show(text); }
        }

        Behavior on opacity { Ease { duration: root.subSwapping ? Theme.durQuick : Theme.durScene } }
    }

    ActionMenu {
        id: gameMenu
        objectName: "gameMenu"
        anchors.fill: parent
        // Covers the ring and halo, and the 5% a grid cover grows by.
        copyMargin: Theme.dp(26)
        gap: Theme.dp(44)
        onDismissed: root.restoreFocus()
    }

    ConfirmDialog {
        id: confirm
        objectName: "confirm"
        anchors.fill: parent
        onClosed: root.restoreFocus()
    }

    LaunchOverlay {
        id: launchOverlay
        objectName: "launchOverlay"
        anchors.fill: parent
        onFinished: {
            root.launching = false;
            if (root.activePage && root.activePage.leave)
                root.activePage.leave();
            root.restoreFocus();
        }
        onFailed: function(game, message) { toast.show("Could not launch" + (game ? " " + game.title : "") + (message ? ": " + message : "")); }
    }

    HomeFlip {
        id: flip
        objectName: "homeFlip"
        anchors.fill: parent
    }

    Toast {
        id: toast
    }

    Connections {
        target: api.universe
        function onError(kind, message) {
            toast.show(message);
            root.pendingLaunch = null;
        }
        function onNotice(message) { toast.show(message); }
        function onSessionEnded(sessionId, id, duration) {
            var game = api.allGames.byId(id);
            if (game)
                toast.show(game.title + " · " + Math.max(1, Math.round(duration / 60)) + " min");
            var next = root.pendingLaunch;
            root.pendingLaunch = null;
            if (next)
                root.launchGame(next);
        }
    }

    Connections {
        target: api.screens.controller
        function onMacroNotice(text) { toast.show(text); }
    }

    Connections {
        target: api.home
        function onPressed() { root.homePressed(); }
        // The unit gets a SIGTERM, a second one after ~3 s: the toast covers the wait.
        function onStopping(title) { toast.show("Quitting " + title + "…"); }
        function onChanged() {
            var shown = api.home.shown;
            if (shown === "launcher" && root.lastShown === "game")
                root.landHome();
            root.lastShown = shown;
        }
    }

    property string lastShown: api.home.shown

    // A game that exits by itself leaves the launcher on screen too: no flip, nothing to zoom.
    function landHome() {
        var landing = api.home.takeLanding();
        if (root.launching || !api.home.flipped || api.home.frame === "") {
            api.home.covered();
            if (!root.launching && api.home.flipped && landing !== "") {
                clearToHome();
                openLanding(landing);
            }
            return;
        }
        clearToHome();
        var page = root.activePage;
        var tile = page && page.landOnPlaying ? page.landOnPlaying(flip) : null;
        // Under a page the frame fades rather than shrinking into a tile nobody sees.
        flip.cover(landing === "" ? tile : null);
        openLanding(landing);
    }

    function clearToHome() {
        subReturn = null;
        subOpen = false;
        if (detailLoader.item)
            detailLoader.item.reset();
        detailOpen = false;
        searchOpen = false;
        goToTab(0);
        focusPage();
    }

    function openLanding(landing) {
        var game = landing !== "" ? api.allGames.byId(playingId) : null;
        if (!game)
            return;
        if (landing === "details")
            openDetail(game);
        else
            openSub("pages/" + { journal: "JournalPage", recordings: "RecordingsPage", screenshots: "ScreenshotsPage" }[landing] + ".qml", { game: game });
    }

    // B held: the way out of the launcher from anywhere, the same question Settings › Quit asks.
    Connections {
        target: api.keys
        function onCancelHeld() {
            if (root.launching || launchOverlay.running || confirm.open || root.searchOpen || root.menuOpen || (root.activePage && root.activePage.modal))
                return;
            confirm.ask({ message: "Quit Universe?", detail: api.universe.currentSession ? "The running game is closed with it." : "", yes: "Quit", no: "Stay" },
                        function(yes) { if (yes) Qt.quit(); });
        }
    }

    Connections {
        target: api.screens.pendingJournals
        function onAppeared(session, title) { toast.show("Journal: writing " + title + "…"); }
        function onResolved(session, id, state, text) { toast.show(state === "failed" ? "Journal failed: " + text : "Journal: " + text); }
    }

    Keys.onPressed: function(event) {
        if (root.launching || root.subOpen || launchOverlay.running || confirm.open) {
            event.accepted = true;
            return;
        }
        if (event.isAutoRepeat)
            return;
        event.accepted = true;
        if (api.keys.isMenu(event)) {
            if (root.menuOpen)
                return;
            var t = root.focusTarget;
            if (t && t.currentGame && t.menuAnchor)
                openMenu(t.currentGame, t.menuAnchor);
            else
                Sound.edge();
        } else if (api.keys.isPrevPage(event)) {
            Sound.space();
            goToTab(root.tabIndex - 1);
        } else if (api.keys.isNextPage(event)) {
            Sound.space();
            goToTab(root.tabIndex + 1);
        } else if (api.keys.isAccept(event)) {
            root.acceptHeld = true;
            holdTimer.restart();
        } else if (api.keys.isDetails(event)) {
            if (root.focusTarget)
                openDetail(root.focusTarget.currentGame);
        } else if (api.keys.isCancel(event) && root.tabIndex !== 0) {
            Sound.cancel();
            goToTab(0);
        } else {
            event.accepted = false;
        }
    }

    Keys.onReleased: function(event) {
        // Pegasus repeats a held button as release/press pairs flagged auto-repeat.
        if (event.isAutoRepeat || !api.keys.isAccept(event) || !root.acceptHeld)
            return;
        event.accepted = true;
        root.acceptHeld = false;
        holdTimer.stop();
        if (root.launching || launchOverlay.running || confirm.open || root.detailOpen || root.menuOpen || !root.focusTarget)
            return;
        launchGame(root.focusTarget.currentGame);
    }
}
