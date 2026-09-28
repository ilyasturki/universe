import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../ui/Controls.js" as Controls
import "Details.js" as Details
import "Forms.js" as Forms

FocusScope {
    id: page

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
    readonly property bool componentsBar: components.job != null && (sectionId === "components" || sources.job == null)
    // X on a section where it has no other use.
    readonly property bool canHideJob: componentsBar && listForm === null && sectionId !== "launch"

    // Search first, then the sections in four named groups.
    readonly property var sections: [
        {
            id: "search",
            label: "Search",
            group: 0,
            detail: "Every setting, by name, purpose or value"
        },
        {
            id: "launch",
            label: "Launch",
            group: 1,
            groupLabel: "Play"
        },
        {
            id: "runners",
            label: "Runners",
            group: 1,
            groupLabel: "Play"
        },
        {
            id: "components",
            label: "Components",
            detail: components.pending > 0 ? components.pending + " to look at" : "",
            group: 1,
            groupLabel: "Play"
        },
        {
            id: "controllers",
            label: "Controllers",
            group: 1,
            groupLabel: "Play"
        },
        {
            id: "sources",
            label: "Sources",
            group: 2,
            groupLabel: "Store"
        },
        {
            id: "updates",
            label: "Updates",
            detail: sources.updates.length > 0 ? sources.updates.length + " pending" : "",
            group: 2,
            groupLabel: "Store"
        },
        {
            id: "modules",
            label: "Modules",
            group: 3,
            groupLabel: "Extras"
        },
        {
            id: "themes",
            label: "Themes",
            group: 3,
            groupLabel: "Extras"
        },
        {
            id: "sound",
            label: "Sound",
            group: 4,
            groupLabel: "System"
        },
        {
            id: "performance",
            label: "Performance",
            group: 4,
            groupLabel: "System",
            detail: "Brightness, refresh, power limit, GPU clock and fan"
        },
        {
            id: "doctor",
            label: "Doctor",
            group: 4,
            groupLabel: "System"
        },
        {
            id: "about",
            label: "About",
            group: 4,
            groupLabel: "System"
        }
    ].filter(function (s) {
        // Steam's Game Mode keeps the sound; Performance needs a control this machine has.
        return !(s.id === "sound" && api.system.steam) && !(s.id === "performance" && api.system.controls.length === 0);
    })
    property int section: 1
    readonly property string sectionId: sections[section].id
    property string zone: "list"
    property var reopen: null

    readonly property var loaders: ({
            runners: function () {
                runners.load();
            },
            components: function () {
                components.load();
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
            },
            sound: function () {
                api.home.loadOutputs();
            },
            performance: function () {
                api.system.reload();
            }
        })
    readonly property var refreshers: ({
            components: function () {
                components.refresh();
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
        if (listForm !== null && zone === "rows" && row && !row.heading)
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
        var label = zone !== "rows" ? "OK" : !row || row.heading || row.type === "info" || row.type === "static" || row.disabled ? "OK" : row.type === "bool" ? "Toggle" : row.type === "radio" ? "Select" : row.type === "action" ? (listForm !== null ? "Open" : "Select") : "Change";
        out.push({
            glyph: "A",
            label: label
        });
        return out;
    }

    function sectionIndex(id) {
        return Math.max(0, sections.map(function (s) {
            return s.id;
        }).indexOf(id));
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
        section = sectionIndex(id);
        list.index = section;
        zone = "rows";
        rows.forceActiveFocus();
        if (target.page === "launch" && target.key) {
            var i = launch.reveal(target.key, "");
            Qt.callLater(function () {
                var at = Forms.rowOf(content, i);
                if (at >= 0)
                    rows.index = at;
            });
        } else if (target.page === "themes" && target.id) {
            Qt.callLater(function () {
                var at = content.findIndex(function (r) {
                    return r.theme === target.id;
                });
                if (at >= 0)
                    rows.index = at;
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
            list.index = section;
        }
    }

    Component.onCompleted: {
        api.screens.search.sections = sections.map(function (s) {
            return {
                id: s.id,
                label: s.label
            };
        });
        list.index = section;
        sources.load();
        loadSection();
    }

    onActiveFocusChanged: {
        if (!activeFocus || !reopen)
            return;
        reopen.form.load();
        var i = content.findIndex(function (r) {
            return r[reopen.field] === reopen.id;
        });
        reopen = null;
        if (i >= 0)
            rows.index = i;
    }

    readonly property var content: {
        if (sectionId === "search")
            return [
                {
                    label: "Search every setting",
                    type: "action",
                    action: "search",
                    display: "",
                    icon: "search",
                    detail: "Every page, every runner, module and game: by name, by what a setting does, by its value. A game's title narrows to it."
                }
            ];
        if (sectionId === "runners")
            return Forms.grouped(runners.groups, runners.rows, function (run, i, g) {
                return {
                    label: run.label,
                    type: "action",
                    action: "runner",
                    runner: run.runner,
                    icon: run.icon,
                    iconSlot: true,
                    display: run.display,
                    detail: "",
                    dim: g.off === true
                };
            });
        if (sectionId === "components")
            return Forms.grouped(components.groups, components.rows, function (c, i, g) {
                return {
                    label: c.label,
                    type: "action",
                    action: "component",
                    form: i,
                    icon: c.icon,
                    iconSlot: true,
                    display: c.tag && c.tag !== "Updated" ? c.tag : c.display,
                    detail: c.detail,
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
                return {
                    label: m.label,
                    type: "action",
                    action: "module",
                    module: m.module,
                    value: m.value,
                    display: m.display,
                    switch: true,
                    warning: m.warning,
                    detail: m.warning ? m.detail : Details.enabledSentence(m.label, m.source),
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
                    action: fixable ? "Install" : "",
                    component: fixable ? c.component : "",
                    value: ok,
                    display: ok ? c.detail || "" : "",
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
        if (sectionId === "controllers")
            return [
                {
                    label: "Controllers",
                    type: "action",
                    action: "controllers",
                    display: "",
                    detail: ""
                }
            ];
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
                    action: "theme",
                    theme: t.id,
                    detail: t.detail || ""
                };
            }).concat([
                {
                    heading: true,
                    label: "Font",
                    display: ""
                },
                {
                    label: "Font file",
                    key: "font_file",
                    type: "path",
                    action: "font",
                    value: api.theme.fontPath,
                    display: api.theme.fontPath ? api.theme.fontPath.split("/").pop() : "Bundled (BIZ UDPGothic)",
                    detail: "A .ttf you own, such as the Switch's own; applies at once."
                },
                {
                    heading: true,
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
                    detail: "WAVs named ok.wav, tick.wav… replace the bundled ones."
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
                    label: "First-run setup",
                    key: "setup",
                    type: "action",
                    action: "Run again",
                    display: "",
                    detail: "What other launchers hold, your stores, a few choices."
                }
            ];
        return [];
    }

    function activate(index, row) {
        if (sectionId === "search") {
            openSearch();
        } else if (sectionId === "about" && row.key === "setup") {
            Sound.play("ok");
            shell.push("pages/OnboardingPage.qml", {});
        } else if (sectionId === "launch" && row.map === true) {
            Sound.play("ok");
            Forms.addEntry(shell, row, function (name, value) {
                launch.setMapEntry(row.form, name, value);
            });
        } else if (sectionId === "components") {
            Sound.play("ok");
            componentMenu(row.form, components.actions(row.form), row.label);
        } else if (sectionId === "doctor" && row.component) {
            Sound.play("ok");
            var ask = components.question(row.component);
            shell.dialogAsk({
                message: ask ? ask.message : "Install " + row.label + "?",
                detail: ask ? ask.detail : "",
                buttons: ["Not now", "Install"]
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
        } else if (sectionId === "controllers") {
            Sound.play("ok");
            shell.push("pages/ControllersPage.qml", {});
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
            // Reprise replaces this tree: let the press finish first.
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
        }
    }

    function componentMenu(index, items, title) {
        if (items.length === 0) {
            Sound.play("edge");
            return;
        }
        shell.menu(title, items.map(function (i) {
            return {
                label: i.label,
                act: i.action
            };
        }), function (action) {
            page.componentAction(index, action);
        });
    }
    function componentAction(index, action) {
        if (action === "versions") {
            componentMenu(index, components.versionActions(index), "Another version");
            return;
        }
        var ask = components.confirm(index, action);
        if (!ask) {
            Sound.play(components.act(index, action) ? "ok" : "edge");
            return;
        }
        shell.dialogAsk({
            message: ask.message,
            detail: ask.detail,
            buttons: [ask.no, ask.yes],
            danger: action === "rollback" || action === "uninstall" || action.indexOf("remove:") === 0 ? 1 : -1
        }, function (i) {
            if (i === 1)
                Sound.play(components.act(index, action) ? "ok" : "edge");
        });
    }
    function toggleModule() {
        var row = rows.currentRow;
        if (listForm === null || zone !== "rows" || !row || row.heading || row.dim === true) {
            Sound.play("edge");
            return;
        }
        Sound.play(row.value ? "deselect" : "select");
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
        Qt.callLater(rows.reset);
    }

    Connections {
        target: page.sources
        function onMessage(text) {
            page.shell.showToast(text);
        }
    }
    Connections {
        target: page.components
        function onRunnerRequested(id) {
            page.shell.push("pages/FormPage.qml", {
                runner: id
            });
        }
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isCancel(event) && page.zone === "rows") {
            event.accepted = true;
            Sound.play("back");
            page.zone = "list";
            list.forceActiveFocus();
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

    PageHeader {
        id: header
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        icon: "settings"
        title: "System Settings"
    }

    SectionList {
        id: list

        x: Theme.dp(130)
        y: header.height + Theme.dp(20)
        width: Theme.dp(470)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight)
        sections: page.sections
        focus: page.zone === "list"

        onActivated: function (i) {
            page.section = i;
        }
        onPointed: page.zone = "list"
        onEscapedRight: {
            if (page.sectionId === "search") {
                page.openSearch();
                return;
            }
            page.zone = "rows";
            rows.forceActiveFocus();
        }
    }

    Rectangle {
        x: Theme.dp(639)
        y: header.height + Theme.dp(20)
        width: 1
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        color: Theme.hairline
    }

    JobLine {
        id: jobLine
        x: Theme.dp(705)
        y: header.height + Theme.dp(40)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        job: page.componentsBar ? page.components.job : page.sources.job
        closable: page.componentsBar
        onCloseRequested: page.hideJob()
    }

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(705)
        y: header.height + Theme.dp(64) + jobLine.height
        width: parent.width - x - Theme.dp(Theme.columnRight)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        model: page.content
        focus: page.zone === "rows"

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedDown: Sound.play("edge")
        onPointed: page.zone = "rows"
        onEscapedLeft: {
            page.zone = "list";
            list.forceActiveFocus();
        }
    }
}
