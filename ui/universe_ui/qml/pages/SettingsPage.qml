import QtQuick
import "../core"
import "../core/Format.js" as Format
import "../sound"
import "../ui"
import "../ui/Macros.js" as Macros
import "../ui/Sections.js" as Sections

FocusScope {
    id: page

    focus: true

    signal chromeRequested
    signal settingsRequested(var game)
    signal launchRequested(var game)
    signal detailRequested(var game)
    signal runnerRequested(string runner)
    signal moduleRequested(string module)
    signal sourceRequested(string source)
    signal setupRequested
    signal artworkRequested(var game, string slot)
    signal message(string text)

    readonly property var currentGame: null
    readonly property bool ownsBackdrop: false
    readonly property real backdropBlur: 30
    readonly property real scrimTop: 0.9
    readonly property real scrimMid: 0.94
    readonly property real scrimBottom: 0.98
    readonly property Item menuAnchor: null
    readonly property bool modal: editor.open || menu.open || dialog.open || testing || learning || walking

    readonly property var sections: Sections.list
    property int section: 0
    readonly property string sectionId: sections[section].id

    function sectionIndex(id) {
        var to = Sections.aliases[id] || id;
        return sections.findIndex(function (s) {
            return s.id === to;
        });
    }

    function land(name) {
        var index = sectionIndex(name);
        if (index < 0)
            return;
        section = index;
        focusMain();
    }

    function focusMain() {
        if (artShown)
            tester.forceActiveFocus();
        else if (sectionId === "artwork" && artwork.item)
            artwork.item.forceActiveFocus();
        else
            cards.forceActiveFocus();
    }

    // A hit of the tab bar's search on this tab: its section, the row revealed (the Advanced row opened when it sits behind it).
    function open(target) {
        var index = sectionIndex(target.page === "section" ? target.id : target.page);
        if (index >= 0) {
            section = index;
            var form = target.page === "launch" ? launch : target.page === "controller" ? controller : null;
            Qt.callLater(function () {
                if (form && target.key) {
                    var i = form.reveal(target.key, "");
                    if (i >= 0)
                        cards.index = i;
                }
                page.focusMain();
            });
        }
    }

    readonly property var modulesForm: api.screens.modules
    readonly property var sourceList: api.screens.sourceList
    readonly property var listForm: sectionId === "modules" ? modulesForm : sectionId === "sources" ? sourceList : null
    readonly property var launch: api.screens.launch
    readonly property var runners: api.screens.runners
    readonly property var sources: api.screens.sources
    readonly property var controller: api.screens.controller

    readonly property bool controllerOpen: sectionId === "controller" && activeFocus
    readonly property bool learning: sectionId === "controller" && controller.learning !== ""
    readonly property bool testing: sectionId === "controller" && controller.testing
    readonly property bool walking: sectionId === "controller" && controller.walking
    // The art fills the section while the buttons are tested or walked through.
    readonly property bool artShown: testing || walking
    property string pendingSlot: ""
    property string pendingTrigger: ""
    property string openedRunner: ""
    property string openedModule: ""
    property string openedSource: ""

    readonly property var hints: editor.open ? editor.hints : menu.open ? menu.hints : dialog.open ? dialog.hints : artwork.item && artwork.item.activeFocus ? artwork.item.hints : testing ? [
        {
            glyph: "B",
            label: "Hold to finish"
        },
        {
            glyph: "Start+Select",
            label: "Finish"
        }
    ] : walking ? [] : learning ? [
        {
            glyph: "B",
            label: "Stop learning"
        }
    ] : side.activeFocus ? [
        {
            glyph: "A",
            label: "Open"
        },
        {
            glyph: "B",
            label: "Back"
        }
    ] : [
        {
            glyph: "A",
            label: acceptLabel !== "" ? acceptLabel : "Select",
            dim: acceptLabel === ""
        },
        {
            glyph: "Start",
            label: "More",
            dim: moreItems.length === 0
        },
        {
            glyph: "B",
            label: "Back"
        }
    ]

    // X and Y do these straight away; More lists them.
    readonly property var moreItems: {
        var out = [];
        var row = cards.currentRow;
        if (listForm !== null && row && !(row.warning !== "" && row.value !== true))
            out.push({
                icon: row.value === true ? "eye-off" : "check",
                label: row.value === true ? "Disable" : "Enable",
                action: "toggle"
            });
        if (canRemove)
            out.push({
                icon: "trash",
                label: "Remove",
                action: "refresh"
            });
        else if (canRefresh)
            out.push({
                icon: "refresh",
                label: "Refresh",
                action: "refresh"
            });
        if (sectionId === "launch" && launch.hasAdvanced)
            out.push({
                icon: "sliders",
                label: launch.showAdvanced ? "Hide advanced" : "Show advanced",
                action: "advanced"
            });
        return out;
    }

    function openMenu() {
        if (side.activeFocus || artShown || sectionId === "artwork" || moreItems.length === 0) {
            Sound.edge();
            return;
        }
        Sound.panel();
        menu.show(moreItems, cards, cards.focusRect, "", function (action) {
            cards.forceActiveFocus();
            if (action === "toggle")
                page.toggleModule();
            else if (action === "refresh")
                page.refreshNow();
            else if (action === "advanced")
                page.toggleAdvanced();
        });
    }

    // X on the Launch section: a variable's row goes out of its map.
    readonly property bool canRemove: sectionId === "launch" && cards.currentRow !== null && cards.currentRow.entry !== undefined && launch.resettable(cards.currentRow)

    readonly property string acceptLabel: {
        var row = cards.currentRow;
        if (!row || row.type === "info" || row.type === "static")
            return "";
        if (row.type === "bool")
            return "Toggle";
        if (row.type === "action")
            return row.action !== undefined ? row.action : "Select";
        return "Change";
    }

    readonly property bool refreshable: sectionId === "install"
    readonly property bool canRefresh: refreshable || sectionId === "doctor" || sectionId === "controller" || sectionId === "sound"

    readonly property real sideMargin: Theme.dp(80)
    readonly property real sideWidth: Theme.dp(300)
    readonly property real mainRoom: width - sideMargin * 2 - sideWidth - Theme.dp(56)
    readonly property real mainWidth: Math.min(mainRoom, Theme.dp(1280))
    readonly property real mainTop: Theme.dp(34)
    readonly property real mainX: sideMargin + sideWidth + Theme.dp(56) + (mainRoom - mainWidth) / 2

    readonly property var currentSource: sources.current
    readonly property string sourceName: currentSource ? currentSource.name : sources.source

    readonly property var content: {
        var rows = [], groups = [];
        if (sectionId === "artwork")
            return {
                rows: rows,
                groups: groups
            };
        if (listForm !== null)
            return {
                rows: listForm.rows,
                groups: listForm.groups
            };
        if (sectionId === "launch")
            return {
                rows: launch.rows,
                groups: launch.groups
            };
        if (sectionId === "runners")
            return {
                rows: runners.rows,
                groups: runners.groups
            };
        if (sectionId === "install") {
            var jobs = [], installed = [], owned = [], running = 0, paused = 0, onDisk = 0;
            var listed = sources.rows;
            for (var i = 0; i < listed.length; i++) {
                var g = listed[i];
                rows.push({
                    section: sourceName,
                    key: "game",
                    label: g.title,
                    type: "action",
                    display: g.partial ? "Paused · " + Format.bytes(g.partial_bytes) + " kept" : g.status,
                    choices: [],
                    detail: "",
                    row: i,
                    installed: g.installed,
                    busy: g.busy,
                    partial: g.partial,
                    pending: g.pending,
                    gameId: g.game_id,
                    image: g.image,
                    action: "Options",
                    size: g.sizeText,
                    accent: g.busy || g.pending,
                    progress: g.partial && g.disk_size > 0 ? g.partial_bytes / g.disk_size : 0
                });
                if (g.busy || g.partial) {
                    jobs.push(rows.length - 1);
                    g.busy ? running++ : paused++;
                } else
                    (g.installed ? installed : owned).push(rows.length - 1);
                if (g.installed)
                    onDisk += g.disk_size;
            }
            var busy = sources.busy ? (listed.length > 0 ? " · refreshing…" : "loading…") : "";
            var stale = sources.error !== "" ? sourceName + " could not be reached" + (sources.libraryAge ? " · listing from " + sources.libraryAge : "") : "";
            if (jobs.length > 0)
                groups.push({
                    title: "Installing",
                    meta: [running > 0 ? running + " running" : "", paused > 0 ? paused + " paused" : ""].filter(Boolean).join(" · "),
                    rows: jobs
                });
            var pending = sources.updates.length;
            if (pending > 0) {
                rows.push({
                    section: sourceName,
                    key: "all",
                    label: "Update everything",
                    type: "action",
                    display: Format.plural(pending, "update", "updates"),
                    detail: "",
                    action: "Update"
                });
                groups.push({
                    title: "Updates",
                    rows: [rows.length - 1]
                });
            }
            var where = currentSource && currentSource.games_dir ? " · " + currentSource.games_dir : "";
            var sized = onDisk > 0 ? " · " + Format.bytes(onDisk) : "";
            groups.push({
                title: "Installed",
                meta: (listed.length > 0 || !busy ? Format.plural(installed.length, "game", "games") + sized + where : "") + busy,
                rows: installed
            });
            groups.push({
                title: "Owned, not installed",
                meta: Format.plural(owned.length, "game", "games") + (sources.libraryAge && !stale ? " · refreshed " + sources.libraryAge : ""),
                warning: stale,
                rows: owned
            });
            return {
                rows: rows,
                groups: groups
            };
        }
        if (sectionId === "controller") {
            if (!learning)
                return {
                    rows: controller.rows,
                    groups: controller.groups
                };
            var bound = controller.rows;
            var listening = bound.map(function (r) {
                return r.slot !== controller.learning ? r : Object.assign({}, r, {
                    display: "Press the button on the pad…",
                    press: null,
                    hold: null
                });
            });
            return {
                rows: listening,
                groups: controller.groups
            };
        }
        if (sectionId === "themes") {
            rows.push({
                section: "Themes",
                key: "theme",
                label: "Theme",
                type: "enum",
                value: api.theme.name,
                display: api.theme.name,
                choices: themeNames(),
                detail: ""
            });
            groups.push({
                title: "Look",
                rows: [0]
            });
            return {
                rows: rows,
                groups: groups
            };
        }
        if (sectionId === "sound") {
            var outs = api.home.outputs;
            for (var k = 0; k < outs.length; k++)
                rows.push({
                    section: "Sound",
                    key: "output",
                    label: outs[k].label,
                    type: "action",
                    display: outs[k].device,
                    accent: outs[k].current,
                    tag: outs[k].current ? "In use" : "",
                    detail: "Where every sound plays: the games, Universe and the desktop.",
                    action: outs[k].current ? "" : "Use",
                    output: outs[k].id
                });
            if (rows.length === 0)
                rows.push({
                    section: "Sound",
                    key: "",
                    label: "No output found",
                    type: "info",
                    value: false,
                    detail: "PipeWire lists none"
                });
            groups.push({
                title: "Output",
                rows: rows.map(function (r, i) {
                    return i;
                })
            });
            return {
                rows: rows,
                groups: groups
            };
        }
        if (sectionId === "about") {
            rows.push({
                section: "About",
                key: "version",
                label: "Version",
                type: "static",
                display: api.universe.version() || "development build",
                detail: ""
            });
            rows.push({
                section: "About",
                key: "setup",
                label: "First-run setup",
                type: "action",
                display: "",
                detail: "What other launchers hold, your stores, a few choices.",
                action: "Run again"
            });
            rows.push({
                section: "About",
                key: "quit",
                label: "Quit Universe",
                type: "action",
                display: "",
                detail: "",
                icon: "power",
                action: "Quit"
            });
            groups.push({
                title: "Universe",
                rows: [0, 1, 2]
            });
            return {
                rows: rows,
                groups: groups
            };
        }
        return {
            rows: modulesForm.doctor,
            groups: modulesForm.doctorGroups
        };
    }

    readonly property int checksPassed: {
        var checks = modulesForm.doctor;
        return checks.filter(function (d) {
            return d.value;
        }).length;
    }

    function themeNames() {
        var looks = api.theme.themes;
        return looks.map(function (t) {
            return t.name;
        });
    }

    function leave() {
        editor.hide();
        if (menu.open)
            menu.hide();
    }

    function refresh() {
        if (listForm !== null)
            listForm.load();
        else if (sectionId === "launch")
            launch.load();
        else if (sectionId === "runners")
            runners.load();
        else if (refreshable)
            sources.load();
        else if (sectionId === "controller")
            controller.load();
        else if (sectionId === "artwork") {
            if (artwork.item)
                artwork.item.load();
        } else if (sectionId === "doctor")
            modulesForm.loadDoctor();
        else if (sectionId === "sound")
            api.home.loadOutputs();
    }

    function refreshNow() {
        if (canRemove) {
            launch.reset(cards.index) ? Sound.enter() : Sound.edge();
            return;
        }
        if (!canRefresh) {
            Sound.edge();
            return;
        }
        Sound.enter();
        if (refreshable)
            sources.refresh();
        else
            refresh();
    }

    function toggleAdvanced() {
        if (sectionId !== "launch" || !launch.hasAdvanced) {
            Sound.edge();
            return;
        }
        Sound.panel();
        launch.showAdvanced = !launch.showAdvanced;
    }

    function toggleModule() {
        var row = cards.currentRow;
        if (listForm === null || !row || (row.warning !== "" && row.value !== true)) {
            Sound.edge();
            return;
        }
        Sound.favourite(!row.value);
        listForm.toggle(cards.index);
    }

    function activate(index, row) {
        if (row.key === "advanced" && sectionId === "controller") {
            Sound.panel();
            controller.showAdvanced = !controller.showAdvanced;
            if (controller.showAdvanced)
                Qt.callLater(cards.stepInto);
            return;
        }
        if (sectionId === "modules") {
            cards.forceActiveFocus();
            openedModule = row.module;
            page.moduleRequested(row.module);
        } else if (sectionId === "sources") {
            cards.forceActiveFocus();
            openedSource = row.module;
            page.sourceRequested(row.module);
        } else if (sectionId === "launch") {
            if (row.type === "bool") {
                launch.toggle(index);
                Sound.favourite(!row.value);
            } else if (row.map === true) {
                Sound.panel();
                editor.promptPair(row.label.replace(/…$/, ""), row.fields, "", "", function (name, value) {
                    launch.setMapEntry(index, name, value) ? Sound.enter() : Sound.edge();
                });
            } else {
                Sound.panel();
                editor.edit(row, function (value) {
                    launch.setValue(index, value);
                });
            }
        } else if (sectionId === "runners") {
            cards.forceActiveFocus();
            openedRunner = row.runner;
            page.runnerRequested(row.runner);
        } else if (sectionId === "install" && row.key === "all") {
            Sound.enter();
            sources.updateAll();
        } else if (sectionId === "install") {
            Sound.panel();
            menu.show(page.gameActions(row), cards, cards.focusRect, "", function (action) {
                page.gameAction(row, action);
            });
        } else if (sectionId === "themes") {
            Sound.panel();
            editor.edit(row, function (value) {
                var looks = api.theme.themes;
                var theme = looks.filter(function (t) {
                    return t.name === value;
                })[0];
                // The switch rebuilds this tree; let the editor finish closing first.
                if (theme)
                    Qt.callLater(function () {
                        api.theme.set(theme.id);
                    });
            });
        } else if (sectionId === "sound") {
            if (row.output && row.action !== "") {
                Sound.enter();
                api.home.setOutput(row.output);
            } else
                Sound.edge();
        } else if (sectionId === "about" && row.key === "setup") {
            Sound.enter();
            page.setupRequested();
        } else if (sectionId === "about" && row.key === "quit") {
            Sound.panel();
            menu.confirm("Stay", "power", api.universe.currentSession ? "Quit and close the game" : "Quit Universe", "Quit Universe?", cards, cards.focusRect, function () {
                Qt.quit();
            });
        } else if (sectionId === "about") {
            Sound.edge();
        } else if (sectionId === "controller") {
            if (row.key === "test") {
                if (controller.setTesting(true))
                    Sound.enter();
            } else if (row.key === "walk") {
                if (controller.startWalk())
                    Sound.enter();
                else
                    Sound.edge();
            } else if (row.type === "enum" || row.type === "int") {
                Sound.panel();
                editor.edit(row, function (value) {
                    controller.setValue(index, value);
                });
            } else if (row.type === "action") {
                Sound.panel();
                menu.show(page.slotActions(row), cards, cards.focusRect, row.label, function (action) {
                    page.slotAction(row, action);
                });
            } else {
                Sound.edge();
            }
        }
    }

    function slotActions(row) {
        var out = [];
        if (!row.bound)
            out.push({
                icon: "keyboard",
                label: "Learn the button",
                action: "learn"
            });
        if (row.home)
            return row.bound ? [
                {
                    icon: "keyboard",
                    label: "Learn the button again",
                    action: "learn"
                }
            ] : out;
        out.push({
            icon: "play",
            label: "On press…",
            action: "press"
        });
        out.push({
            icon: "stop",
            label: "On hold…",
            action: "hold"
        });
        if (row.press)
            out.push({
                icon: "trash",
                label: "Clear press",
                action: "clear-press",
                danger: true
            });
        if (row.hold)
            out.push({
                icon: "trash",
                label: "Clear hold",
                action: "clear-hold",
                danger: true
            });
        if (row.bound)
            out.push({
                icon: "keyboard",
                label: "Learn the button again",
                action: "learn"
            });
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
            out.push({
                icon: Macros.icon(p.id),
                label: p.label,
                action: "preset:" + p.id
            });
        }
        out.push({
            icon: Macros.icon("keys"),
            label: "Key combo…",
            action: "keys"
        });
        out.push({
            icon: Macros.icon("command"),
            label: "Command…",
            action: "command"
        });
        return out;
    }

    function slotAction(row, action) {
        if (action === "learn") {
            if (controller.learn(row.key))
                Sound.enter();
        } else if (action === "press" || action === "hold") {
            Sound.panel();
            pendingSlot = row.key;
            pendingTrigger = action;
            menu.show(page.presetActions(action), cards, cards.focusRect, row.label + " · " + (action === "press" ? "On press" : "On hold"), function (next) {
                page.slotAction(row, next);
            });
            return;
        } else if (action.indexOf("preset:") === 0) {
            Sound.enter();
            controller.bind(pendingSlot, pendingTrigger, action.substring(7), "", "");
        } else if (action === "keys" || action === "command") {
            Sound.panel();
            var current = pendingTrigger === "hold" ? row.hold : row.press;
            var value = current && current.action === action ? current[action] : "";
            editor.prompt((action === "keys" ? "Key combo for " : "Command for ") + row.label, value, function (typed) {
                if (typed !== "")
                    controller.bind(pendingSlot, pendingTrigger, action, action === "keys" ? typed : "", action === "command" ? typed : "");
            });
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
        if (!activeFocus)
            return;
        var store = openedRunner !== "" ? runners : openedModule !== "" ? modulesForm : openedSource !== "" ? sourceList : null;
        if (!store)
            return;
        var name = openedRunner || openedModule || openedSource;
        openedRunner = openedModule = openedSource = "";
        store.load();
        var i = store.indexOf(name);
        if (i >= 0)
            cards.index = i;
    }

    onControllerOpenChanged: {
        if (controllerOpen)
            controller.suspend();
        else
            controller.resume();
    }

    onTestingChanged: artToggled("test")
    onWalkingChanged: artToggled("walk")

    // `artShown` is a binding on the property that just changed: read here it lags one step behind, so the parts are read instead.
    function artToggled(key) {
        if (art.item)
            art.item.clear();
        holdOut.stop();
        if (testing || walking) {
            Qt.callLater(tester.forceActiveFocus);
            return;
        }
        var bound = controller.rows;
        var i = bound.findIndex(function (r) {
            return r.key === key;
        });
        if (i >= 0)
            cards.index = i;
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

    function gameActions(row) {
        var out = [];
        if (row.busy)
            return [
                {
                    icon: "stop",
                    label: "Cancel " + (row.pending ? "update" : "install"),
                    action: "cancel",
                    danger: true
                }
            ];
        if (!row.installed) {
            out.push({
                icon: "download",
                label: row.partial ? "Resume" : "Install",
                action: row.partial ? "resume" : "install"
            });
            return out;
        }
        if (row.pending)
            out.push({
                icon: "refresh",
                label: "Update",
                action: "update"
            });
        if (row.gameId && api.allGames.byId(row.gameId))
            out.push({
                icon: "play",
                label: "Play",
                action: "play"
            }, {
                icon: "info",
                label: "Details",
                action: "details"
            }, {
                icon: "sliders",
                label: "Game settings",
                action: "settings"
            });
        if (row.gameId) {
            out.push({
                icon: "trash",
                label: "Uninstall…",
                action: "uninstall",
                danger: true
            });
            out.push({
                icon: "eye-off",
                label: "Remove from library…",
                action: "remove",
                danger: true
            });
        }
        return out;
    }

    function gameAction(row, action) {
        if (action === "install") {
            var g = sources.rows[row.row], parts = [];
            if (g.download_size > 0)
                parts.push(Format.bytes(g.download_size) + " to download");
            if (g.disk_size > 0)
                parts.push(Format.bytes(g.disk_size) + " on disk");
            var where = currentSource && currentSource.games_dir ? currentSource.games_dir : "";
            var free = sources.freeSpace > 0 ? Format.bytes(sources.freeSpace) + " free" + (where ? " in " + where : "") : where ? "Into " + where : "";
            dialog.ask({
                message: "Install " + row.label + "?",
                detail: [parts.length > 0 ? parts.join(" · ") : "Size not known yet", free].filter(Boolean).join("\n"),
                yes: "Install",
                no: "Not now"
            }, function (yes) {
                if (yes)
                    sources.install(row.row) !== "" ? Sound.enter() : Sound.edge();
                cards.forceActiveFocus();
            });
            return;
        } else if (action === "resume" || action === "update") {
            sources.install(row.row) !== "" ? Sound.enter() : Sound.edge();
        } else if (action === "cancel") {
            sources.cancel() ? Sound.cancel() : Sound.edge();
        } else if (action === "play") {
            cards.forceActiveFocus();
            page.launchRequested(api.allGames.byId(row.gameId));
            return;
        } else if (action === "details") {
            cards.forceActiveFocus();
            page.detailRequested(api.allGames.byId(row.gameId));
            return;
        } else if (action === "settings") {
            cards.forceActiveFocus();
            page.settingsRequested(api.allGames.byId(row.gameId));
            return;
        } else if (action === "uninstall") {
            Sound.panel();
            menu.confirm("Keep it", "trash", "Trash the install folder", "Uninstall " + row.label + "?", cards, cards.focusRect, function () {
                Sound.enter();
                sources.uninstall(row.gameId);
            });
            return;
        } else if (action === "remove") {
            Sound.panel();
            menu.confirm("Keep it", "eye-off", "Remove from the library", "Remove " + row.label + "?", cards, cards.focusRect, function () {
                Sound.enter();
                sources.remove(row.gameId);
            });
            return;
        }
        cards.forceActiveFocus();
    }

    Component.onCompleted: {
        modulesForm.load();
        sourceList.load();
        runners.load();
        sources.load();
        if (api.theme.takeLanding() === "themes")
            section = sectionIndex("themes");
        else
            refresh();
    }

    // The cards' rows rebind on the same signal; the cursor resets once they have. A section opens with its Advanced row closed.
    onSectionChanged: {
        var mainHad = !side.activeFocus;
        launch.showAdvanced = false;
        controller.showAdvanced = false;
        refresh();
        // `sectionId` still names the section left.
        if (sections[section].id === "sound")
            api.home.loadOutputs();
        else if (sections[section].id === "doctor")
            modulesForm.loadDoctor();
        Qt.callLater(function () {
            cards.reset();
            if (mainHad)
                page.focusMain();
        });
    }

    function stepSection(d) {
        var n = sections.length;
        section = (section + d + n) % n;
        Sound.tick();
    }

    Connections {
        target: page.sources
        function onMessage(text) {
            page.message(text);
        }
    }

    Connections {
        target: page.modulesForm
        function onDoctorChanged() {
            if (page.sectionId === "doctor")
                Qt.callLater(cards.reset);
        }
    }

    Connections {
        target: page.controller
        function onButtonPressed(id, slot, pressed) {
            if (!page.testing || id !== page.controller.current || !art.item)
                return;
            art.item.press(slot, pressed);
            if (slot === "east")
                pressed ? holdOut.restart() : holdOut.stop();
            var held = art.item.pressed;
            if (pressed && held.start && held.select) {
                Sound.cancel();
                page.controller.setTesting(false);
            }
        }
        function onAxisMoved(id, axis, value) {
            if (page.testing && id === page.controller.current && art.item)
                art.item.axis(axis, value);
        }
        function onUnknownPressed(id, code) {
            if (page.sectionId === "controller" && !page.learning && id === page.controller.current)
                page.message(code + " is not one of the pad's buttons yet: learn it from a row");
        }
        function onLearned(family, slot, code) {
            var bound = page.controller.rows;
            var r = bound.find(function (r) {
                return r.slot === slot;
            });
            page.message((r ? r.label : slot) + " is now " + code);
        }
        function onMessage(text) {
            page.message(text);
        }
    }

    SectionList {
        id: side

        x: page.sideMargin
        y: page.mainTop
        width: page.sideWidth
        focus: true
        sections: page.sections
        badges: page.sections.map(function (s, i) {
            return i === page.sectionIndex("install") && page.sources.updates.length > 0 ? page.sources.updates.length.toString() : "";
        })
        current: page.section
        opacity: page.artShown ? 0.35 : 1.0

        Behavior on opacity {
            Ease {
                duration: Theme.durView
            }
        }

        onRequested: function (index) {
            page.section = index;
        }
        onEntered: page.focusMain()
        onEscapedUp: page.chromeRequested()
        onCancelled: page.chromeRequested()
    }

    Loader {
        id: artwork

        x: page.mainX
        y: page.mainTop
        width: page.mainWidth
        height: page.height - y
        active: page.sectionId === "artwork"
        source: "ArtworkOverview.qml"
        visible: active

        onLoaded: item.load()

        Connections {
            target: artwork.item
            ignoreUnknownSignals: true
            function onOpenRequested(game, slot) {
                page.artworkRequested(game, slot);
            }
            function onFetchRequested(games) {
                dialog.ask({
                    message: "Fetch the missing art of " + games + (games === 1 ? " game" : " games") + "?",
                    detail: "From SteamGridDB, into each game's media folder; your picks stay on top. Stop any time from the same button.",
                    yes: "Fetch",
                    no: "Not now"
                }, function (yes) {
                    if (yes)
                        artwork.item.store.refreshAll();
                    artwork.item.forceActiveFocus();
                });
            }
            function onEscapedLeft() {
                Sound.panel();
                side.forceActiveFocus();
            }
            function onEscapedUp() {
                page.chromeRequested();
            }
            function onMessage(text) {
                page.message(text);
            }
        }
    }

    Column {
        id: above

        x: page.mainX
        y: page.mainTop
        width: page.mainWidth
        spacing: Theme.dp(32)

        Rectangle {
            id: jobBar

            width: parent.width
            height: Theme.dp(64)
            radius: Theme.dp(14)
            color: Theme.surface
            visible: page.sources.job != null

            readonly property var job: page.sources.job
            readonly property real fraction: job && job.total > 0 ? job.done / job.total : 0

            Text {
                anchors.left: parent.left
                anchors.leftMargin: Theme.dp(22)
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                anchors.verticalCenterOffset: -Theme.dp(6)
                text: jobBar.job ? jobBar.job.message + (jobBar.job.ok === true ? " ✓" : jobBar.job.ok === false && !jobBar.job.cancelled ? " ✗" : "") : ""
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
                    width: parent.width * (jobBar.job && jobBar.job.ok != null ? 1 : jobBar.fraction)
                    radius: height / 2
                    color: Theme.text

                    Behavior on width {
                        Ease {
                            duration: Theme.durQuick
                        }
                    }
                }
            }
        }

        Row {
            id: tally

            visible: page.sectionId === "doctor" && page.modulesForm.doctor.length > 0
            height: Theme.dp(30)
            spacing: Theme.dp(28)

            Repeater {
                model: {
                    var passed = page.checksPassed, failed = page.modulesForm.doctor.length - passed;
                    return [
                        {
                            count: passed,
                            text: Format.plural(passed, "check passes", "checks pass"),
                            color: "#5fd48a"
                        },
                        {
                            count: failed,
                            text: Format.plural(failed, "needs attention", "need attention"),
                            color: "#e0655a"
                        }
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
        opacity: page.artShown || page.sectionId === "artwork" ? 0.0 : 1.0
        visible: opacity > 0.01

        Behavior on opacity {
            Ease {
                duration: Theme.durView
            }
        }

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedUp: page.chromeRequested()
        onEscapedDown: Sound.edge()
        onEscapedLeft: {
            Sound.panel();
            side.forceActiveFocus();
        }

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                Sound.cancel();
                page.learning ? page.controller.cancelLearn() : side.forceActiveFocus();
            } else if (api.keys.isDetails(event)) {
                event.accepted = true;
                page.refreshNow();
            } else if (api.keys.isFilters(event) && page.listForm !== null) {
                event.accepted = true;
                page.toggleModule();
            } else if (api.keys.isFilters(event) && page.sectionId === "launch") {
                event.accepted = true;
                page.toggleAdvanced();
            }
        }
    }

    Loader {
        id: art

        x: page.mainX
        y: cards.y
        width: page.mainWidth
        height: cards.height
        active: page.artShown || opacity > 0.01
        visible: opacity > 0.01
        opacity: page.artShown ? 1.0 : 0.0

        Behavior on opacity {
            Ease {
                duration: Theme.durView
            }
        }

        sourceComponent: ControllerArt {
            family: page.controller.family
            connected: page.controller.connected
            rows: page.controller.rows
            unbound: page.controller.unboundSlots
            learningSlot: page.walking ? page.controller.learning : ""
            step: page.walking ? page.controller.walkStep : null
            onStopRequested: {
                Sound.cancel();
                page.controller.cancelWalk();
            }
            onBackRequested: page.controller.backStep() ? Sound.tick() : Sound.edge()
        }
    }

    FocusScope {
        id: tester

        anchors.fill: cards

        Keys.onPressed: function (event) {
            event.accepted = true;
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                Sound.cancel();
                if (page.walking)
                    page.controller.cancelWalk();
                else
                    page.controller.setTesting(false);
            } else if (event.key === Qt.Key_Left && page.walking) {
                page.controller.backStep() ? Sound.tick() : Sound.edge();
            }
        }
        Keys.onReleased: function (event) {
            event.accepted = true;
        }
    }

    ValueEditor {
        id: editor

        anchors.fill: parent
        cards: cards
        z: 3

        onClosed: cards.forceActiveFocus()
    }

    ActionMenu {
        id: menu

        anchors.fill: parent
        z: 4

        onDismissed: cards.forceActiveFocus()
    }

    ConfirmDialog {
        id: dialog

        anchors.fill: parent
        z: 4
    }

    // The cursor on a game without a size: ask the store for it, one at a time.
    Connections {
        target: cards
        function onIndexChanged() {
            var row = cards.currentRow;
            if (page.sectionId === "install" && row)
                page.sources.peek(row.row);
        }
    }

    Keys.onPressed: function (event) {
        if (api.keys.isPageUp(event) || api.keys.isPageDown(event))
            event.accepted = true;
    }

    Keys.onReleased: function (event) {
        if (event.isAutoRepeat || editor.open || menu.open || artShown)
            return;
        var d = api.keys.isPageUp(event) ? -1 : api.keys.isPageDown(event) ? 1 : 0;
        if (!d)
            return;
        event.accepted = true;
        page.stepSection(d);
    }
}
