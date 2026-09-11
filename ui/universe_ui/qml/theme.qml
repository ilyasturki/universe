import QtQuick
import "core"
import "sound"
import "ui"

FocusScope {
    id: root

    focus: true

    Component.onCompleted: Sound.preload()

    readonly property var tabNames: ["Home", "Library", "Favourites", "Settings"]
    property int tabIndex: 0
    property bool detailOpen: false
    property var detailGame: null
    property bool searchOpen: false
    // A screen over the detail page or the tabs: settings, recordings or journal of one game.
    property bool subOpen: false
    property var subGame: null
    property string subSource: ""
    // "page" | "chrome" | "search": one owner, so no two focus bindings race.
    property string focusOwner: "page"

    // Repeater.itemAt() is not a binding source; the active loader publishes itself.
    property var activePage: null
    // What A, X and a held A act on: the search overlay while it is up.
    readonly property var focusTarget: searchOpen && searchLoader.item ? searchLoader.item : activePage
    readonly property var focusedGame: subOpen ? subGame
                                               : detailOpen ? detailGame
                                               : (activePage ? activePage.currentGame : null)
    readonly property bool menuOpen: contextMenu.open

    Binding {
        target: Theme
        property: "vscale"
        value: root.height / 1080
    }

    function goToTab(index) {
        tabIndex = (index + tabNames.length) % tabNames.length;
    }

    function restoreFocus() {
        if (focusOwner === "search" && searchLoader.item)
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
        if (searchLoader.item)
            searchLoader.item.forceActiveFocus();
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
        // Scroll home first, so the hero rides back down through the fade instead of popping.
        if (detailLoader.item)
            detailLoader.item.reset();
        detailOpen = false;
        // Qt clears the loader's focus from C++ without re-evaluating the binding.
        restoreFocus();
    }

    function launchGame(game) {
        if (!game || launchOverlay.running)
            return;
        if (api.universe.currentSession) {
            Sound.edge();
            toast.show(api.universe.currentSession.title + " is still running");
            return;
        }
        launching = true;
        Sound.enter();
        Sound.launch();
        launchOverlay.begin(game);
    }

    function openSub(source, game) {
        if (!game)
            return;
        Sound.enter();
        subGame = game;
        subSource = source;
        subOpen = true;
        if (subLoader.item)
            subLoader.item.forceActiveFocus();
    }

    function closeSub() {
        Sound.cancel();
        subOpen = false;
        if (detailOpen && detailLoader.item)
            detailLoader.item.forceActiveFocus();
        else
            restoreFocus();
    }

    function stopSession() {
        var session = api.universe.currentSession;
        if (!session)
            return;
        Sound.cancel();
        api.universe.stop(session.session_id);
    }

    function openMenu(game, anchor) {
        if (!game || !anchor)
            return;
        Sound.panel();
        contextMenu.show(game, anchor);
    }

    // A page that keeps a removed favourite in place does the flip itself, but
    // only while it owns the focus: over search the game is not the page's.
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

    // One hero for home and detail: the same art item survives the page switch,
    // so opening details is a rise, not a crossfade.
    readonly property bool heroActive: detailOpen || pageOwnsBackdrop
    readonly property real heroHeight: detailOpen ? Theme.dp(Theme.heroDetail)
                                                  : Theme.dp(Theme.tabBarHeight + Theme.heroBand)
    readonly property real detailScroll: detailLoader.item ? detailLoader.item.scrollY : 0

    property real scrimTop: activePage ? activePage.scrimTop : 0
    property real scrimMid: activePage ? activePage.scrimMid : 0
    property real scrimBottom: activePage ? activePage.scrimBottom : 0

    Behavior on scrimTop { NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic } }
    Behavior on scrimMid { NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic } }
    Behavior on scrimBottom { NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic } }

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

        Behavior on blurRadius {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
        }
        Behavior on opacity {
            NumberAnimation {
                duration: root.launching ? Theme.durLaunch : Theme.durScene
                easing.type: root.launching ? Easing.InOutQuad : Easing.OutCubic
            }
        }
    }

    Item {
        id: heroFrame

        // Scrolling the detail page pulls the hero up at half speed and dims it.
        y: -root.detailScroll * 0.45
        width: parent.width
        height: root.heroHeight
        clip: true
        opacity: root.heroActive && !root.launching ? 1.0 : 0.0
        visible: opacity > 0.01

        // Under the tab bar the art fades right out; on the detail page there is
        // nothing above it, so it only settles.
        property real topA: root.detailOpen ? 0.30 : 1.00
        property real topB: root.detailOpen ? 0.12 : 0.62
        property real topEnd: root.detailOpen ? 0.30 : 0.36
        property real bottomA: root.detailOpen ? 0.80 : 0.55
        property real sideA: root.detailOpen ? 0.72 : 0.92
        property real sideB: root.detailOpen ? 0.36 : 0.66

        Behavior on height { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } }
        Behavior on opacity {
            NumberAnimation {
                duration: root.launching ? Theme.durLaunch : Theme.durScene
                easing.type: root.launching ? Easing.InOutQuad : Easing.OutCubic
            }
        }
        Behavior on topA { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } }
        Behavior on topB { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } }
        Behavior on topEnd { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } }
        Behavior on bottomA { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } }
        Behavior on sideA { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } }
        Behavior on sideB { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } }

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
                GradientStop { position: 0.00; color: Qt.rgba(0.055, 0.059, 0.075, heroFrame.sideA) }
                GradientStop { position: 0.38; color: Qt.rgba(0.055, 0.059, 0.075, heroFrame.sideB) }
                GradientStop { position: 0.74; color: Qt.rgba(0.055, 0.059, 0.075, 0.10) }
                GradientStop { position: 1.00; color: Qt.rgba(0.055, 0.059, 0.075, 0.00) }
            }
        }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.00; color: Qt.rgba(0.055, 0.059, 0.075, heroFrame.topA) }
                GradientStop { position: 0.14; color: Qt.rgba(0.055, 0.059, 0.075, heroFrame.topB) }
                GradientStop { position: heroFrame.topEnd; color: Qt.rgba(0.055, 0.059, 0.075, 0.00) }
                GradientStop { position: 0.80; color: Qt.rgba(0.055, 0.059, 0.075, heroFrame.bottomA) }
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
                               Behavior on y { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } } }
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: root.launching ? Theme.durLaunch : Theme.durScene
                easing.type: root.launching ? Easing.InOutQuad : Easing.OutCubic
            }
        }

        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                GradientStop { position: 0.00; color: Qt.rgba(0.055, 0.059, 0.075, root.scrimTop) }
                GradientStop { position: 0.42; color: Qt.rgba(0.055, 0.059, 0.075, root.scrimMid) }
                GradientStop { position: 1.00; color: Qt.rgba(0.055, 0.059, 0.075, root.scrimBottom) }
            }
        }

        Item {
            anchors.fill: parent
            opacity: root.activePage ? root.activePage.chromeScrim : 0

            Behavior on opacity {
                NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
            }

            Rectangle {
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                height: tabBar.height
                color: Theme.ground
            }

            Rectangle {
                anchors.bottom: parent.bottom
                anchors.left: parent.left
                anchors.right: parent.right
                height: hintBar.height
                color: Theme.ground
            }
        }

        TabBar {
            id: tabBar
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            tabs: root.tabNames
            currentIndex: root.tabIndex
            focus: root.focusOwner === "chrome"

            onTabRequested: function(index) { root.goToTab(index); }
            onSearchRequested: root.openSearch()
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
                model: [ "pages/HomePage.qml", "pages/LibraryPage.qml", "pages/FavouritesPage.qml", "pages/SettingsPage.qml" ]

                Loader {
                    id: pageLoader

                    readonly property bool isActive: index === root.tabIndex
                    property bool activated: false

                    width: pageArea.width
                    height: pageArea.height
                    source: modelData
                    active: isActive || activated
                    focus: isActive && root.focusOwner === "page"
                    opacity: isActive ? 1.0 : 0.0
                    visible: opacity > 0.01
                    x: isActive ? 0 : (index > root.tabIndex ? 1 : -1) * Theme.dp(40)

                    Behavior on opacity {
                        NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
                    }
                    Behavior on x {
                        NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
                    }

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
                        function onTabRequested(index) {
                            Sound.enter();
                            root.goToTab(index);
                        }
                        function onChromeRequested() { root.focusChrome(); }
                    }
                }
            }
        }

        HintBar {
            id: hintBar
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            hints: root.menuOpen ? contextMenu.hints
                 : root.focusOwner === "chrome" ? tabBar.hints
                 : (root.focusTarget ? root.focusTarget.hints : [])
        }
    }

    Loader {
        id: searchLoader

        property bool activated: false

        anchors.fill: parent
        // The sheet rises to the hint bar's top edge, which is not a sibling.
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight)
        active: root.searchOpen || activated
        focus: root.focusOwner === "search"
        source: "ui/SearchOverlay.qml"
        opacity: (root.detailOpen || root.launching) ? 0.0 : 1.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: root.launching ? Theme.durLaunch : Theme.durScene
                easing.type: root.launching ? Easing.InOutQuad : Easing.OutCubic
            }
        }

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
                               Behavior on y { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } } }

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
        }

        Behavior on opacity {
            NumberAnimation {
                duration: root.launching ? Theme.durLaunch : Theme.durScene
                easing.type: root.launching ? Easing.InOutQuad : Easing.OutCubic
            }
        }
    }

    Loader {
        id: subLoader

        anchors.fill: parent
        active: root.subOpen || subLoader.opacity > 0.01
        source: root.subSource
        focus: root.subOpen
        opacity: root.subOpen && !root.launching ? 1.0 : 0.0
        visible: opacity > 0.01
        transform: Translate { y: root.subOpen ? 0 : Theme.dp(48)
                               Behavior on y { NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic } } }

        onLoaded: {
            item.game = Qt.binding(function() { return root.subGame; });
            item.forceActiveFocus();
        }

        Connections {
            target: subLoader.item
            ignoreUnknownSignals: true
            function onCloseRequested() { root.closeSub(); }
        }

        Behavior on opacity {
            NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic }
        }
    }

    ContextMenu {
        id: contextMenu
        anchors.fill: parent
        onPlayRequested: function(game) { root.launchGame(game); }
        onDetailRequested: function(game) { root.openDetail(game); }
        onFavouriteRequested: root.toggleFavourite(game)
        onSettingsRequested: root.openSub("pages/GameSettingsPage.qml", game)
        onRecordingsRequested: root.openSub("pages/RecordingsPage.qml", game)
        onJournalRequested: root.openSub("pages/JournalPage.qml", game)
        onStopRequested: root.stopSession()
        onClosed: {
            if (root.subOpen && subLoader.item)
                subLoader.item.forceActiveFocus();
            else if (root.detailOpen && detailLoader.item)
                detailLoader.item.forceActiveFocus();
            else
                root.restoreFocus();
        }
    }

    LaunchOverlay {
        id: launchOverlay
        anchors.fill: parent
        onFinished: {
            root.launching = false;
            // The overlay's snapshot has let go of the page by now, so a prune is safe.
            if (root.activePage && root.activePage.leave)
                root.activePage.leave();
        }
        onFailed: function(game, message) { toast.show("Could not launch" + (game ? " " + game.title : "") + (message ? ": " + message : "")); }
    }

    Toast {
        id: toast
    }

    Connections {
        target: api.universe
        function onError(kind, message) { toast.show(message); }
        function onSessionEnded(sessionId, id, duration) {
            var game = api.allGames.byId(id);
            if (game)
                toast.show(game.title + " · " + Math.max(1, Math.round(duration / 60)) + " min");
        }
    }

    Keys.onPressed: function(event) {
        if (root.launching || root.subOpen) {
            event.accepted = true;
            return;
        }
        if (event.isAutoRepeat)
            return;

        // Start opens the game menu; with nothing to act on it goes to the Settings tab.
        if (api.keys.isMenu(event)) {
            event.accepted = true;
            if (root.menuOpen)
                return;
            var t = root.focusTarget;
            if (t && t.currentGame && t.menuAnchor)
                openMenu(t.currentGame, t.menuAnchor);
            else {
                Sound.space();
                goToTab(root.tabIndex === tabNames.length - 1 ? 0 : tabNames.length - 1);
            }
            return;
        }
        if (api.keys.isPrevPage(event)) {
            event.accepted = true;
            Sound.space();
            goToTab(root.tabIndex - 1);
            return;
        }
        if (api.keys.isNextPage(event)) {
            event.accepted = true;
            Sound.space();
            goToTab(root.tabIndex + 1);
            return;
        }
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            root.acceptHeld = true;
            holdTimer.restart();
            return;
        }
        if (api.keys.isDetails(event)) {
            event.accepted = true;
            if (root.focusTarget)
                openDetail(root.focusTarget.currentGame);
            return;
        }
        if (api.keys.isCancel(event) && root.tabIndex !== 0) {
            event.accepted = true;
            Sound.cancel();
            goToTab(0);
            return;
        }
    }

    Keys.onReleased: function(event) {
        // Pegasus repeats a held button as release/press pairs flagged auto-repeat.
        if (event.isAutoRepeat || !api.keys.isAccept(event) || !root.acceptHeld)
            return;
        event.accepted = true;
        root.acceptHeld = false;
        holdTimer.stop();
        if (root.launching || root.detailOpen || root.menuOpen || !root.focusTarget)
            return;
        launchGame(root.focusTarget.currentGame);
    }
}
