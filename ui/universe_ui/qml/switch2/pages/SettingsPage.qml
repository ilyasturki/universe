import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../ui/Controls.js" as Controls
import "Details.js" as Details
import "Forms.js" as Forms

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
    readonly property bool componentsBar: components.job != null && (sectionId === "runners" || sources.job == null)
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
            id: "network",
            label: "Internet",
            group: 4,
            groupLabel: "System",
            detail: api.screens.network.link === "wired" ? "Wired" : api.screens.network.ssid
        },
        {
            id: "sound",
            label: "Sound",
            group: 4,
            groupLabel: "System"
        },
        {
            id: "bluetooth",
            label: "Bluetooth",
            group: 4,
            groupLabel: "System",
            detail: "Headphones, speakers and keyboards"
        },
        {
            id: "performance",
            label: "Performance",
            group: 4,
            groupLabel: "System",
            detail: "Brightness, refresh, power limit, GPU clock and fan"
        },
        {
            id: "data",
            label: "Data Management",
            group: 4,
            groupLabel: "System",
            detail: "Saves, prefixes and every folder Universe writes to"
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
        // Steam's Game Mode keeps the sound; Performance needs a control this machine has; Internet and Bluetooth, what the machine has.
        return !(s.id === "sound" && api.system.steam) && !(s.id === "performance" && api.system.controls.length === 0) && !(s.id === "network" && !api.system.network) && !(s.id === "bluetooth" && !api.system.bluetooth);
    })
    property int section: 1
    readonly property string sectionId: sections[Math.min(section, sections.length - 1)].id
    // The section shown, by id: its index moves as Internet and Bluetooth come and go.
    property string shownId: ""
    property string zone: "list"
    property var reopen: null
    // A section asked for before the machine said it has it: Internet and Bluetooth appear once NetworkManager and BlueZ answer.
    property string pending: ""

    readonly property var network: api.screens.network
    readonly property var bluetooth: api.screens.bluetooth
    readonly property bool padsOnly: args && args.pads === true
    readonly property bool onTop: shell !== null && shell.topPage === page
    readonly property string searching: onTop && (sectionId === "network" || sectionId === "bluetooth") ? sectionId : ""
    property string searched: ""

    onSearchingChanged: {
        if (searched !== "")
            radio(searched).close();
        searched = searching;
        if (searched !== "")
            radio(searched).open();
    }
    Component.onDestruction: {
        if (searched !== "")
            radio(searched).close();
    }

    function radio(id) {
        return id === "network" ? network : bluetooth;
    }

    function publishSections() {
        api.screens.search.sections = sections.map(function (s) {
            return {
                id: s.id,
                label: s.label
            };
        });
    }

    onSectionsChanged: {
        publishSections();
        var ids = sections.map(function (s) {
            return s.id;
        });
        var want = shownId;
        if (pending !== "" && ids.indexOf(pending) >= 0) {
            want = pending;
            pending = "";
        }
        var at = ids.indexOf(want);
        if (at < 0)
            at = Math.min(section, sections.length - 1);
        if (at !== section)
            section = at;
        list.index = section;
    }

    readonly property var loaders: ({
            runners: function () {
                runners.load();
            },
            launch: function () {
                launch.load();
            },
            themes: function () {
                api.theme.rescan();
                api.screens.addons.load();
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
            },
            data: function () {
                api.screens.storage.loadFree();
            }
        })
    readonly property var refreshers: ({
            runners: function () {
                runners.refresh();
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
        else if (sectionId === "network" && zone === "rows" && row && row.key === "network" && row.saved)
            out.push({
                glyph: "X",
                label: "Forget"
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

    // Sections folded into another: a landing on one opens the other.
    readonly property var aliases: ({
            components: "runners"
        })

    function sectionIndex(id) {
        return Math.max(0, sections.map(function (s) {
            return s.id;
        }).indexOf(aliases[id] || id));
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
        } else if (target.page === "themes" && (target.id || target.key)) {
            Qt.callLater(function () {
                var at = content.findIndex(function (r) {
                    return target.id ? r.theme === target.id : r.key === target.key;
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
        var id = sections[Math.min(section, sections.length - 1)].id;
        if (loaded[id] || !loaders[id])
            return;
        loaded[id] = true;
        loaders[id]();
    }

    onArgsChanged: {
        if (args && args.section) {
            pending = sections.some(function (s) {
                return s.id === args.section;
            }) ? "" : args.section;
            section = sectionIndex(args.section);
            list.index = section;
        }
    }

    Component.onCompleted: {
        publishSections();
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
        if (sectionId === "data")
            return [
                {
                    key: "storage",
                    label: "Data Management",
                    type: "action",
                    display: api.screens.storage.free !== "" ? api.screens.storage.free + " free" : "",
                    detail: "Each folder's size and free space, the games by size, and what no game uses any more. Save backups are set in Launch › Saves."
                }
            ];
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
        if (sectionId === "network")
            return networkRows();
        if (sectionId === "bluetooth")
            return bluetoothRows();
        var looks = api.theme.themes;
        if (sectionId === "themes")
            return looks.map(function (t) {
                return {
                    label: t.name,
                    type: "radio",
                    value: t.id === api.theme.current,
                    swatch: t.ground,
                    swatchAccent: t.screenshot ? "" : t.accent,
                    swatchImage: t.screenshot,
                    action: "theme",
                    theme: t.id,
                    dim: t.unavailable !== "",
                    detail: t.unavailable ? "Can't be used: " + t.unavailable : t.detail || ""
                };
            }).concat([
                {
                    label: "Get more…",
                    type: "action",
                    key: "more",
                    addons: "theme",
                    detail: "Add-ons: themes others made"
                },
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
                    detail: "WAVs named ok.wav, tick.wav, boot.wav… replace the bundled ones."
                },
                {
                    heading: true,
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

    function networkRows() {
        var link = network.link;
        var internet = ({
                full: "Connected to the internet.",
                limited: "Connected, without access to the internet.",
                portal: "The network wants a sign-in from a web browser first.",
                none: "No internet."
            })[network.connectivity] || "";
        var out = [
            {
                key: "check",
                label: "Test Connection",
                type: "action",
                display: network.checking ? "Testing…" : "",
                detail: "Checks that the internet answers through this connection."
            },
            {
                key: "status",
                label: "Connection Status",
                type: "static",
                display: link === "wired" ? "Wired" : link === "wifi" ? network.ssid : "Not connected",
                detail: link !== "" ? internet : ""
            },
            {
                key: "wifi",
                label: "Wi-Fi",
                type: "bool",
                value: network.enabled,
                display: "",
                detail: ""
            },
            {
                heading: true,
                label: "Internet Settings",
                display: network.enabled ? "Networks found" : ""
            }
        ];
        var found = network.networks.map(function (n) {
            return Object.assign({}, n, {
                key: "network",
                label: n.ssid,
                type: "action",
                icon: "wifi",
                display: network.connecting === n.ssid ? "Connecting…" : n.active ? "Connected" : n.saved ? "Saved" : "",
                secondary: !n.joinable ? "Set up from a desktop" : n.secured ? "Secured" : "Open",
                dim: !n.joinable
            });
        });
        return out.concat(found.length > 0 ? found : [
            {
                key: "none",
                label: network.enabled ? "Searching for networks…" : "Wi-Fi is off",
                type: "static",
                display: "",
                detail: ""
            }
        ]);
    }

    function deviceRow(d) {
        return {
            key: "device",
            label: d.name,
            type: "action",
            icon: d.kind === "pad" ? "gamepad" : "bluetooth",
            address: d.address,
            paired: d.paired,
            connected: d.connected,
            kind: d.kind,
            battery: d.battery,
            display: bluetooth.pairing === d.address ? "Pairing…" : d.connected ? "Connected" : d.paired ? "Not Connected" : "Pair",
            secondary: d.battery !== null && d.battery !== undefined ? d.battery + "% battery" : ""
        };
    }

    function bluetoothRows() {
        var keep = function (d) {
            return !padsOnly || d.kind === "pad";
        };
        var paired = bluetooth.paired.filter(keep), found = bluetooth.found.filter(keep);
        var out = [];
        if (padsOnly)
            out.push({
                key: "hint",
                label: "Pair New Controller",
                type: "static",
                display: "",
                wraps: true,
                detail: "Hold the controller's pairing button (SYNC, or PS and Create, or Xbox and the pair button) until its lights blink. With no controller connected yet, plug one in with a USB cable first."
            });
        if (!padsOnly || !bluetooth.powered)
            out.push({
                key: "power",
                label: "Bluetooth",
                type: "bool",
                value: bluetooth.powered,
                display: "",
                detail: ""
            });
        if (paired.length > 0)
            out = out.concat([
                {
                    heading: true,
                    label: padsOnly ? "Paired Controllers" : "Paired Devices",
                    display: ""
                }
            ], paired.map(deviceRow));
        out.push({
            heading: true,
            label: padsOnly ? "Controllers Found" : "Devices Found",
            display: bluetooth.discovering ? "Searching…" : ""
        });
        return out.concat(found.length > 0 ? found.map(deviceRow) : [
            {
                key: "searching",
                label: !bluetooth.powered ? "Bluetooth is off" : bluetooth.discovering ? "Searching…" : "Nothing found",
                type: "static",
                display: "",
                detail: ""
            }
        ]);
    }

    function deviceAction(row) {
        if (!row.paired) {
            Sound.play(bluetooth.pair(row.address) ? "ok" : "edge");
            return;
        }
        Sound.play("ok");
        shell.dialogAsk({
            message: row.label,
            detail: row.connected ? "Connected." : "Paired, not connected.",
            buttons: ["Cancel", "Forget", row.connected ? "Disconnect" : "Connect"],
            danger: 1
        }, function (i) {
            if (i === 1)
                bluetooth.forget(row.address);
            else if (i === 2 && row.connected)
                bluetooth.disconnectDevice(row.address);
            else if (i === 2)
                bluetooth.connectDevice(row.address);
        });
    }

    function forgetRow() {
        var row = rows.currentRow;
        if (sectionId !== "network" || zone !== "rows" || !row || row.key !== "network" || !row.saved) {
            Sound.play("edge");
            return;
        }
        shell.forgetNetwork(row.ssid);
    }

    function activate(index, row) {
        if (sectionId === "search") {
            openSearch();
        } else if (sectionId === "about" && row.key === "changelog") {
            Sound.play("ok");
            shell.push("pages/ChangelogPage.qml", {});
        } else if (sectionId === "about" && row.key === "setup") {
            Sound.play("ok");
            shell.push("pages/OnboardingPage.qml", {});
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
        } else if (sectionId === "controllers") {
            Sound.play("ok");
            shell.push("pages/ControllersPage.qml", {});
        } else if (sectionId === "data") {
            Sound.play("ok");
            shell.push("pages/StoragePage.qml", {});
        } else if (sectionId === "network") {
            if (row.key === "wifi") {
                Sound.play("select");
                network.setEnabled(!row.value);
            } else if (row.key === "check") {
                Sound.play(network.check() ? "ok" : "edge");
            } else if (row.key === "network") {
                shell.joinNetwork(row);
            } else {
                Sound.play("edge");
            }
        } else if (sectionId === "bluetooth") {
            if (row.key === "power") {
                Sound.play("select");
                bluetooth.setPowered(!row.value);
            } else if (row.key === "device") {
                deviceAction(row);
            } else {
                Sound.play("edge");
            }
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
        } else if (row.action === "theme" && row.dim) {
            Sound.play("edge");
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
        shownId = sections[Math.min(section, sections.length - 1)].id;
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
        } else if (api.keys.isDetails(event) && page.sectionId === "network") {
            event.accepted = true;
            page.forgetRow();
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
