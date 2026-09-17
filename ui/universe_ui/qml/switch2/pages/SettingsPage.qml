import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Details.js" as Details
import "Forms.js" as Forms

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    signal closeRequested()
    focus: true

    readonly property var modulesForm: api.screens.modules
    readonly property var sourceList: api.screens.sourceList
    readonly property var launch: api.screens.launch
    readonly property var runners: api.screens.runners
    readonly property var sources: api.screens.sources
    // Modules and Sources are the same list: a switch per entry, A opens its page, X flips it.
    readonly property var listForm: sectionId === "modules" ? modulesForm : sectionId === "sources" ? sourceList : null

    readonly property var sections: [
        { id: "runners", label: "Runners", group: 0 },
        { id: "launch", label: "Launch", group: 0 },
        { id: "modules", label: "Modules", group: 0 },
        { id: "sources", label: "Sources", group: 0 },
        { id: "updates", label: "Updates", detail: sources.updates.length > 0 ? sources.updates.length + " pending" : "", group: 0 },
        { id: "controllers", label: "Controllers", group: 1 },
        { id: "themes", label: "Themes", group: 1 },
        { id: "doctor", label: "Doctor", group: 2 },
        { id: "about", label: "About", group: 2 }
    ]
    property int section: 0
    readonly property string sectionId: sections[section].id
    property string zone: "list"
    property var reopen: null

    readonly property var loaders: ({
        runners: function() { runners.load(); },
        launch: function() { launch.load(); },
        modules: function() { modulesForm.load(); },
        sources: function() { sourceList.load(); },
        doctor: function() { modulesForm.loadDoctor(); }
    })
    readonly property var refreshers: ({
        runners: function() { runners.load(); },
        updates: function() { sources.refresh(); },
        doctor: function() { modulesForm.loadDoctor(); }
    })
    readonly property var loaded: ({})

    readonly property var hints: {
        var out = [];
        if (refreshers[sectionId])
            out.push({ glyph: "Y", label: "Refresh" });
        var row = rows.currentRow;
        if (listForm !== null && zone === "rows" && row && !row.heading)
            out.push({ glyph: "X", label: row.value === true ? "Disable" : "Enable", dim: row.dim === true });
        out.push({ glyph: "B", label: "Back" });
        var label = zone !== "rows" ? "OK" : !row || row.heading || row.type === "info" || row.type === "static" || row.disabled ? "OK"
                  : row.type === "bool" ? "Toggle" : row.type === "radio" ? "Select" : row.type === "action" ? (listForm !== null ? "Open" : "Select") : "Change";
        out.push({ glyph: "A", label: label });
        return out;
    }


    function sectionIndex(id) {
        return Math.max(0, sections.map(function(s) { return s.id; }).indexOf(id));
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
        sources.load();
        loadSection();
    }

    onActiveFocusChanged: {
        if (!activeFocus || !reopen)
            return;
        reopen.form.load();
        var i = content.findIndex(function(r) { return r[reopen.field] === reopen.id; });
        reopen = null;
        if (i >= 0)
            rows.index = i;
    }

    readonly property var content: {
        if (sectionId === "runners")
            return Forms.grouped(runners.groups, runners.rows, function(run, i, g) {
                return { label: run.label, type: "action", action: "runner", runner: run.runner, icon: run.icon, iconSlot: true,
                         display: run.display, detail: "", dim: g.off === true };
            });
        if (sectionId === "launch")
            return Forms.grouped(launch.groups, launch.rows, function(r, i) { return Object.assign(Details.withDetail(r, ""), { form: i }); });
        if (listForm !== null)
            return Forms.grouped(listForm.groups, listForm.rows, function(m, i) {
                return { label: m.label, type: "action", action: "module", module: m.module, value: m.value, display: m.display, switch: true, warning: m.warning,
                         detail: m.warning && !m.value ? m.detail : Details.enabledSentence(m.label, m.source),
                         form: i, dim: m.warning !== "" && m.value !== true };
            });
        if (sectionId === "updates") {
            var n = sources.updates.length;
            if (n === 0)
                return [{ label: sources.busy ? "Checking…" : "Everything is up to date", type: "info", value: true, display: "", detail: "" }];
            return [{ label: "Update everything", type: "action", display: n + " pending", action: "update-all", detail: "" }].concat(
                sources.updates.map(function(u, i) {
                    return { label: u.title, type: "action", display: (u.version ? u.version + " · " : "") + (u.date || ""), action: "update", row: i, detail: "" };
                }));
        }
        if (sectionId === "doctor") {
            var checks = Forms.grouped(modulesForm.doctorGroups, modulesForm.doctor, function(c) {
                return { label: c.label, type: "info", value: c.value === true, display: c.detail || "", detail: "" };
            });
            return checks.length > 0 ? checks : [{ label: "No checks yet", type: "info", value: true, display: "", detail: "" }];
        }
        if (sectionId === "controllers")
            return [{ label: "Controllers", type: "action", action: "controllers", display: "", detail: "" }];
        if (sectionId === "themes")
            return api.theme.themes.map(function(t) {
                return { label: t.name, type: "radio", value: t.id === api.theme.current, swatch: t.ground, action: "theme", theme: t.id, detail: t.detail || "" };
            }).concat([
                { heading: true, label: "Font", display: "" },
                { label: "Font file", key: "font_file", type: "path", action: "font", value: api.theme.fontPath,
                  display: api.theme.fontPath ? api.theme.fontPath.split("/").pop() : "Bundled (BIZ UDPGothic)",
                  detail: "A .ttf you own, such as the Switch's own; applies at once." }]);
        if (sectionId === "about")
            return [{ label: "Universe", type: "static", display: api.universe.version() || "development build", detail: "" },
                    { label: "Look", type: "static", display: api.theme.name, detail: "" },
                    { label: "Library", type: "static", display: api.allGames.count + (api.allGames.count === 1 ? " game" : " games"), detail: "" }];
        return [];
    }

    function activate(index, row) {
        if (sectionId === "runners") {
            Sound.play("ok");
            reopen = { form: runners, field: "runner", id: row.runner };
            shell.push("pages/FormPage.qml", { runner: row.runner });
        } else if (sectionId === "launch") {
            if (row.type === "bool") {
                launch.toggle(row.form);
                Sound.play("select");
            } else {
                rows.edit(row, function(value) { launch.setValue(row.form, value); });
            }
        } else if (sectionId === "modules") {
            Sound.play("ok");
            reopen = { form: modulesForm, field: "module", id: row.module };
            shell.push("pages/FormPage.qml", { module: row.module });
        } else if (sectionId === "sources") {
            Sound.play("ok");
            reopen = { form: sourceList, field: "module", id: row.module };
            shell.push("pages/FormPage.qml", { source: row.module });
        } else if (sectionId === "updates") {
            Sound.play("ok");
            if (row.action === "update-all")
                sources.updateAll();
            else
                sources.update(row.row);
        } else if (sectionId === "controllers") {
            Sound.play("ok");
            shell.push("pages/ControllersPage.qml", {});
        } else if (row.action === "theme") {
            Sound.play("select");
            var id = row.theme;
            // Reprise replaces this tree: let the press finish first.
            Qt.callLater(function() { api.theme.set(id); });
        } else if (row.action === "font") {
            rows.edit(row, function(path) { api.theme.fontPath = path; });
        }
    }

    function toggleModule() {
        var row = rows.currentRow;
        if (listForm === null || zone !== "rows" || !row || row.heading || row.dim === true) {
            Sound.play("edge");
            return;
        }
        Sound.play("select");
        listForm.toggle(row.form);
    }

    function refreshNow() {
        var f = refreshers[sectionId];
        if (!f) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        f();
    }

    onSectionChanged: {
        loadSection();
        Qt.callLater(rows.reset);
    }

    Connections {
        target: page.sources
        function onMessage(text) { page.shell.showToast(text); }
    }

    Keys.onPressed: function(event) {
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
        }
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

        onActivated: function(i) { page.section = i; }
        onEscapedRight: {
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
        width: Theme.dp(1023)
        job: page.sources.job
    }

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(705)
        y: header.height + Theme.dp(64) + jobLine.height
        width: Theme.dp(1023)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        model: page.content
        focus: page.zone === "rows"

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedLeft: {
            page.zone = "list";
            list.forceActiveFocus();
        }
    }
}
