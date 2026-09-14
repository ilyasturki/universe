import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../ui/Macros.js" as Macros

// The Settings tab: a sidebar of sections and one column of cards beside it. The runners (a list;
// each opens its own page), modules and their global settings, a source's library to install
// from, pending updates, the login flow, the controller's macros, the look, doctor's checks. One
// set of cards, eight row sources.
FocusScope {
    id: page

    focus: true

    signal chromeRequested()
    signal settingsRequested(var game)
    signal runnerRequested(string runner)
    signal artworkRequested(var game, string slot)

    readonly property var currentGame: null
    readonly property bool ownsBackdrop: false
    readonly property real backdropBlur: 30
    readonly property real chromeScrim: 0
    readonly property real scrimTop: 0.9
    readonly property real scrimMid: 0.94
    readonly property real scrimBottom: 0.98
    readonly property Item focusedArtItem: null
    readonly property Item menuAnchor: null
    readonly property bool ownsAccept: true
    property bool menuOpen: false

    readonly property var sections: ["Runners", "Modules", "Install", "Updates", "Login", "Controller", "Themes", "Doctor", "Artwork"]
    readonly property var sectionIcons: ["play", "grid", "download", "refresh", "user", "gamepad", "sun", "pulse", "image"]
    readonly property int runnersSection: 0
    readonly property int modulesSection: 1
    readonly property int installSection: 2
    readonly property int updatesSection: 3
    readonly property int loginSection: 4
    readonly property int controllerSection: 5
    readonly property int themesSection: 6
    readonly property int doctorSection: 7
    // Its own column, not cards: ArtworkOverview.qml.
    readonly property int artworkSection: 8
    property int section: 0

    readonly property var modulesForm: api.screens.modules
    readonly property var runners: api.screens.runners
    readonly property var sources: api.screens.sources
    readonly property var login: api.screens.login
    readonly property var controller: api.screens.controller

    // The watcher reports instead of firing while the section is on screen, so a paddle
    // pressed to find its row does not take a screenshot.
    readonly property bool controllerOpen: section === controllerSection && activeFocus
    readonly property bool learning: section === controllerSection && controller.learning !== ""
    // The live view: the pad alone in the column, its presses muted as keys (see Api), left by
    // holding Circle/B or pressing Start and Select together, both read from the watcher.
    readonly property bool testing: section === controllerSection && controller.testing
    property var held: ({})
    property string pendingSlot: ""
    property string pendingTrigger: ""
    // The runner whose page is open: the list reloads under it and the cursor finds it again.
    property string openedRunner: ""

    readonly property var hints: editor.open ? editor.hints
        : menu.open ? menu.hints
        : artwork.item && artwork.item.activeFocus ? artwork.item.hints
        : testing ? [ { glyph: "B", label: "Hold to finish" }, { glyph: "Start+Select", label: "Finish" } ]
        : learning ? [ { glyph: "B", label: "Stop learning" } ]
        : side.activeFocus
        ? [ { glyph: "A", label: "Open" }, { glyph: "B", label: "Back" }, { glyph: "dpad", label: "Section" }, { glyph: "LB RB", label: "Tabs" } ]
        : (acceptLabel !== "" ? [ { glyph: "A", label: acceptLabel } ] : []).concat(
            refreshable ? [ { glyph: "X", label: "Refresh" } ] : [],
            [ { glyph: "B", label: "Sections" }, { glyph: "dpad", label: "Navigate" }, { glyph: "LT RT", label: "Section" }, { glyph: "LB RB", label: "Tabs" } ])

    readonly property string acceptLabel: {
        var row = cards.currentRow;
        if (!row || row.type === "info")
            return "";
        if (row.type === "bool")
            return "Toggle";
        if (row.type === "search")
            return "Search";
        if (row.type === "action")
            return row.action !== undefined ? row.action : "Select";
        return "Change";
    }

    // The sections that reach the network keep what they fetched; X fetches again.
    readonly property bool refreshable: section >= installSection && section <= loginSection

    // The sidebar at the left margin; the column of cards centred in what is left, capped so a
    // row's label and its value stay within one glance.
    readonly property real sideMargin: Theme.dp(80)
    readonly property real sideWidth: Theme.dp(300)
    readonly property real mainRoom: width - sideMargin * 2 - sideWidth - Theme.dp(56)
    readonly property real mainWidth: Math.min(mainRoom, Theme.dp(1280))
    readonly property real mainX: sideMargin + sideWidth + Theme.dp(56) + (mainRoom - mainWidth) / 2

    readonly property var currentSource: sources.current
    readonly property string sourceName: currentSource ? currentSource.name : sources.source
    readonly property bool loggedIn: currentSource ? currentSource.logged_in === true : false

    // The source's module card carries its version and kind; the login card borrows them.
    readonly property string sourceMeta: {
        var groups = modulesForm.groups;
        for (var i = 0; i < groups.length; i++)
            if (groups[i].title === sourceName)
                return groups[i].meta;
        return "";
    }

    function games(n) { return n + (n === 1 ? " game" : " games"); }

    // The library's own cover when the game is in it, else the store's picture.
    function artOf(g) {
        var game = g.game_id ? api.allGames.byId(g.game_id) : null;
        if (game) {
            var art = String(game.assets.boxFront) !== "" ? game.assets.boxFront : game.assets.tile;
            if (String(art) !== "")
                return art;
        }
        return g.image || "";
    }

    // The rows and the cards that arrange them for the open section, from the host's data.
    readonly property var content: {
        var rows = [], groups = [];
        if (section === artworkSection)
            return { rows: rows, groups: groups };
        if (section === modulesSection)
            return { rows: modulesForm.rows, groups: modulesForm.groups };
        if (section === runnersSection)
            return { rows: runners.rows, groups: runners.groups };
        if (section === installSection) {
            // The search field is the first row of the first card; the cursor lands past it.
            rows.push({ section: sourceName, key: "search", label: "Search " + sourceName, type: "search", icon: "search",
                        display: sources.query || "", choices: [], detail: "" });
            var installed = [], owned = [], all = [];
            for (var i = 0; i < sources.rows.length; i++) {
                var g = sources.rows[i];
                rows.push({ section: sourceName, key: "game", label: g.title, type: "action",
                            display: g.status, choices: [], detail: "", row: i, installed: g.installed,
                            pending: g.pending, gameId: g.game_id, image: page.artOf(g), action: "Options" });
                all.push(rows.length - 1);
                (g.installed ? installed : owned).push(rows.length - 1);
            }
            var busy = sources.busy ? (all.length > 0 ? " · refreshing…" : "loading…") : "";
            if (sources.query)
                groups.push({ title: "Results", meta: games(all.length) + " · “" + sources.query + "”" + busy, rows: [0].concat(all) });
            else {
                var where = currentSource && currentSource.games_dir ? " · " + currentSource.games_dir : "";
                groups.push({ title: "Installed", meta: (all.length > 0 || !busy ? games(installed.length) + where : "") + busy, rows: [0].concat(installed) });
                groups.push({ title: "Owned, not installed", meta: games(owned.length), rows: owned });
            }
            return { rows: rows, groups: groups };
        }
        if (section === updatesSection) {
            var n = sources.updates.length;
            if (n === 0) {
                rows.push({ section: "Updates", key: "", label: sources.busy ? "Checking…" : "Everything is up to date",
                            type: "info", value: true, detail: "" });
                groups.push({ title: "Updates", rows: [0] });
                return { rows: rows, groups: groups };
            }
            rows.push({ section: "Updates", key: "all", label: "Update everything", type: "action", display: n + " pending", detail: "" });
            for (var j = 0; j < n; j++) {
                var u = sources.updates[j];
                rows.push({ section: "Updates", key: "update", label: u.title, type: "action",
                            display: (u.version ? u.version + " · " : "") + (u.date || ""), detail: "", row: j });
            }
            groups.push({ title: "Pending", meta: n + (n === 1 ? " update" : " updates"),
                          rows: rows.map(function(r, i) { return i; }) });
            return { rows: rows, groups: groups };
        }
        if (section === loginSection) {
            rows.push({ section: sourceName, key: "", label: "Signed in", type: "info", value: loggedIn, detail: loggedIn ? "yes" : "no" });
            rows.push({ section: sourceName, key: "link", label: "Get a sign-in link", type: "action", display: login.url ? "ready" : "", detail: "" });
            rows.push({ section: sourceName, key: "code", label: "Enter the code", type: "action", display: "", detail: "" });
            groups.push({ title: sourceName, meta: sourceMeta, rows: [0, 1, 2] });
            return { rows: rows, groups: groups };
        }
        if (section === controllerSection) {
            if (!learning)
                return { rows: controller.rows, groups: controller.groups };
            // The row being learned says so in place of its chips, for as long as it listens.
            var listening = controller.rows.map(function(r) {
                if (r.slot !== controller.learning)
                    return r;
                var out = {};
                for (var k in r)
                    out[k] = r[k];
                out.display = "Press the button on the pad…";
                out.press = null;
                out.hold = null;
                return out;
            });
            return { rows: listening, groups: controller.groups };
        }
        if (section === themesSection) {
            rows.push({ section: "Themes", key: "theme", label: "Theme", type: "enum", value: api.theme.name, display: api.theme.name,
                        choices: api.theme.themes.map(function(t) { return t.name; }), detail: "" });
            groups.push({ title: "Look", rows: [0] });
            return { rows: rows, groups: groups };
        }
        return { rows: modulesForm.doctor, groups: modulesForm.doctorGroups };
    }

    readonly property int checksPassed: {
        var n = 0;
        for (var i = 0; i < modulesForm.doctor.length; i++)
            if (modulesForm.doctor[i].value)
                n++;
        return n;
    }

    function leave() {
        editor.hide();
        if (menu.open)
            menu.hide();
    }

    // Modules and Doctor read the core in-process; the sources keep their last fetch (see load).
    function refresh() {
        if (section === modulesSection)
            modulesForm.load();
        else if (section === runnersSection)
            runners.load();
        else if (refreshable)
            sources.load();
        else if (section === controllerSection)
            controller.load();
        else if (section === artworkSection) {
            if (artwork.item)
                artwork.item.load();
        } else
            modulesForm.loadDoctor();
    }

    function refreshNow() {
        if (refreshable) {
            Sound.enter();
            sources.refresh();
        } else if (section === doctorSection) {
            Sound.enter();
            modulesForm.loadDoctor();
        } else if (section === controllerSection) {
            Sound.enter();
            controller.load();
        } else if (section === runnersSection) {
            Sound.enter();
            runners.load();
        } else {
            Sound.edge();
        }
    }

    function activate(index, row) {
        if (section === modulesSection) {
            if (row.type === "bool") {
                modulesForm.toggle(index);
                Sound.favourite(!row.value);
            } else {
                Sound.panel();
                editor.edit(index, row);
            }
        } else if (section === runnersSection) {
            // The runner's page takes the focus next and hands it back here, on this row.
            cards.forceActiveFocus();
            openedRunner = row.runner;
            page.runnerRequested(row.runner);
        } else if (section === installSection) {
            if (row.key === "search") {
                Sound.panel();
                editor.prompt("search", "Search " + sourceName, sources.query);
            } else {
                Sound.panel();
                menu.row = row;
                menu.show(page.gameActions(row), cards, cards.focusRect, row.label);
            }
        } else if (section === updatesSection) {
            Sound.enter();
            if (row.key === "all")
                sources.updateAll();
            else
                sources.update(row.row);
        } else if (section === loginSection) {
            if (row.key === "link") {
                Sound.enter();
                login.begin(sources.source);
            } else if (row.key === "code") {
                Sound.panel();
                editor.prompt("code", "Code from " + sourceName, "");
            }
        } else if (section === themesSection) {
            Sound.panel();
            editor.edit(index, row);
        } else if (section === controllerSection) {
            if (row.key === "test") {
                if (controller.setTesting(true))
                    Sound.enter();
            } else if (row.type === "enum") {
                Sound.panel();
                editor.edit(index, row);
            } else if (row.type === "action") {
                Sound.panel();
                menu.row = row;
                menu.show(page.slotActions(row), cards, cards.focusRect, row.label);
            } else {
                Sound.edge();
            }
        }
    }

    // What a button of the pad can be given: a macro on press or on hold, its button learned
    // (first when the slot has none on this connection), a macro cleared.
    function slotActions(row) {
        var out = [];
        if (!row.bound)
            out.push({ icon: "keyboard", label: "Learn the button", action: "learn" });
        out.push({ icon: "play", label: "On press…", action: "press" });
        out.push({ icon: "stop", label: "On hold…", action: "hold" });
        if (row.press)
            out.push({ icon: "trash", label: "Clear press", action: "clear-press", danger: true });
        if (row.hold)
            out.push({ icon: "trash", label: "Clear hold", action: "clear-hold", danger: true });
        if (row.bound)
            out.push({ icon: "keyboard", label: "Learn the button again", action: "learn" });
        return out;
    }

    function presetActions(trigger) {
        var out = [];
        var presets = controller.presets;
        for (var i = 0; i < presets.length; i++) {
            var p = presets[i];
            if (p.hold_only && trigger !== "hold")
                continue;
            if (p.id === "keys" || p.id === "command")
                continue;
            out.push({ icon: Macros.icon(p.id), label: p.label, action: "preset:" + p.id });
        }
        out.push({ icon: Macros.icon("keys"), label: "Key combo…", action: "keys" });
        out.push({ icon: Macros.icon("command"), label: "Command…", action: "command" });
        return out;
    }

    function slotAction(action) {
        var row = menu.row;
        if (!row)
            return;
        if (action === "learn") {
            if (controller.learn(row.key))
                Sound.enter();
        } else if (action === "press" || action === "hold") {
            Sound.panel();
            pendingSlot = row.key;
            pendingTrigger = action;
            menu.show(page.presetActions(action), cards, cards.focusRect,
                      row.label + " · " + (action === "press" ? "On press" : "On hold"));
            return;
        } else if (action.indexOf("preset:") === 0) {
            Sound.enter();
            controller.bind(pendingSlot, pendingTrigger, action.substring(7), "", "");
        } else if (action === "keys" || action === "command") {
            Sound.panel();
            var current = pendingTrigger === "hold" ? row.hold : row.press;
            var value = current && current.action === action ? current[action] : "";
            editor.prompt(action, (action === "keys" ? "Key combo for " : "Command for ") + row.label, value);
            return;
        } else if (action === "clear-press") {
            Sound.cancel();
            controller.unbind(row.key, "press");
        } else if (action === "clear-hold") {
            Sound.cancel();
            controller.unbind(row.key, "hold");
        }
        cards.forceActiveFocus();
    }

    onActiveFocusChanged: {
        if (!activeFocus || openedRunner === "")
            return;
        runners.load();
        var i = runners.indexOf(openedRunner);
        openedRunner = "";
        if (i >= 0)
            cards.index = i;
    }

    onControllerOpenChanged: {
        if (controllerOpen)
            controller.suspend();
        else
            controller.resume();
    }

    onTestingChanged: {
        art.clear();
        held = ({});
        holdOut.stop();
        if (testing) {
            tester.forceActiveFocus();
            return;
        }
        var rows = controller.rows;
        for (var i = 0; i < rows.length; i++)
            if (rows[i].key === "test") {
                cards.index = i;
                break;
            }
        cards.forceActiveFocus();
    }

    Timer {
        id: holdOut
        interval: 1000
        onTriggered: {
            Sound.cancel();
            page.controller.setTesting(false);
        }
    }

    // What a game of the source can have done to it: installed, or, once it is, updated,
    // configured (when the library lists it), uninstalled, removed.
    function gameActions(row) {
        var out = [];
        if (!row.installed) {
            out.push({ icon: "download", label: "Install", action: "install" });
            return out;
        }
        if (row.pending)
            out.push({ icon: "refresh", label: "Update", action: "update" });
        if (row.gameId && api.allGames.byId(row.gameId))
            out.push({ icon: "sliders", label: "Game settings", action: "settings" });
        if (row.gameId) {
            out.push({ icon: "trash", label: "Uninstall…", action: "uninstall", danger: true });
            out.push({ icon: "eye-off", label: "Remove from library…", action: "remove", danger: true });
        }
        return out;
    }

    function gameAction(action) {
        var row = menu.row;
        if (!row)
            return;
        if (action === "install" || action === "update") {
            Sound.enter();
            sources.install(row.row);
        } else if (action === "settings") {
            // The cards get the focus back first: the sub-screen takes it next, and
            // returns it to the page, where the cursor must still be a row.
            cards.forceActiveFocus();
            page.settingsRequested(api.allGames.byId(row.gameId));
            return;
        } else if (action === "uninstall") {
            Sound.panel();
            menu.show([ { icon: "", label: "Keep it", action: "" },
                        { icon: "trash", label: "Trash the install folder", action: "uninstall!", danger: true } ],
                      cards, cards.focusRect, "Uninstall " + row.label + "?");
            return;
        } else if (action === "remove") {
            Sound.panel();
            menu.show([ { icon: "", label: "Keep it", action: "" },
                        { icon: "eye-off", label: "Remove from the library", action: "remove!", danger: true } ],
                      cards, cards.focusRect, "Remove " + row.label + "?");
            return;
        } else if (action === "uninstall!") {
            Sound.enter();
            sources.uninstall(row.gameId);
        } else if (action === "remove!") {
            Sound.enter();
            sources.remove(row.gameId);
        }
        cards.forceActiveFocus();
    }

    Component.onCompleted: {
        modulesForm.load();
        runners.load();
        modulesForm.loadDoctor();
        sources.load();
    }

    // The cards' rows rebind on the same signal; the cursor resets once they have.
    onSectionChanged: {
        refresh();
        Qt.callLater(cards.reset);
    }

    // Triggers cycle the section from anywhere, as they cycle the collection in the library.
    function stepSection(d) {
        var n = sections.length;
        section = (section + d + n) % n;
        Sound.tick();
    }

    Connections {
        target: page.sources
        function onMessage(text) { toast.show(text); }
    }

    Connections {
        target: page.login
        function onFinished(ok, text) {
            toast.show(text);
            page.sources.load();
        }
    }

    // In the live view a press lights its button on the art and counts towards the way out.
    Connections {
        target: page.controller
        function onButtonPressed(id, slot, pressed) {
            if (!page.testing || id !== page.controller.current)
                return;
            art.press(slot, pressed);
            var next = {};
            for (var k in page.held)
                if (k !== slot)
                    next[k] = true;
            if (pressed)
                next[slot] = true;
            page.held = next;
            if (slot === "east") {
                if (pressed)
                    holdOut.restart();
                else
                    holdOut.stop();
            }
            if (pressed && next.start && next.select) {
                Sound.cancel();
                page.controller.setTesting(false);
            }
        }
        function onAxisMoved(id, axis, value) {
            if (page.testing && id === page.controller.current)
                art.axis(axis, value);
        }
        function onUnknownPressed(id, code) {
            if (page.section === page.controllerSection && !page.learning && id === page.controller.current)
                toast.show(code + " is not one of the pad's buttons yet: learn it from a row");
        }
        function onLearned(family, slot, code) {
            var rows = page.controller.rows;
            for (var i = 0; i < rows.length; i++)
                if (rows[i].slot === slot) {
                    toast.show(rows[i].label + " is now " + code);
                    return;
                }
            toast.show(slot + " is now " + code);
        }
        function onMessage(text) { toast.show(text); }
    }

    Text {
        id: titleText

        x: page.sideMargin
        y: Theme.dp(34)
        text: "Settings"
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(50)
    }

    SectionList {
        id: side

        x: page.sideMargin
        y: titleText.y + titleText.height + Theme.dp(26)
        width: page.sideWidth
        focus: true
        sections: page.sections
        icons: page.sectionIcons
        badges: page.sections.map(function(name, i) {
            return i === page.updatesSection && page.sources.updates.length > 0 ? page.sources.updates.length.toString() : "";
        })
        current: page.section
        opacity: page.testing ? 0.35 : 1.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
        }

        onRequested: function(index) { page.section = index; }
        onEntered: page.section === page.artworkSection && artwork.item ? artwork.item.forceActiveFocus() : cards.forceActiveFocus()
        onEscapedUp: page.chromeRequested()
    }

    Loader {
        id: artwork

        x: page.mainX
        y: side.y
        width: page.mainWidth
        height: page.height - y
        active: page.section === page.artworkSection
        source: "ArtworkOverview.qml"
        visible: active

        onLoaded: item.load()

        Connections {
            target: artwork.item
            ignoreUnknownSignals: true
            function onOpenRequested(game, slot) { page.artworkRequested(game, slot); }
            function onEscapedLeft() {
                Sound.panel();
                side.forceActiveFocus();
            }
            function onEscapedUp() { page.chromeRequested(); }
            function onMessage(text) { toast.show(text); }
        }
    }

    // Above the cards: a running job's message and bar, and on Doctor the tally of checks.
    Column {
        id: above

        x: page.mainX
        y: side.y
        width: page.mainWidth
        spacing: Theme.dp(32)

        Rectangle {
            id: jobBar

            width: parent.width
            height: Theme.dp(64)
            radius: Theme.dp(14)
            color: Theme.surface
            visible: page.sources.job !== null && page.sources.job !== undefined

            readonly property var job: page.sources.job
            readonly property real fraction: job && job.total > 0 ? job.done / job.total : 0

            Text {
                anchors.left: parent.left
                anchors.leftMargin: Theme.dp(22)
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                anchors.verticalCenterOffset: -Theme.dp(6)
                text: jobBar.job ? jobBar.job.message + (jobBar.job.ok === true ? " ✓" : jobBar.job.ok === false ? " ✗" : "") : ""
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(21)
                elide: Text.ElideRight
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: Theme.dp(10)
                height: Theme.dp(5)
                radius: height / 2
                color: Qt.rgba(1, 1, 1, 0.15)

                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: parent.width * (jobBar.job && jobBar.job.ok !== null && jobBar.job.ok !== undefined ? 1 : jobBar.fraction)
                    radius: height / 2
                    color: Theme.text

                    Behavior on width {
                        NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                    }
                }
            }
        }

        Row {
            id: tally

            visible: page.section === page.doctorSection && page.modulesForm.doctor.length > 0
            height: Theme.dp(30)
            spacing: Theme.dp(28)

            Repeater {
                model: {
                    var passed = page.checksPassed, failed = page.modulesForm.doctor.length - passed;
                    return [
                        { count: passed, text: passed + (passed === 1 ? " check passes" : " checks pass"), color: "#5fd48a" },
                        { count: failed, text: failed + (failed === 1 ? " needs attention" : " need attention"), color: "#e0655a" }
                    ];
                }

                Row {
                    visible: modelData.count > 0
                    spacing: Theme.dp(12)
                    anchors.verticalCenter: parent.verticalCenter

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: Theme.dp(14)
                        height: width
                        radius: width / 2
                        color: modelData.color
                    }

                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.text
                        color: Theme.text
                        font.family: Theme.sans
                        font.weight: Font.Medium
                        font.pixelSize: Theme.dp(22)
                    }
                }
            }
        }
    }

    SettingsCards {
        id: cards

        x: page.mainX
        y: above.y + above.height + (above.height > 0 ? Theme.dp(32) : 0)
        width: page.mainWidth
        height: page.height - y
        columns: 1
        rows: page.content.rows
        groups: page.content.groups
        dimmed: editor.open
        opacity: page.testing || page.section === page.artworkSection ? 0.0 : 1.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic }
        }

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedUp: page.chromeRequested()
        onEscapedLeft: {
            Sound.panel();
            side.forceActiveFocus();
        }

        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event) && page.learning) {
                event.accepted = true;
                Sound.cancel();
                page.controller.cancelLearn();
            } else if (api.keys.isCancel(event)) {
                event.accepted = true;
                Sound.cancel();
                side.forceActiveFocus();
            } else if (api.keys.isDetails(event)) {
                event.accepted = true;
                page.refreshNow();
            }
        }
    }

    // The sign-in link as a card under the login rows: the code to scan, the link, the state.
    Item {
        id: loginCard

        x: page.mainX
        y: cards.y + cards.layout.height + Theme.dp(32)
        width: page.mainWidth
        height: Math.max(Theme.dp(74), loginHead.height) + loginBody.height + Theme.dp(8) * 2 + 2
        visible: page.section === page.loginSection && (page.login.url !== "" || page.login.status !== "")

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(24)
            color: Qt.rgba(1, 1, 1, 0.04)
            border.width: 1
            border.color: Qt.rgba(1, 1, 1, 0.10)
        }

        Text {
            id: loginHead
            x: Theme.dp(8) + 1 + Theme.dp(18)
            y: Theme.dp(8) + 1
            height: Theme.dp(74)
            verticalAlignment: Text.AlignVCenter
            text: "Sign-in link"
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(27)
        }

        Row {
            id: loginBody

            x: Theme.dp(8) + 1 + Theme.dp(16)
            y: loginHead.y + loginHead.height
            width: parent.width - x * 2
            height: Math.max(qr.height, loginText.height) + Theme.dp(36)
            spacing: Theme.dp(28)

            QrCode {
                id: qr
                y: Theme.dp(18)
                width: Theme.dp(300)
                height: width
                matrix: page.login.matrix
                visible: page.login.url !== ""
            }

            Column {
                id: loginText
                y: Theme.dp(18)
                width: parent.width - (qr.visible ? qr.width + parent.spacing : 0)
                spacing: Theme.dp(14)

                Text {
                    width: parent.width
                    text: "Scan to sign in on your phone"
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Medium
                    font.pixelSize: Theme.dp(22)
                }

                Text {
                    width: parent.width
                    text: page.login.url
                    color: Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(18)
                    lineHeight: 1.3
                    wrapMode: Text.WrapAnywhere
                    maximumLineCount: 5
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: page.login.status
                    color: Theme.textMuted
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(20)
                    wrapMode: Text.WordWrap
                }
            }
        }
    }

    // The live view: the pad takes the column, every press and pull shown on it.
    ControllerArt {
        id: art

        x: page.mainX
        y: cards.y
        width: page.mainWidth
        height: cards.height
        visible: opacity > 0.01
        opacity: page.testing ? 1.0 : 0.0
        family: page.controller.family
        connected: page.controller.connected
        rows: page.controller.rows
        unbound: page.controller.unboundSlots

        Behavior on opacity { NumberAnimation { duration: Theme.durView; easing.type: Easing.OutCubic } }
    }

    // Holds the keyboard while the pad is on show: Escape leaves, everything else stays put.
    FocusScope {
        id: tester

        anchors.fill: cards

        Keys.onPressed: function(event) {
            event.accepted = true;
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                Sound.cancel();
                page.controller.setTesting(false);
            }
        }
        Keys.onReleased: function(event) { event.accepted = true; }
    }

    ValueEditor {
        id: editor

        anchors.fill: parent
        cards: cards
        z: 3

        onAccepted: function(index, value) {
            if (page.section === page.themesSection) {
                var theme = api.theme.themes.filter(function(t) { return t.name === value; })[0];
                // The switch rebuilds this tree; let the editor finish closing first.
                if (theme)
                    Qt.callLater(function() { api.theme.set(theme.id); });
            } else if (page.section === page.controllerSection)
                page.controller.setValue(index, value);
            else
                page.modulesForm.setValue(index, value);
        }
        onPrompted: function(tag, value) {
            if (tag === "search")
                page.sources.search(value);
            else if (tag === "code")
                page.login.submit(value);
            else if ((tag === "keys" || tag === "command") && value !== "")
                page.controller.bind(page.pendingSlot, page.pendingTrigger, tag, tag === "keys" ? value : "", tag === "command" ? value : "");
        }
        onClosed: cards.forceActiveFocus()
    }

    ActionMenu {
        id: menu

        property var row: null

        anchors.fill: parent
        z: 4

        onChosen: function(action) {
            if (page.section === page.controllerSection)
                page.slotAction(action);
            else
                page.gameAction(action);
        }
        onDismissed: cards.forceActiveFocus()
    }

    Toast {
        id: toast
        z: 6
    }

    Keys.onPressed: function(event) {
        if (api.keys.isPageUp(event) || api.keys.isPageDown(event))
            event.accepted = true;
    }

    Keys.onReleased: function(event) {
        if (event.isAutoRepeat || editor.open || menu.open)
            return;
        if (api.keys.isPageUp(event)) {
            event.accepted = true;
            page.stepSection(-1);
        } else if (api.keys.isPageDown(event)) {
            event.accepted = true;
            page.stepSection(1);
        }
    }
}
