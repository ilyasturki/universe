import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../ui/Controls.js" as Controls
import "../../core/Format.js" as Format
import "Details.js" as Details
import "Forms.js" as Forms
import "Sections.js" as Sections

// The console's Settings: a list of sections, each opening on two columns, its sub-sections left and their rows right.
FocusScope {
    id: page

    objectName: "settingsPage"
    property var shell: null
    property var args: ({})
    signal closeRequested
    focus: true

    readonly property var modulesForm: api.screens.modules
    readonly property var sourceList: api.screens.sourceList
    readonly property var launch: api.screens.launch
    readonly property var runners: api.screens.runners
    readonly property var components: api.screens.components
    readonly property var sources: api.screens.sources
    readonly property var listForm: sectionId === "modules" ? modulesForm : sectionId === "sources" ? sourceList : null
    readonly property bool componentsBar: sectionId !== "artwork" && components.job != null && (sectionId === "runners" || sources.job == null)
    // X on a section where it has no other use.
    readonly property bool canHideJob: componentsBar && listForm === null && sectionId !== "launch"

    readonly property var artworkOverview: api.screens.artworkOverview
    readonly property var artJob: artworkOverview.job
    readonly property bool artFetching: artJob !== null && artJob !== undefined && (artJob.ok === null || artJob.ok === undefined)

    readonly property var sections: Sections.shown(api.system).map(function (s) {
        var out = Object.assign({}, s);
        if (s.id === "updates")
            out.detail = sources.updates.length > 0 ? sources.updates.length + " pending" : "";
        else if (s.id === "runners")
            out.detail = components.pending > 0 ? components.pending + " to look at" : "";
        else if (s.id === "artwork")
            out.detail = artworkOverview.missingGames > 0 ? artworkOverview.missingGames + (artworkOverview.missingGames === 1 ? " game misses art" : " games miss art") : "";
        return out;
    })
    property int section: 1
    readonly property string sectionId: sections[section].id
    // "root": the list of sections; "section": one open, its two columns.
    property string level: "root"
    property int part: 0
    property string zone: "parts"
    property var reopen: null

    readonly property bool strip: level === "section"

    readonly property var loaders: ({
            runners: function () {
                runners.load();
            },
            launch: function () {
                launch.load();
            },
            modules: function () {
                modulesForm.load();
            },
            sources: function () {
                sourceList.load();
            },
            doctor: function () {
                modulesForm.loadDoctor();
                components.load();
            },
            sound: function () {
                api.home.loadOutputs();
            },
            performance: function () {
                api.system.reload();
            },
            artwork: function () {
                artworkOverview.load();
            }
        })
    readonly property var refreshers: ({
            runners: function () {
                runners.refresh();
            },
            artwork: function () {
                artworkOverview.load();
            },
            sound: function () {
                api.home.loadOutputs();
            },
            performance: function () {
                api.system.reload();
            },
            updates: function () {
                sources.refresh();
            },
            doctor: function () {
                modulesForm.loadDoctor();
            }
        })
    readonly property var loaded: ({})

    readonly property var hints: {
        if (level === "root")
            return [];
        var out = [];
        if (refreshers[sectionId])
            out.push({
                glyph: "Y",
                label: "Refresh"
            });
        if (sectionId === "launch" && launch.hasAdvanced)
            out.push({
                glyph: "Y",
                label: launch.showAdvanced ? "Hide advanced" : "Show advanced"
            });
        var row = rows.currentRow;
        if (listForm !== null && zone === "rows" && row && !row.heading && row.key !== "more")
            out.push({
                glyph: "X",
                label: row.value === true ? "Disable" : "Enable",
                dim: row.dim === true
            });
        else if (sectionId === "launch" && zone === "rows" && row && row.entry)
            out.push({
                glyph: "X",
                label: "Remove"
            });
        else if (canHideJob)
            out.push({
                glyph: "X",
                label: "Hide progress"
            });
        out.push({
            glyph: "B",
            label: "Back"
        });
        var label = zone !== "rows" ? "OK" : !row || row.heading || row.type === "info" || row.type === "static" || row.disabled ? "OK" : row.type === "bool" ? "Toggle" : row.type === "radio" ? "Select" : row.type === "action" ? (sectionId === "doctor" && row.component ? "Install" : sectionId === "runners" && !row.runner ? "Options" : listForm !== null ? "Open" : "Select") : "Change";
        out.push({
            glyph: "A",
            label: label
        });
        return out;
    }

    function sectionIndex(id) {
        return Math.max(0, sections.map(function (s) {
            return s.id;
        }).indexOf(Sections.aliases[id] || id));
    }

    function openSection(i) {
        section = i;
        roots.index = i;
        if (sections[i].id === "search") {
            openSearch();
            return;
        }
        if (sections[i].page) {
            shell.push(sections[i].page, {});
            return;
        }
        part = 0;
        partList.index = 0;
        zone = "parts";
        level = "section";
        partList.forceActiveFocus();
    }

    function closeSection() {
        level = "root";
        roots.forceActiveFocus();
    }

    // The sub-section holding the first row that passes, open, the cursor on that row.
    function focusWhere(test) {
        for (var p = 0; p < parts.length; p++) {
            var at = parts[p].rows.findIndex(test);
            if (at < 0)
                continue;
            part = p;
            partList.index = p;
            zone = "rows";
            rows.forceActiveFocus();
            Qt.callLater(function () {
                rows.index = at;
            });
            return true;
        }
        return false;
    }

    // A search hit on this page: its section, the row revealed (the Advanced row opened when it sits behind it).
    function land(target) {
        var id = target.page === "section" ? target.id : target.page === "controller" ? "controllers" : target.page;
        if (target.page === "controller" && target.key) {
            shell.push("pages/ControllersPage.qml", {
                key: target.key
            });
            return;
        }
        openSection(sectionIndex(id));
        if (sections[sectionIndex(id)].page)
            return;
        zone = "rows";
        rows.forceActiveFocus();
        if (target.page === "launch" && target.key) {
            var i = launch.reveal(target.key, "");
            Qt.callLater(function () {
                focusWhere(function (r) {
                    return r.form === i;
                });
            });
        } else if (target.page === "themes" && (target.id || target.key)) {
            Qt.callLater(function () {
                focusWhere(function (r) {
                    return target.id ? r.theme === target.id : r.key === target.key;
                });
            });
        }
    }

    function openSearch() {
        Sound.play("ok");
        shell.push("pages/SettingsSearchPage.qml", {});
    }

    // Read from `section`: the derived `sectionId` is still stale inside onSectionChanged.
    function loadSection() {
        var id = sections[section].id;
        if (loaded[id] || !loaders[id])
            return;
        loaded[id] = true;
        loaders[id]();
    }

    onArgsChanged: {
        if (args && args.section) {
            section = sectionIndex(args.section);
            roots.index = section;
            if (args.section !== "search")
                openSection(section);
        }
    }

    Component.onCompleted: {
        api.screens.search.sections = Sections.forSearch(api.system);
        roots.index = section;
        sources.load();
        loadSection();
    }

    onActiveFocusChanged: {
        if (!activeFocus || !reopen)
            return;
        var back = reopen;
        reopen = null;
        back.form.load();
        focusWhere(function (r) {
            return r[back.field] === back.id;
        });
    }

    readonly property var content: {
        if (sectionId === "search")
            return [];
        if (sectionId === "runners")
            return Forms.grouped(runners.groups, runners.rows, function (run, i, g) {
                return {
                    label: run.label,
                    type: "action",
                    action: run.runner ? "runner" : "component",
                    runner: run.runner || "",
                    component: run.component,
                    icon: run.icon,
                    iconSlot: true,
                    display: run.tag && run.tag !== "Updated" ? run.tag : run.display,
                    detail: run.detail,
                    dim: g.off === true
                };
            });
        if (sectionId === "launch")
            return Forms.grouped(launch.groups, launch.rows, function (r, i) {
                return Object.assign(Details.withDetail(r, ""), {
                    form: i
                });
            });
        if (listForm !== null)
            return Forms.grouped(listForm.groups, listForm.rows, function (m, i) {
                if (m.key === "more")
                    return {
                        label: m.label,
                        type: "action",
                        key: "more",
                        addons: m.addons,
                        detail: m.detail,
                        form: i
                    };
                return {
                    label: m.label,
                    type: "action",
                    action: "module",
                    module: m.module,
                    value: m.value,
                    display: m.display,
                    switch: true,
                    warning: m.warning,
                    detail: m.warning ? m.detail : (m.tag ? m.tag + " · " : "") + Details.enabledSentence(m.label, m.source),
                    form: i,
                    dim: m.warning !== "" && m.value !== true
                };
            });
        if (sectionId === "updates") {
            var n = sources.updates.length;
            if (n === 0)
                return [
                    {
                        label: sources.busy ? "Checking…" : "Everything is up to date",
                        type: "info",
                        value: true,
                        display: "",
                        detail: ""
                    }
                ];
            return [
                {
                    label: "Update everything",
                    type: "action",
                    display: n + " pending",
                    action: "update-all",
                    detail: ""
                }
            ].concat(sources.updates.map(function (u, i) {
                return {
                    label: u.title,
                    type: "action",
                    display: (u.version ? u.version + " · " : "") + (u.date || ""),
                    action: "update",
                    row: i,
                    detail: ""
                };
            }));
        }
        if (sectionId === "doctor") {
            var checks = Forms.grouped(modulesForm.doctorGroups, modulesForm.doctor, function (c) {
                var ok = c.value === true;
                var fixable = !ok && c.component !== "";
                return {
                    label: c.label,
                    path: c.path || "",
                    type: fixable ? "action" : "info",
                    component: fixable ? c.component : "",
                    value: ok,
                    display: fixable ? "Install" : ok ? c.detail || "" : "",
                    detail: ok ? "" : c.detail || "",
                    fix: ok ? "" : c.fix || ""
                };
            });
            return checks.length > 0 ? checks : [
                {
                    label: "No checks yet",
                    type: "info",
                    value: true,
                    display: "",
                    detail: ""
                }
            ];
        }
        if (sectionId === "artwork") {
            var job = artJob;
            var missing = artworkOverview.rows.filter(function (r) {
                return r.slots.some(function (sl) {
                    return sl.kind === "missing";
                });
            });
            return [
                {
                    label: artFetching ? (job.cancelled ? "Stopping…" : "Stop fetching") : "Fetch missing art",
                    type: "action",
                    action: "artwork-fetch",
                    icon: "download",
                    display: artFetching ? (job.total > 0 ? (job.done + 1) + " of " + job.total : "") : artworkOverview.missingGames === 0 ? "Nothing missing" : Format.plural(artworkOverview.missingGames, "game", "games"),
                    disabled: !artFetching && artworkOverview.missingGames === 0,
                    detail: "Asks the stores, GOG GamesDB and libretro (and SteamGridDB with your key) for every game's missing covers, banners, backgrounds and logos."
                }
            ].concat(missing.length > 0 ? [
                {
                    heading: true,
                    part: true,
                    label: "Missing Art",
                    display: ""
                }
            ] : [], missing.map(function (r) {
                return {
                    label: r.title,
                    type: "action",
                    action: "artwork-game",
                    gameId: r.id,
                    display: "Missing: " + r.slots.filter(function (sl) {
                        return sl.kind === "missing";
                    }).map(function (sl) {
                        return sl.label;
                    }).join(", "),
                    detail: ""
                };
            }), [
                {
                    heading: true,
                    part: true,
                    label: "Every Game",
                    display: ""
                }
            ], artworkOverview.rows.map(function (r) {
                var mine = r.slots.filter(function (sl) {
                    return sl.kind !== "missing";
                }).length;
                return {
                    label: r.title,
                    type: "action",
                    action: "artwork-game",
                    gameId: r.id,
                    display: mine + " of " + r.slots.length,
                    detail: ""
                };
            }));
        }
        if (sectionId === "performance")
            return api.system.controls.map(function (c) {
                var toggle = c.kind === "toggle";
                return {
                    key: c.id,
                    label: c.label,
                    type: toggle ? "bool" : "enum",
                    value: toggle ? c.value === "on" : Controls.label(c),
                    display: Controls.label(c),
                    choices: toggle ? [] : Controls.labels(c),
                    action: "system",
                    control: c,
                    detail: c.detail
                };
            });
        if (sectionId === "sound") {
            var outs = api.home.outputs.map(function (o) {
                return {
                    label: o.label,
                    type: "radio",
                    value: o.current,
                    path: o.device,
                    action: "output",
                    output: o.id,
                    detail: ""
                };
            });
            return outs.length > 0 ? outs : [
                {
                    label: "No output found",
                    type: "info",
                    value: false,
                    display: "PipeWire lists none",
                    detail: ""
                }
            ];
        }
        var looks = api.theme.themes;
        if (sectionId === "themes")
            return looks.map(function (t) {
                return {
                    label: t.name,
                    type: "radio",
                    value: t.id === api.theme.current,
                    swatch: t.ground,
                    swatchAccent: t.accent,
                    action: "theme",
                    theme: t.id,
                    detail: t.detail || ""
                };
            }).concat([
                {
                    label: api.theme.affiliation,
                    key: "affiliation",
                    type: "info",
                    wraps: true,
                    display: "",
                    detail: api.theme.trademarks
                },
                {
                    heading: true,
                    part: true,
                    label: "Font",
                    display: ""
                },
                {
                    label: "Font file",
                    key: "font_file",
                    type: "path",
                    action: "font",
                    value: api.theme.fontPath,
                    display: api.theme.fontPath ? api.theme.fontPath.split("/").pop() : "Bundled (Source Sans 3)",
                    detail: "A .ttf you own, such as the console's SST; applies at once."
                },
                {
                    heading: true,
                    part: true,
                    label: "Sounds",
                    display: ""
                },
                {
                    label: "Sound folder",
                    key: "sound_dir",
                    type: "path",
                    action: "sounds",
                    value: api.theme.soundsPath,
                    display: api.theme.soundsPath ? api.theme.soundsPath.split("/").pop() + " · " + Object.keys(api.theme.soundFiles).length + " sounds" : "Bundled",
                    detail: "WAVs named ok.wav, tick.wav, boot.wav… replace the bundled ones."
                },
                {
                    heading: true,
                    part: true,
                    label: "Startup",
                    display: ""
                },
                {
                    label: "Startup animation",
                    key: "boot_intro",
                    type: "bool",
                    action: "boot",
                    value: api.theme.bootIntro,
                    display: "",
                    detail: "The Universe mark when the launcher starts full screen; any button skips it."
                }
            ]);
        if (sectionId === "about")
            return [
                {
                    label: "Universe",
                    type: "static",
                    display: api.universe.version() || "development build",
                    detail: ""
                },
                {
                    label: "Look",
                    type: "static",
                    display: api.theme.name,
                    detail: ""
                },
                {
                    label: "Library",
                    type: "static",
                    display: api.allGames.count + (api.allGames.count === 1 ? " game" : " games"),
                    detail: ""
                },
                {
                    label: "Changelog",
                    key: "changelog",
                    type: "action",
                    action: "Open",
                    display: "",
                    detail: "What changed in each version, newest first."
                },
                {
                    heading: true,
                    part: true,
                    label: "Setup and Power",
                    display: ""
                },
                {
                    label: "First-run setup",
                    key: "setup",
                    type: "action",
                    action: "Run again",
                    display: "",
                    detail: "What other launchers hold, your stores, a few choices."
                },
                {
                    label: "Power",
                    key: "power",
                    type: "action",
                    icon: "power",
                    display: "",
                    detail: api.system.steam ? "Back to Steam, whose menu has the power options." : (api.system.session ? "Log out" : "Quit Universe") + ", or rest, restart or turn off the machine."
                }
            ];
        return [];
    }

    readonly property var parts: Forms.parts(content, sections[section].first || sections[section].label)
    readonly property var partRows: parts[Math.max(0, Math.min(part, parts.length - 1))].rows

    function activate(index, row) {
        if (sectionId === "about" && row.key === "changelog") {
            Sound.play("ok");
            shell.push("pages/ChangelogPage.qml", {});
        } else if (sectionId === "about" && row.key === "setup") {
            Sound.play("ok");
            shell.push("pages/OnboardingPage.qml", {});
        } else if (sectionId === "about" && row.key === "power") {
            Sound.play("ok");
            shell.askPower();
        } else if (sectionId === "launch" && row.map === true) {
            Sound.play("ok");
            Forms.addEntry(shell, row, function (name, value) {
                launch.setMapEntry(row.form, name, value);
            });
        } else if (sectionId === "runners" && !row.runner) {
            shell.componentOptions(row.component, row.label);
        } else if (sectionId === "doctor" && row.component) {
            Sound.play("ok");
            var ask = components.question(row.component);
            shell.dialogAsk({
                message: ask ? ask.message : "Install " + row.label + "?",
                detail: ask ? ask.detail : "",
                buttons: ["Not Now", "Install"]
            }, function (i) {
                if (i === 1)
                    Sound.play(components.installById(row.component) ? "ok" : "edge");
            });
        } else if (sectionId === "runners") {
            Sound.play("ok");
            reopen = {
                form: runners,
                field: "runner",
                id: row.runner
            };
            shell.push("pages/FormPage.qml", {
                runner: row.runner
            });
        } else if (sectionId === "launch") {
            if (row.type === "bool") {
                launch.toggle(row.form);
                Sound.play("select");
            } else {
                rows.edit(row, function (value) {
                    launch.setValue(row.form, value);
                });
            }
        } else if (row.key === "more") {
            shell.addonsMenu(row.addons);
        } else if (sectionId === "modules") {
            Sound.play("ok");
            reopen = {
                form: modulesForm,
                field: "module",
                id: row.module
            };
            shell.push("pages/FormPage.qml", {
                module: row.module
            });
        } else if (sectionId === "sources") {
            Sound.play("ok");
            reopen = {
                form: sourceList,
                field: "module",
                id: row.module
            };
            shell.push("pages/FormPage.qml", {
                source: row.module
            });
        } else if (sectionId === "updates") {
            Sound.play("ok");
            if (row.action === "update-all")
                sources.updateAll();
            else
                sources.update(row.row);
        } else if (row.action === "artwork-fetch") {
            if (artFetching)
                artworkOverview.cancelRefresh() ? Sound.play("back") : Sound.play("edge");
            else {
                Sound.play("ok");
                artworkOverview.refreshAll();
            }
        } else if (row.action === "artwork-game") {
            Sound.play("ok");
            shell.push("pages/ArtworkPage.qml", {
                gameId: row.gameId
            });
        } else if (row.action === "system") {
            if (row.type === "bool") {
                Sound.play("select");
                api.system.set(row.key, row.value ? "off" : "on");
            } else {
                rows.edit(row, function (label) {
                    var at = row.choices.indexOf(label);
                    if (at >= 0)
                        api.system.set(row.key, Controls.values(row.control)[at]);
                });
            }
        } else if (row.action === "output") {
            Sound.play(row.value ? "edge" : "select");
            if (!row.value)
                api.home.setOutput(row.output);
        } else if (row.action === "theme") {
            Sound.play("select");
            var id = row.theme;
            // The new look replaces this tree: let the press finish first.
            Qt.callLater(function () {
                api.theme.set(id);
            });
        } else if (row.action === "font") {
            rows.edit(row, function (path) {
                api.theme.fontPath = path;
            });
        } else if (row.action === "sounds") {
            rows.edit(row, function (path) {
                api.theme.soundsPath = path;
            });
        } else if (row.action === "boot") {
            Sound.play("select");
            api.theme.bootIntro = !row.value;
        }
    }

    function toggleModule() {
        var row = rows.currentRow;
        if (listForm === null || zone !== "rows" || !row || row.heading || row.dim === true || row.key === "more") {
            Sound.play("edge");
            return;
        }
        Sound.play("select");
        listForm.toggle(row.form);
    }

    function refreshNow() {
        var f = refreshers[sectionId];
        if (f) {
            Sound.play("ok");
            f();
        } else if (sectionId === "launch" && launch.hasAdvanced) {
            Sound.play("select");
            launch.showAdvanced = !launch.showAdvanced;
        } else
            Sound.play("edge");
    }

    // X on the Launch section: a variable's row goes out of its map.
    function removeEntry() {
        var row = rows.currentRow;
        if (sectionId !== "launch" || zone !== "rows" || !row || !row.entry || !launch.resettable(row)) {
            Sound.play("edge");
            return;
        }
        Sound.play(launch.reset(row.form) ? "select" : "edge");
    }

    // A section opens with its Advanced row closed.
    onSectionChanged: {
        launch.showAdvanced = false;
        loadSection();
    }
    onPartChanged: Qt.callLater(rows.reset)

    Connections {
        target: page.sources
        function onMessage(text) {
            page.shell.showToast(text);
        }
    }

    Connections {
        target: page.artworkOverview
        function onMessage(text) {
            page.shell.showToast(text);
        }
    }

    Component.onDestruction: artworkOverview.unload()

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (page.level === "root")
            return;
        if (api.keys.isCancel(event)) {
            event.accepted = true;
            Sound.play("back");
            if (page.zone === "rows") {
                page.zone = "parts";
                partList.forceActiveFocus();
            } else
                page.closeSection();
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            page.refreshNow();
        } else if (api.keys.isDetails(event) && page.listForm !== null) {
            event.accepted = true;
            page.toggleModule();
        } else if (api.keys.isDetails(event) && page.sectionId === "launch") {
            event.accepted = true;
            page.removeEntry();
        } else if (api.keys.isDetails(event) && page.canHideJob) {
            event.accepted = true;
            page.hideJob();
        }
    }

    function hideJob() {
        Sound.play("select");
        components.hideJob();
    }

    Backdrop {
        anchors.fill: parent
    }

    Item {
        id: rootLayer

        anchors.fill: parent
        opacity: page.level === "root" ? 1.0 : 0.0
        visible: opacity > 0.01
        transform: Translate {
            x: page.level === "root" ? 0 : -Theme.dp(60)

            Behavior on x {
                NumberAnimation {
                    duration: Theme.durPage
                    easing.type: Easing.OutCubic
                }
            }
        }

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }

        PageTitle {
            id: rootTitle
            anchors.left: parent.left
            anchors.right: parent.right
            title: "Settings"
        }

        RootList {
            id: roots

            x: Theme.dp(300)
            y: rootTitle.height + Theme.dp(4)
            width: Math.min(Theme.dp(1320), parent.width - x - Theme.dp(Theme.columnRight + 128))
            height: parent.height - y
            sections: page.sections
            focus: page.level === "root"

            onMoved: function (i) {
                page.section = i;
            }
            onChosen: function (i) {
                Sound.play("ok");
                page.openSection(i);
            }
        }
    }

    Item {
        id: sectionLayer

        anchors.fill: parent
        opacity: page.level === "section" ? 1.0 : 0.0
        visible: opacity > 0.01
        transform: Translate {
            x: page.level === "section" ? 0 : Theme.dp(60)

            Behavior on x {
                NumberAnimation {
                    duration: Theme.durPage
                    easing.type: Easing.OutCubic
                }
            }
        }

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }

        PageTitle {
            id: sectionTitle
            anchors.left: parent.left
            anchors.right: parent.right
            title: page.sections[page.section].label
        }

        SectionList {
            id: partList

            x: Theme.dp(172)
            y: sectionTitle.height + Theme.dp(10)
            width: Theme.dp(430)
            height: parent.height - y - Theme.dp(96)
            sections: page.parts
            focus: page.level === "section" && page.zone === "parts"

            onActivated: function (i) {
                page.part = i;
            }
            onPointed: page.zone = "parts"
            onEscapedRight: {
                page.zone = "rows";
                rows.forceActiveFocus();
            }
        }

        JobLine {
            id: jobLine
            x: rows.x
            y: sectionTitle.height + Theme.dp(24)
            width: rows.width
            job: page.sectionId === "artwork" ? page.artJob : page.componentsBar ? page.components.job : page.sources.job
            closable: page.componentsBar
            onCloseRequested: page.hideJob()
        }

        SettingsRows {
            id: rows

            shell: page.shell
            x: Theme.dp(672)
            y: sectionTitle.height + Theme.dp(24) + (jobLine.visible ? jobLine.height : 0)
            width: parent.width - x - Theme.dp(Theme.columnRight + 16)
            height: parent.height - y - Theme.dp(96)
            model: page.partRows
            focus: page.level === "section" && page.zone === "rows"

            onActivated: function (index, row) {
                page.activate(index, row);
            }
            onEscapedDown: Sound.play("edge")
            onPointed: page.zone = "rows"
            onEscapedLeft: {
                page.zone = "parts";
                partList.forceActiveFocus();
            }
        }
    }
}
