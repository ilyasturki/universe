import QtQuick
import "../core" as Base
import "core"
import "sound"
import "ui"
import "pages"

FocusScope {
    id: root

    focus: true

    Component.onCompleted: {
        Sound.preload();
        if (api.theme.takeLanding() === "themes")
            push("pages/SettingsPage.qml", { section: "themes" });
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

    // args travel as JSON: a library reload can delete a Game under a page, so pages carry ids.
    readonly property ListModel stack: ListModel {}
    readonly property int depth: stack.count
    readonly property bool onHome: depth === 0
    property string homeFocus: "home"
    property bool launching: false
    readonly property bool modal: dialog.open || sheet.open || picker.open || folder.open || launching
    readonly property var session: api.universe.currentSession
    readonly property bool sessionRunning: session !== null && session !== undefined && session.session_id !== undefined
    property var pendingLaunch: null

    readonly property var barItems: [
        { id: "software", icon: "grid", color: Theme.barRed, label: "All Software", source: "pages/AllSoftwarePage.qml" },
        { id: "news", icon: "news", color: Theme.barGreen, label: "News", source: "pages/NewsPage.qml" },
        { id: "shop", icon: "shop", color: Theme.barOrange, label: "Install", source: "pages/InstallPage.qml" },
        { id: "album", icon: "album", color: Theme.barBlue, label: "Album", source: "pages/AlbumPage.qml" },
        { id: "controllers", icon: "controllers", color: Theme.barGrey, label: "Controllers", source: "pages/ControllersPage.qml" },
        { id: "settings", icon: "settings", color: Theme.barGrey, label: "System Settings", source: "pages/SettingsPage.qml" },
        { id: "power", icon: "power", color: Theme.barGrey, label: "Quit" }
    ]

    readonly property Item topPage: pages.count > 0 && pages.itemAt(pages.count - 1) ? pages.itemAt(pages.count - 1).item : null
    readonly property var hints: dialog.open ? dialog.hints
                               : sheet.open ? sheet.hints
                               : picker.open ? picker.hints
                               : folder.open ? folder.hints
                               : launching ? []
                               : !onHome && topPage ? topPage.hints
                               : homeFocus === "bar" ? bottomBar.hints
                               : home.hints

    function push(source, args) {
        stack.append({ source: source, argsJson: JSON.stringify(args || {}) });
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
        homeFocus = "home";
        Qt.callLater(focusTop);
    }

    function focusTop() {
        var target = onHome ? (homeFocus === "bar" ? bottomBar : home) : topPage;
        if (!modal && target)
            target.forceActiveFocus();
    }

    function openBar(item) {
        if (item.id === "power")
            dialog.show({ message: "Quit Universe?", detail: api.universe.currentSession ? "The running game is closed with it." : "", buttons: ["Cancel", "Quit"] },
                        function(i) { if (i === 1) Qt.quit(); else focusTop(); });
        else
            push(item.source, {});
    }

    function after(done) {
        return function(v) { if (done) done(v); focusTop(); };
    }

    function dialogAsk(spec, done) { dialog.show(spec, after(done)); }
    function prompt(spec, done) { sheet.show(spec, after(done)); }
    function pick(spec, done) { picker.show(spec, after(done)); }
    function browse(spec, done) { folder.show(spec, after(done)); }

    function menu(title, items, done) {
        pick({ title: title, choices: items.map(function(i) { return i.label; }) }, function(i) { if (i >= 0) done(items[i].act); });
    }

    function launch(game) {
        if (!game || launchScreen.running)
            return;
        if (sessionRunning) {
            if (game.id === session.id) {
                resume();
                return;
            }
            var running = session.title;
            dialogAsk({ message: "Close " + running + " and start " + game.title + "?",
                        detail: "Unsaved progress in " + running + " will be lost.",
                        buttons: ["Cancel", "Close and start"], danger: 1 },
                      function(i) {
                          if (i !== 1)
                              return;
                          root.pendingLaunch = game;
                          root.stopSession();
                      });
            return;
        }
        Sound.play("launch");
        launching = true;
        launchScreen.begin(game);
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
        dialogAsk({ message: "Close the software?", detail: "Unsaved progress in " + session.title + " will be lost.",
                    buttons: ["Cancel", "Close"], danger: 1 },
                  function(i) { if (i === 1) root.stopSession(); });
    }

    // The unit gets a SIGTERM, a second one after ~3 s: the toast covers the wait.
    function stopSession() {
        if (!sessionRunning)
            return;
        toast.show("Closing " + session.title + "…");
        api.home.stop();
    }

    function showToast(text) { toast.show(text); }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    Item {
        id: homeLayer

        anchors.fill: parent
        opacity: root.onHome && !root.launching ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity { Ease {} }

        TopBar {
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
        }

        HomePage {
            id: home
            objectName: "homePage"
            anchors.fill: parent
            shell: root
            focus: root.onHome && root.homeFocus === "home"
            onEscapedDown: {
                root.homeFocus = "bar";
                bottomBar.forceActiveFocus();
            }
        }

        BottomBar {
            id: bottomBar
            x: (parent.width - width) / 2
            y: Theme.dp(Theme.barY)
            items: root.barItems
            focus: root.onHome && root.homeFocus === "bar"
            onActivated: function(item) { root.openBar(item); }
            onEscapedUp: {
                root.homeFocus = "home";
                home.forceActiveFocus();
            }
        }
    }

    Repeater {
        id: pages
        model: root.stack

        Loader {
            id: pageLoader

            readonly property bool isTop: index === root.depth - 1

            anchors.fill: parent
            source: model.source
            opacity: 0.0
            visible: opacity > 0.01
            focus: isTop

            Behavior on opacity { Ease {} }

            Rectangle {
                anchors.fill: parent
                z: -1
                color: Theme.ground
            }

            // Bound after creation, so a page fades in instead of appearing at full opacity.
            Component.onCompleted: opacity = Qt.binding(function() { return isTop && !root.launching ? 1.0 : 0.0; })

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

    Rectangle {
        id: chip

        readonly property bool shown: root.sessionRunning && hintBar.visible && !root.onHome
        property int elapsed: 0

        x: Theme.dp(160)
        y: parent.height - (Theme.dp(Theme.hintBarHeight) + height) / 2
        z: 11.6
        width: chipRow.width + Theme.dp(36)
        height: Theme.dp(50)
        radius: height / 2
        color: Theme.card
        border.width: 1
        border.color: Theme.hairline
        opacity: shown ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity { Ease { duration: Theme.durQuick } }

        Timer {
            interval: 60000
            running: chip.shown
            repeat: true
            triggeredOnStart: true
            onTriggered: {
                var started = root.session && root.session.started_at ? Date.parse(root.session.started_at) : NaN;
                chip.elapsed = isNaN(started) ? 0 : Math.max(0, Math.round((Date.now() - started) / 1000));
            }
        }

        Row {
            id: chipRow
            anchors.centerIn: parent
            spacing: Theme.dp(12)

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(12)
                height: width
                radius: width / 2
                color: Theme.okGreen
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: (root.session && root.session.title ? root.session.title : "") + " · " + Math.max(1, Math.floor(chip.elapsed / 60)) + " min"
                font.pixelSize: Theme.dp(Theme.fontTiny)
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
        hairline: !root.onHome && !(root.topPage && root.topPage.bare === true)
        visible: !root.launching && !(root.topPage && root.topPage.bare === true && !dialog.open && !sheet.open)
    }

    TextSheet {
        id: sheet
        anchors.fill: parent
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight)
        z: 10
    }

    Picker {
        id: picker
        z: 11
    }

    FolderSheet {
        id: folder
        shell: root
        z: 11
    }

    Dialog {
        id: dialog
        z: 11
    }

    LaunchScreen {
        id: launchScreen
        anchors.fill: parent
        z: 12
        onFinished: {
            root.launching = false;
            root.focusTop();
        }
        onFailed: function(game, message) {
            toast.show("Could not start" + (game ? " " + game.title : "") + (message ? ": " + message : ""));
        }
    }

    Toast {
        id: toast
        z: 13
    }

    Connections {
        target: api.universe
        function onError(kind, message) {
            toast.show(message);
            root.pendingLaunch = null;
        }
        function onSessionEnded(sessionId, id, duration) {
            var game = api.allGames.byId(id);
            if (game)
                toast.show(game.title + " · " + Math.max(1, Math.round(duration / 60)) + " min");
            if (root.pendingLaunch)
                root.launch(root.pendingLaunch);
            root.pendingLaunch = null;
        }
    }

    Connections {
        target: api.screens.controller
        function onMacroNotice(text) { toast.show(text); }
    }

    property string lastShown: api.home.shown

    Connections {
        target: api.home
        // Nothing bridges the swap here: the HOME menu is up as soon as the host asks.
        function onChanged() {
            if (api.home.shown === "launcher" && root.lastShown === "game")
                api.home.covered();
            root.lastShown = api.home.shown;
        }
        function onPressed() {
            if (!root.sessionRunning || root.launching)
                return;
            if (api.home.shown === "game") {
                if (root.depth === 0)
                    Sound.play("home");
                api.home.toLauncher();
                root.goHome();
            } else if (!root.modal) {
                root.resume();
            }
        }
    }

    Keys.onPressed: function(event) {
        if (root.modal) {
            event.accepted = true;
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            if (!root.onHome) {
                Sound.play("back");
                root.pop();
            } else if (root.homeFocus === "bar") {
                Sound.play("back");
                root.homeFocus = "home";
                home.forceActiveFocus();
            } else {
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
