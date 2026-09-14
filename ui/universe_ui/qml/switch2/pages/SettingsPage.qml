import QtQuick
import "../core"
import "../sound"
import "../ui"
import "Details.js" as Details

FocusScope {
    id: page

    property var shell: null
    property var args: ({})
    signal closeRequested()
    focus: true

    readonly property var modulesForm: api.screens.modules
    readonly property var runners: api.screens.runners
    readonly property var sources: api.screens.sources
    readonly property var login: api.screens.login

    readonly property var sections: [
        { id: "runners", label: "Runners", group: 0 },
        { id: "modules", label: "Modules", group: 0 },
        { id: "updates", label: "Updates", detail: sources.updates.length > 0 ? sources.updates.length + " pending" : "", group: 0 },
        { id: "signin", label: "Sign-in", group: 0 },
        { id: "controllers", label: "Controllers", group: 1 },
        { id: "themes", label: "Themes", group: 1 },
        { id: "doctor", label: "Doctor", group: 2 },
        { id: "about", label: "About", group: 2 }
    ]
    property int section: 0
    readonly property string sectionId: sections[section].id
    property string zone: "list"
    readonly property bool folderOpen: folder.open
    // The runner whose page is open: the list reloads under it and the cursor finds it again.
    property string openedRunner: ""

    readonly property var hints: {
        if (folderOpen)
            return folder.hints;
        var out = [];
        if (sectionId === "runners" || sectionId === "updates" || sectionId === "signin" || sectionId === "doctor")
            out.push({ glyph: "Y", label: "Refresh" });
        out.push({ glyph: "B", label: "Back" });
        var row = rows.currentRow;
        var label = zone !== "rows" ? "OK" : !row || row.heading || row.type === "info" || row.type === "static" || row.disabled ? "OK"
                  : row.type === "bool" ? "Toggle" : row.type === "radio" || row.type === "action" ? "Select" : "Change";
        out.push({ glyph: "A", label: label });
        return out;
    }

    readonly property string sourceName: sources.current ? sources.current.name : (sources.source || "Source")
    readonly property bool loggedIn: sources.current ? sources.current.logged_in === true : false

    function sectionIndex(id) {
        return Math.max(0, sections.map(function(s) { return s.id; }).indexOf(id));
    }

    onArgsChanged: {
        if (args && args.section) {
            section = sectionIndex(args.section);
            list.index = section;
            list.current = section;
        }
    }

    Component.onCompleted: {
        runners.load();
        modulesForm.load();
        modulesForm.loadDoctor();
        sources.load();
    }

    onActiveFocusChanged: {
        if (!activeFocus || openedRunner === "")
            return;
        runners.load();
        var i = rowOfRunner(openedRunner);
        openedRunner = "";
        if (i >= 0)
            rows.index = i;
    }

    function rowOfRunner(id) {
        var list = content;
        for (var i = 0; i < list.length; i++)
            if (list[i].runner === id)
                return i;
        return -1;
    }

    readonly property var content: {
        var out = [], i, j;
        if (sectionId === "runners") {
            var rg = runners.groups, rr = runners.rows;
            for (i = 0; i < rg.length; i++) {
                if (rg[i].title)
                    out.push({ heading: true, label: rg[i].title, display: "" });
                for (j = 0; j < rg[i].rows.length; j++) {
                    var run = rr[rg[i].rows[j]];
                    out.push({ label: run.label, type: "action", action: "runner", runner: run.runner, icon: run.icon, iconSlot: true,
                               display: run.display, detail: "", dim: rg[i].off === true });
                }
            }
            return out;
        }
        if (sectionId === "modules") {
            var groups = modulesForm.groups, all = modulesForm.rows;
            for (i = 0; i < groups.length; i++) {
                var g = groups[i];
                out.push({ heading: true, label: g.title, display: g.meta || "" });
                if (g.control >= 0) {
                    var control = Details.withDetail(all[g.control], "");
                    control.form = g.control;
                    control.detail = g.warning ? "Cannot be enabled: " + g.warning.replace(/^unavailable:?\s*/, "") : Details.enabledSentence(g.title, g.meta);
                    control.disabled = g.warning !== "" && !control.value;
                    out.push(control);
                }
                if (g.off)
                    continue;
                for (j = 0; j < g.rows.length; j++) {
                    var r = Details.withDetail(all[g.rows[j]], all[g.rows[j]].module);
                    r.form = g.rows[j];
                    out.push(r);
                }
            }
            return out;
        }
        if (sectionId === "updates") {
            var n = sources.updates.length;
            if (n === 0) {
                out.push({ label: sources.busy ? "Checking…" : "Everything is up to date", type: "info", value: true, display: "", detail: "" });
                return out;
            }
            out.push({ label: "Update everything", type: "action", display: n + " pending", action: "update-all", detail: "" });
            for (i = 0; i < n; i++) {
                var u = sources.updates[i];
                out.push({ label: u.title, type: "action", display: (u.version ? u.version + " · " : "") + (u.date || ""),
                           action: "update", row: i, detail: "" });
            }
            return out;
        }
        if (sectionId === "signin") {
            out.push({ heading: true, label: sourceName, display: "" });
            out.push({ label: "Signed in", type: "info", value: loggedIn, display: loggedIn ? "Yes" : "No", detail: "" });
            out.push({ label: "Get a sign-in link", type: "action", action: "link", display: login.url ? "Ready" : "", detail: "" });
            out.push({ label: "Enter the code", type: "action", action: "code", display: "", detail: "" });
            return out;
        }
        if (sectionId === "doctor") {
            var dg = modulesForm.doctorGroups, dr = modulesForm.doctor;
            for (i = 0; i < dg.length; i++) {
                out.push({ heading: true, label: dg[i].title, display: dg[i].meta || "" });
                for (j = 0; j < dg[i].rows.length; j++) {
                    var c = dr[dg[i].rows[j]];
                    out.push({ label: c.label, type: "info", value: c.value === true, display: c.detail || "", detail: "" });
                }
            }
            if (out.length === 0)
                out.push({ label: "No checks yet", type: "info", value: true, display: "", detail: "" });
            return out;
        }
        if (sectionId === "controllers") {
            out.push({ label: "Controllers", type: "action", action: "controllers", display: "", detail: "" });
            return out;
        }
        if (sectionId === "themes") {
            var themes = api.theme.themes;
            for (i = 0; i < themes.length; i++)
                out.push({ label: themes[i].name, type: "radio", value: themes[i].id === api.theme.current, swatch: themes[i].ground,
                           action: "theme", theme: themes[i].id, detail: themes[i].detail || "" });
            out.push({ heading: true, label: "Font", display: "" });
            out.push({ label: "Font file", type: "path", action: "font", value: api.theme.fontPath,
                       display: api.theme.fontPath ? api.theme.fontPath.split("/").pop() : "Bundled (BIZ UDPGothic)",
                       detail: "A .ttf you own, such as the Switch's own; applies at once." });
            return out;
        }
        if (sectionId === "about") {
            out.push({ label: "Universe", type: "static", display: api.universe.version() || "development build", detail: "" });
            out.push({ label: "Look", type: "static", display: api.theme.name, detail: "" });
            out.push({ label: "Library", type: "static", display: api.allGames.count + (api.allGames.count === 1 ? " game" : " games"), detail: "" });
            return out;
        }
        return out;
    }

    function activate(index, row) {
        if (sectionId === "runners") {
            Sound.ok();
            openedRunner = row.runner;
            shell.push("pages/RunnerPage.qml", { runner: row.runner });
            return;
        }
        if (sectionId === "modules") {
            if (row.type === "bool") {
                modulesForm.toggle(row.form);
                Sound.select();
            } else {
                rows.edit(row, function(value) { modulesForm.setValue(row.form, value); });
            }
            return;
        }
        if (sectionId === "updates") {
            Sound.ok();
            if (row.action === "update-all")
                sources.updateAll();
            else
                sources.update(row.row);
            return;
        }
        if (sectionId === "signin") {
            if (row.action === "link") {
                Sound.ok();
                login.begin(sources.source);
            } else if (row.action === "code") {
                shell.prompt({ title: "Code from " + sourceName, value: "" }, function(value) {
                    if (value !== null && value !== "")
                        login.submit(value);
                });
            }
            return;
        }
        if (sectionId === "controllers") {
            Sound.ok();
            shell.push("pages/ControllersPage.qml", {});
            return;
        }
        if (sectionId === "themes") {
            if (row.action === "theme") {
                Sound.select();
                var id = row.theme;
                // Reprise replaces this tree: let the press finish first.
                Qt.callLater(function() { api.theme.set(id); });
            } else if (row.action === "font") {
                folder.show({ title: "Font file", path: api.theme.fontPath, files: true }, function(path) {
                    if (path !== null)
                        api.theme.fontPath = path;
                    rows.forceActiveFocus();
                });
            }
            return;
        }
        Sound.edge();
    }

    function refreshNow() {
        if (sectionId === "runners") {
            Sound.ok();
            runners.load();
        } else if (sectionId === "updates" || sectionId === "signin") {
            Sound.ok();
            sources.refresh();
        } else if (sectionId === "doctor") {
            Sound.ok();
            modulesForm.loadDoctor();
        } else {
            Sound.edge();
        }
    }

    onSectionChanged: Qt.callLater(rows.reset)

    Connections {
        target: page.sources
        function onMessage(text) { page.shell.showToast(text); }
    }

    Connections {
        target: page.login
        function onFinished(ok, text) {
            page.shell.showToast(text);
            page.sources.refresh();
        }
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat || page.folderOpen)
            return;
        if (api.keys.isCancel(event) && page.zone === "rows") {
            event.accepted = true;
            Sound.back();
            page.zone = "list";
            list.forceActiveFocus();
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            page.refreshNow();
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
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
        current: page.section
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
        folder: folder
        x: Theme.dp(705)
        y: header.height + Theme.dp(64) + jobLine.height
        width: Theme.dp(1023)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20) - (qrCard.visible ? qrCard.height + Theme.dp(20) : 0)
        model: page.content
        focus: page.zone === "rows"

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedLeft: {
            page.zone = "list";
            list.forceActiveFocus();
        }
        onEscapedUp: Sound.edge()
    }

    Item {
        id: qrCard

        x: rows.x
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(Theme.hintBarHeight) + Theme.dp(20)
        width: rows.width
        height: Theme.dp(330)
        visible: page.sectionId === "signin" && (page.login.url !== "" || page.login.status !== "")

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(6)
            color: Theme.card
            border.width: 1
            border.color: Theme.hairline
        }

        Loader {
            id: qr
            x: Theme.dp(24)
            y: Theme.dp(24)
            width: Theme.dp(282)
            height: width
            visible: page.login.url !== ""
            source: "../../ui/QrCode.qml"
            onLoaded: item.matrix = Qt.binding(function() { return page.login.matrix; })
        }

        Column {
            x: qr.visible ? qr.x + qr.width + Theme.dp(30) : Theme.dp(30)
            y: Theme.dp(30)
            width: parent.width - x - Theme.dp(30)
            spacing: Theme.dp(14)

            Text {
                width: parent.width
                text: "Scan to sign in on your phone"
                color: Theme.text
                font.family: Theme.sans
                font.pixelSize: Theme.dp(Theme.fontBody)
            }

            Text {
                width: parent.width
                text: page.login.url
                color: Theme.accent
                wrapMode: Text.WrapAnywhere
                maximumLineCount: 4
                elide: Text.ElideRight
                font.family: Theme.sans
                font.pixelSize: Theme.dp(Theme.fontTiny)
            }

            Text {
                width: parent.width
                text: page.login.status
                color: Theme.textSecondary
                wrapMode: Text.WordWrap
                font.family: Theme.sans
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }

    FolderPage {
        id: folder
        z: 5
        onTypeRequested: function(path) {
            page.shell.prompt({ title: "Path", value: path, path: true }, function(v) {
                folder.finish(v);
            });
        }
    }
}
