import QtQuick
import "../core" as Base
import "core"
import "sound"
import "ui"
import "pages"
import "pages/Sections.js" as Sections

FocusScope {
    id: root

    focus: true

    Component.onCompleted: {
        Sound.preload();
        // The search from home finds the sections before Settings has ever been opened.
        api.screens.search.sections = Sections.forSearch(api.system);
        if (api.theme.takeLanding() === "themes")
            push("pages/SettingsPage.qml", {
                section: "themes"
            });
        else if (api.screens.onboarding.needed)
            push("pages/OnboardingPage.qml", {});
    }

    Binding {
        target: Theme
        property: "vscale"
        value: root.height / 1080
    }

    Binding {
        target: Base.Theme
        property: "software"
        value: root.GraphicsInfo.api === GraphicsInfo.Software
    }

    Binding {
        target: Base.Theme
        property: "covered"
        value: api.home.underGame || !api.focus.active
    }

    // args travel as JSON: a library reload can delete a Game under a page, so pages carry ids.
    readonly property ListModel stack: ListModel {}
    readonly property int depth: stack.count
    readonly property bool onHome: depth === 0
    property bool launching: false
    readonly property bool modal: dialog.open || sheet.open || popup.open || folder.open || launching
    readonly property var session: api.universe.currentSession
    readonly property bool sessionRunning: session !== null && session !== undefined && session.session_id !== undefined
    readonly property string playingId: sessionRunning && session.id !== undefined ? session.id : ""
    property var pendingLaunch: null

    readonly property Item topPage: pages.count > 0 && pages.itemAt(pages.count - 1) ? pages.itemAt(pages.count - 1).item : null
    readonly property bool topOverlay: topPage !== null && topPage.overlay === true
    readonly property var hints: dialog.open ? dialog.hints : sheet.open ? sheet.hints : popup.open ? popup.hints : folder.open ? folder.hints : launching ? [] : !onHome && topPage ? topPage.hints : home.hints
    // The console shows no button hints: the strip comes up only over what it never had.
    readonly property bool stripShown: !launching && !dialog.open && !popup.open && (sheet.open || folder.open || (!onHome && topPage !== null && topPage.strip === true))

    function push(source, args) {
        stack.append({
            source: source,
            argsJson: JSON.stringify(args || {})
        });
        Qt.callLater(focusTop);
    }

    function pop() {
        if (depth === 0)
            return;
        stack.remove(depth - 1);
        Qt.callLater(focusTop);
    }

    function goHome() {
        if (depth === 0)
            return;
        Sound.play("home");
        stack.clear();
        home.reset();
        Qt.callLater(focusTop);
    }

    // A game picked on a page (the library, a search): home, its tile focused, its hero open, as the console's.
    function openGame(id) {
        if (!api.allGames.byId(id)) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        stack.clear();
        home.focusGame(id);
        home.down();
        Qt.callLater(focusTop);
    }

    function focusTop() {
        var target = onHome ? home : topPage;
        if (!modal && target)
            target.forceActiveFocus();
    }

    // B held and Settings › About › Power: the console's Power panel, with Universe's own way out first.
    function askPower() {
        var session = api.universe.currentSession;
        var can = api.system.actions;
        var playing = session && session.title ? session.title : "";
        var items = [
            {
                label: "Enter Rest Mode",
                glyph: "power",
                act: "suspend",
                detail: playing !== "" ? "Suspend the machine; " + playing + " waits where it is." : "Suspend the machine; everything waits where it is."
            },
            {
                label: "Turn Off",
                glyph: "ring",
                act: "power_off"
            },
            {
                label: "Restart",
                glyph: "restart",
                act: "reboot"
            }
        ].filter(function (i) {
            return can.indexOf(i.act) >= 0;
        }).concat([
            {
                label: "Quit Universe",
                glyph: "exit",
                act: "quit",
                detail: api.system.steam ? "Back to Steam, whose menu has the power options." : "",
                gap: can.length > 0
            }
        ]);
        showMenu({
            title: "Power",
            items: items,
            width: Theme.dp(650)
        }, function (i) {
            var act = i >= 0 ? items[i].act : "";
            if (act === "quit")
                Qt.quit();
            else if (act === "suspend") {
                Base.Notices.show("Entering rest mode…", "power");
                api.system.run("suspend");
            } else if (act === "reboot" || act === "power_off")
                confirmPower(act, session);
            else
                api.keys.dropHold();
        });
    }

    function confirmPower(act, session) {
        var restart = act === "reboot";
        dialogAsk({
            message: restart ? "Restart the machine?" : "Turn off the machine?",
            detail: session ? session.title + " will be closed. Unsaved progress will be lost." : "",
            buttons: ["Cancel", restart ? "Restart" : "Turn Off"],
            danger: 1,
            index: 0
        }, function (i) {
            if (i !== 1)
                return;
            Base.Notices.show(restart ? "Restarting…" : "Turning off…", "power");
            api.system.run(act);
        });
    }

    function after(done) {
        return function (v) {
            if (done)
                done(v);
            focusTop();
        };
    }

    function dialogAsk(spec, done) {
        dialog.show(spec, after(done));
    }
    function prompt(spec, done) {
        sheet.show(spec, after(done));
    }
    // `done(first, second)`, or `done(null)` when cancelled.
    function promptPair(spec, done) {
        sheet.showPair(spec, after(function (v) {
            done(v === null ? null : v[0], v === null ? "" : v[1]);
        }));
    }
    // { title, choices, icons, index, at }: `done(i)`, -1 when cancelled; the value in force is ticked.
    function pick(spec, done) {
        var icons = spec.icons || [];
        var current = spec.index !== undefined ? spec.index : -1;
        showMenu({
            title: spec.title,
            items: (spec.choices || []).map(function (c, i) {
                return {
                    label: c,
                    check: i === current,
                    icon: icons[i] || ""
                };
            }),
            index: Math.max(0, current),
            at: spec.at
        }, done);
    }
    function showMenu(spec, done) {
        popup.show(spec, after(done));
    }
    function browse(spec, done) {
        folder.show(spec, after(done));
    }

    function menu(title, items, done) {
        showMenu({
            title: title,
            items: items.map(function (i) {
                return {
                    label: i.label,
                    glyph: i.glyph || "",
                    detail: i.detail || "",
                    danger: i.danger === true,
                    gap: i.gap === true
                };
            })
        }, function (i) {
            if (i >= 0)
                done(items[i].act);
        });
    }

    function componentOptions(ident, title) {
        var items = api.screens.components.actions(ident);
        if (items.length === 0) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        componentMenu(ident, items, title);
    }
    function componentMenu(ident, items, title) {
        menu(title, items.map(function (i) {
            return {
                label: i.label,
                glyph: i.icon || "",
                danger: i.danger === true,
                act: i.action
            };
        }), function (action) {
            root.componentAction(ident, action);
        });
    }
    function componentAction(ident, action) {
        var components = api.screens.components;
        if (action === "versions") {
            componentMenu(ident, components.versionActions(ident), "Another version");
            return;
        }
        var ask = components.confirm(ident, action);
        if (!ask) {
            Sound.play(components.act(ident, action) ? "ok" : "edge");
            return;
        }
        dialogAsk({
            message: ask.message,
            detail: ask.detail,
            buttons: [ask.no, ask.yes],
            danger: action === "rollback" || action === "uninstall" || action.indexOf("remove:") === 0 ? 1 : -1
        }, function (i) {
            if (i === 1)
                Sound.play(components.act(ident, action) ? "ok" : "edge");
        });
    }

    function launch(game) {
        if (!game || splash.running)
            return;
        if (sessionRunning) {
            if (game.id === session.id) {
                resume();
                return;
            }
            var running = session.title;
            dialogAsk({
                message: "Close " + running + " and start " + game.title + "?",
                detail: "Unsaved progress in " + running + " will be lost.",
                buttons: ["Cancel", "Close and start"],
                danger: 1
            }, function (i) {
                if (i !== 1)
                    return;
                root.pendingLaunch = game;
                root.stopSession();
            });
            return;
        }
        Sound.play("launch");
        launching = true;
        splash.begin(game);
    }

    function resume() {
        if (!sessionRunning) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        api.home.toGame();
    }

    function closeSoftware(game) {
        if (!sessionRunning) {
            Sound.play("edge");
            return;
        }
        dialogAsk({
            message: "Close " + session.title + "?",
            detail: "Unsaved progress will be lost.",
            buttons: ["Cancel", "Close Game"],
            danger: 1
        }, function (i) {
            if (i === 1)
                root.stopSession();
        });
    }

    // The unit gets a SIGTERM, a second one after ~3 s: `stopping` puts up the notice that covers the wait.
    function stopSession() {
        if (sessionRunning)
            api.home.stop();
    }

    function showToast(text) {
        Base.Notices.show(text);
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    Item {
        id: homeLayer

        anchors.fill: parent
        opacity: (root.onHome || root.depth === 1 && root.topOverlay) && !root.launching ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
                easing.type: Easing.OutCubic
            }
        }

        HomePage {
            id: home
            objectName: "homePage"
            anchors.fill: parent
            shell: root
            focus: root.onHome
        }
    }

    Repeater {
        id: pages
        model: root.stack

        Loader {
            id: pageLoader

            readonly property bool isTop: index === root.depth - 1
            readonly property bool shown: isTop || index === root.depth - 2 && root.topOverlay

            property bool entered: false

            anchors.fill: parent
            source: model.source
            opacity: 0.0
            scale: !entered ? 1.02 : shown ? 1.0 : 0.985
            visible: opacity > 0.01
            focus: isTop

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.durPage
                    easing.type: Easing.OutCubic
                }
            }
            Behavior on scale {
                NumberAnimation {
                    duration: Theme.durPage
                    easing.type: Easing.OutCubic
                }
            }

            Rectangle {
                anchors.fill: parent
                z: -1
                color: Theme.ground
                visible: !(pageLoader.item && pageLoader.item.overlay === true)
            }

            // Bound after creation, so a page settles in instead of appearing at full size and opacity.
            Component.onCompleted: {
                entered = true;
                opacity = Qt.binding(function () {
                    return shown && !root.launching ? 1.0 : 0.0;
                });
            }

            onLoaded: {
                item.shell = root;
                if ("args" in item)
                    item.args = JSON.parse(model.argsJson);
                if (isTop && !root.modal)
                    item.forceActiveFocus();
            }

            Connections {
                target: pageLoader.item
                ignoreUnknownSignals: true
                function onCloseRequested() {
                    Sound.play("back");
                    root.pop();
                }
            }
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        z: 11.5
        hints: root.hints
        opacity: root.stripShown && root.hints.length > 0 ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
            }
        }
    }

    TextSheet {
        id: sheet
        anchors.fill: parent
        anchors.bottomMargin: hintBar.height
        z: 10
    }

    Menu {
        id: popup
        objectName: "popup"
        z: 11
    }

    FolderSheet {
        id: folder
        objectName: "folder"
        shell: root
        anchors.bottomMargin: hintBar.height
        z: 11
    }

    Dialog {
        id: dialog
        objectName: "dialog"
        z: 11
    }

    LaunchSplash {
        id: splash
        anchors.fill: parent
        z: 12
        onFinished: {
            root.launching = false;
            root.focusTop();
        }
        onFailed: function (game, message) {
            Base.Notices.fail("Could not start" + (game ? " " + game.title : "") + (message ? ": " + message : ""));
        }
    }

    Toast {
        z: 13
    }

    Connections {
        target: api.universe
        function onError(kind, message) {
            Base.Notices.fail(message);
            root.pendingLaunch = null;
        }
        function onNotice(message) {
            Base.Notices.show(message);
        }
        function onSessionEnded(sessionId, id, duration, end) {
            var game = api.allGames.byId(id);
            var minutes = Math.max(1, Math.round(duration / 60)) + " min";
            if (game && (end === "crashed" || end === "killed"))
                Base.Notices.fail(game.title + (end === "crashed" ? " crashed after " : " was killed after ") + minutes + ". See its play log.", "stop");
            else if (game)
                Base.Notices.show(game.title + " · " + minutes, "stop");
            if (root.pendingLaunch)
                root.launch(root.pendingLaunch);
            root.pendingLaunch = null;
        }
    }

    Connections {
        target: api.screens.components
        function onInstallProposed(gameId, component, name, version) {
            if (dialog.open)
                return;
            var ask = api.screens.components.question(component);
            var game = api.allGames.byId(gameId);
            dialogAsk({
                message: gameId ? "Install " + name + " " + version + " to play" + (game ? " " + game.title : "") + "?" : ask.message,
                detail: ask ? ask.detail : "",
                buttons: ["Not Now", gameId ? "Install and Play" : "Install"]
            }, function (i) {
                if (i === 1)
                    api.screens.components.installFor(gameId, component);
            });
        }
        function onReadyToLaunch(gameId) {
            var game = api.allGames.byId(gameId);
            if (game)
                root.launch(game);
        }
        function onMessage(text) {
            Base.Notices.show(text);
        }
    }

    Connections {
        target: api.screens.controller
        function onMacroNotice(text) {
            Base.Notices.show(text);
        }
        // A pad of a family never set up: the walk through its buttons, offered once.
        function onWalkOffered(family, name) {
            if (root.modal || root.launching)
                return;
            dialogAsk({
                message: "Set up the buttons of " + name + "?",
                detail: "Universe already knows this controller. Pressing each button in turn makes sure every one is where it should be. You can also do it later, from Settings › Controllers.",
                buttons: ["Not Now", "Set Up"]
            }, function (i) {
                if (i !== 1) {
                    api.screens.controller.declineWalk(family);
                    return;
                }
                if (root.topPage && root.topPage.startWalk)
                    root.topPage.startWalk();
                else
                    root.push("pages/ControllersPage.qml", {
                        walk: true
                    });
            });
        }
    }

    Connections {
        target: api.screens.pendingJournals
        function onAppeared(session, title) {
            Base.Notices.show("Journal: writing " + title + "…", "journal:" + session);
        }
        function onResolved(session, id, state, text) {
            if (state === "failed")
                Base.Notices.fail("Journal failed: " + text, "journal:" + session);
            else
                Base.Notices.show((state === "deferred" ? "Journal put off: " : "Journal: ") + text, "journal:" + session);
        }
    }

    // B held: the way out from anywhere, the Power panel.
    Connections {
        target: api.keys
        function onCancelHeld() {
            if (!root.modal)
                askPower();
        }
    }

    Connections {
        target: api.system
        function onFailed(action, message) {
            Base.Notices.fail("Could not " + ({
                    suspend: "enter rest mode",
                    reboot: "restart",
                    power_off: "turn off"
                })[action] + ": " + message, "power");
        }
        function onControlFailed(id, message) {
            Base.Notices.fail(message, "bolt");
        }
    }

    Connections {
        target: api.home
        // A press opens the Control Center over the game (held, the host goes home); with no overlay window the host lands home itself.
        function onPressed() {
            if (root.launching || splash.running) {
                // The splash holds while the game loads: HOME raises the reduced Control Center over it.
                if (!splash.waiting)
                    return;
                if (api.home.open)
                    api.home.closeDock();
                else
                    api.home.openDock();
                return;
            }
            if (!root.sessionRunning) {
                if (!root.modal)
                    root.goHome();
                return;
            }
            if (api.home.shown !== "game") {
                if (!root.modal)
                    root.resume();
            } else if (api.home.open)
                api.home.closeDock();
            else
                api.home.openDock();
        }
        function onStopping(title) {
            Base.Notices.show("Closing " + title + "…", "stop");
        }
        function onChanged() {
            var shown = api.home.shown;
            if (shown === "game" && root.lastShown !== "game")
                home.blank();
            else if (shown === "launcher" && root.lastShown === "game")
                root.landHome();
            root.lastShown = shown;
        }
    }

    property string lastShown: api.home.shown

    // Back from a game: no frame bridges the swap, so the home builds itself up from black, as the console's.
    function landHome() {
        var landing = api.home.takeLanding();
        stack.clear();
        home.rebuild();
        Qt.callLater(focusTop);
        openLanding(landing);
    }

    // What the dock asked for over the game: its details, trophies, captures, journal or log.
    function openLanding(landing) {
        var id = playingId;
        if (landing === "" || id === "")
            return;
        if (landing === "details") {
            home.focusGame(id);
            home.down();
            return;
        }
        var page = {
            achievements: "AchievementsPage",
            journal: "NewsPage",
            recordings: "MediaGalleryPage",
            screenshots: "MediaGalleryPage",
            sessions: "PlayLogPage"
        }[landing];
        if (page)
            push("pages/" + page + ".qml", {
                gameId: id
            });
    }

    Keys.onPressed: function (event) {
        if (root.modal) {
            event.accepted = true;
            if (root.launching && splash.waiting && splash.launchedSession !== "" && !event.isAutoRepeat && api.keys.isCancel(event))
                splash.done();
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            if (!root.onHome) {
                Sound.play("back");
                root.pop();
            } else if (!home.back()) {
                Sound.play("edge");
            }
            return;
        }
        if (api.keys.isMenu(event)) {
            event.accepted = true;
            root.goHome();
        }
    }
}
