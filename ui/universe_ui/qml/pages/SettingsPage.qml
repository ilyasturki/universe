import QtQuick
import "../core"
import "../sound"
import "../ui"

// The Settings tab: modules and their global settings, a source's library to install from,
// pending updates, the login flow, and doctor's checks. One generic list, five row sources.
FocusScope {
    id: page

    focus: true

    signal chromeRequested()

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

    readonly property var sections: ["Modules", "Install", "Updates", "Login", "Doctor"]
    property int section: 0

    readonly property var modulesForm: api.screens.modules
    readonly property var sources: api.screens.sources
    readonly property var login: api.screens.login

    readonly property var hints: sheet.open ? sheet.hints
        : picker.open ? picker.hints
        : chipBar.activeFocus
        ? [ { glyph: "A", label: "Open" }, { glyph: "dpad", label: "Section" }, { glyph: "LB RB", label: "Tabs" } ]
        : (acceptLabel !== "" ? [ { glyph: "A", label: acceptLabel } ] : []).concat(
            [ { glyph: "dpad", label: "Navigate" }, { glyph: "LT RT", label: "Section" }, { glyph: "LB RB", label: "Tabs" } ])

    readonly property string acceptLabel: {
        var row = list.currentRow;
        if (!row || row.type === "info")
            return "";
        if (row.type === "bool")
            return "Toggle";
        if (row.type === "action")
            return row.display && section === 1 ? row.display : "Select";
        return "Change";
    }

    readonly property real sideMargin: Theme.dp(80)

    readonly property string sourceName: {
        for (var i = 0; i < sources.sources.length; i++)
            if (sources.sources[i].id === sources.source)
                return sources.sources[i].name;
        return sources.source;
    }

    readonly property bool loggedIn: {
        for (var i = 0; i < sources.sources.length; i++)
            if (sources.sources[i].id === sources.source)
                return sources.sources[i].logged_in === true;
        return false;
    }

    // The rows the list shows for the open section, built from the host's data.
    readonly property var rows: {
        var out = [];
        if (section === 0)
            return modulesForm.rows;
        if (section === 1) {
            out.push({ section: sourceName, key: "search", label: "Search " + sourceName, type: "action",
                       display: sources.query || "", choices: [], detail: "" });
            for (var i = 0; i < sources.rows.length; i++) {
                var g = sources.rows[i];
                out.push({ section: sourceName, key: "game", label: g.title, type: "action",
                           display: g.status, choices: [], detail: "", row: i, installed: g.installed, pending: g.pending });
            }
            return out;
        }
        if (section === 2) {
            if (sources.updates.length === 0)
                out.push({ section: "Updates", key: "", label: "Everything is up to date", type: "info", value: true, detail: "" });
            else
                out.push({ section: "Updates", key: "all", label: "Update everything", type: "action", display: sources.updates.length + " pending", detail: "" });
            for (var j = 0; j < sources.updates.length; j++) {
                var u = sources.updates[j];
                out.push({ section: "Updates", key: "update", label: u.title, type: "action",
                           display: (u.version ? u.version + " · " : "") + (u.date || ""), detail: "", row: j });
            }
            return out;
        }
        if (section === 3) {
            out.push({ section: sourceName, key: "", label: "Signed in", type: "info", value: loggedIn, detail: loggedIn ? "yes" : "no" });
            out.push({ section: sourceName, key: "link", label: "Get a sign-in link", type: "action", display: login.url ? "ready" : "", detail: "" });
            out.push({ section: sourceName, key: "code", label: "Enter the code", type: "action", display: "", detail: "" });
            return out;
        }
        return modulesForm.doctor;
    }

    function leave() {
        picker.hide();
    }

    function refresh() {
        if (section === 0)
            modulesForm.load();
        else if (section === 1 || section === 2)
            sources.load();
        else if (section === 3)
            sources.load();
        else
            modulesForm.loadDoctor();
    }

    function activate(index, row) {
        if (section === 0) {
            if (row.type === "bool") {
                modulesForm.toggle(index);
                Sound.favourite(!row.value);
            } else if (row.type === "enum") {
                Sound.panel();
                picker.pendingIndex = index;
                picker.show(list, row.choices.map(function(c) { return { label: c }; }), Math.max(0, row.choices.indexOf(row.value)));
            } else {
                Sound.panel();
                sheet.pendingIndex = index;
                sheet.pendingKind = "module";
                sheet.show(row.label, row.value, row.type === "path");
            }
        } else if (section === 1) {
            if (row.key === "search") {
                Sound.panel();
                sheet.pendingKind = "search";
                sheet.show("Search " + sourceName, sources.query, false);
            } else if (row.installed && !row.pending) {
                Sound.edge();
            } else {
                Sound.enter();
                sources.install(row.row);
            }
        } else if (section === 2) {
            Sound.enter();
            if (row.key === "all")
                sources.updateAll();
            else
                sources.update(row.row);
        } else if (section === 3) {
            if (row.key === "link") {
                Sound.enter();
                login.begin(sources.source);
            } else if (row.key === "code") {
                Sound.panel();
                sheet.pendingKind = "code";
                sheet.show("Code from " + sourceName, "", false);
            }
        }
    }

    Component.onCompleted: {
        modulesForm.load();
        modulesForm.loadDoctor();
        sources.load();
    }

    onSectionChanged: {
        list.index = 0;
        refresh();
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

    Item {
        id: header

        z: 2
        anchors.top: parent.top
        anchors.topMargin: Theme.dp(34)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: titleText.height + Theme.dp(18) + chips.height

        Text {
            id: titleText
            text: "Settings"
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Bold
            font.pixelSize: Theme.dp(58)
        }

        FocusScope {
            id: chipBar

            anchors.top: titleText.bottom
            anchors.topMargin: Theme.dp(18)
            anchors.left: parent.left
            width: chips.width
            height: chips.height

            function step(d) {
                var next = Math.max(0, Math.min(page.sections.length - 1, page.section + d));
                if (next === page.section) {
                    Sound.edge();
                    return;
                }
                page.section = next;
                Sound.tick();
            }

            Row {
                id: chips
                spacing: Theme.dp(14)

                Repeater {
                    model: page.sections

                    Chip {
                        label: modelData
                        trailing: index === 2 && page.sources.updates.length > 0 ? page.sources.updates.length.toString() : ""
                        focused: chipBar.activeFocus && index === page.section
                        opacity: index === page.section || chipBar.activeFocus ? 1.0 : 0.6
                    }
                }
            }

            Keys.onLeftPressed: chipBar.step(-1)
            Keys.onRightPressed: chipBar.step(1)
            Keys.onUpPressed: page.chromeRequested()
            Keys.onDownPressed: function(event) {
                Sound.panel();
                list.forceActiveFocus();
            }

            Keys.onPressed: function(event) {
                if (event.isAutoRepeat)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    Sound.panel();
                    list.forceActiveFocus();
                    return;
                }
                if (api.keys.isCancel(event)) {
                    event.accepted = true;
                    Sound.cancel();
                    list.forceActiveFocus();
                    return;
                }
            }
        }
    }

    // A running job: its message and a bar, above whatever section is open.
    Rectangle {
        id: jobBar

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(24)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        height: visible ? Theme.dp(64) : 0
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

    SettingsList {
        id: list

        anchors.top: jobBar.bottom
        anchors.topMargin: Theme.dp(20)
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.leftMargin: page.sideMargin - Theme.dp(22)
        anchors.right: page.section === 3 ? loginPane.left : parent.right
        anchors.rightMargin: page.section === 3 ? Theme.dp(40) : page.sideMargin - Theme.dp(22)
        focus: true
        rows: page.rows
        dimmed: picker.open || sheet.open

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedUp: {
            Sound.panel();
            chipBar.forceActiveFocus();
        }

        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                event.accepted = true;
                Sound.cancel();
                chipBar.forceActiveFocus();
            }
        }
    }

    Column {
        id: loginPane

        anchors.top: jobBar.bottom
        anchors.topMargin: Theme.dp(20)
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        width: Theme.dp(640)
        spacing: Theme.dp(20)
        visible: page.section === 3

        QrCode {
            width: Theme.dp(360)
            height: width
            matrix: page.login.matrix
            visible: page.login.url !== ""
        }

        Text {
            width: parent.width
            text: page.login.url
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(18)
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 4
            elide: Text.ElideRight
        }

        Text {
            width: parent.width
            text: page.login.status
            color: Theme.text
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(22)
            wrapMode: Text.WordWrap
        }
    }

    ChipPicker {
        id: picker

        property int pendingIndex: -1

        x: page.width / 2 - width / 2
        y: Math.min(page.height - height - Theme.dp(120),
                    list.y + Theme.dp(60) + Math.max(0, (list.index - 1)) * (list.rowHeight + Theme.dp(4)))
        z: 3

        onChosen: function(index) {
            var choices = page.modulesForm.row(pendingIndex).choices || [];
            picker.hide();
            list.forceActiveFocus();
            if (index >= 0 && index < choices.length) {
                Sound.sort();
                page.modulesForm.setValue(pendingIndex, choices[index]);
            }
        }
        onDismissed: {
            picker.hide();
            list.forceActiveFocus();
        }
    }

    Toast {
        id: toast
        z: 4
    }

    KeyboardSheet {
        id: sheet

        property int pendingIndex: -1
        property string pendingKind: ""

        anchors.fill: parent
        anchors.bottomMargin: -Theme.dp(Theme.hintBarHeight)
        z: 5

        onAccepted: function(value) {
            list.forceActiveFocus();
            if (pendingKind === "module")
                page.modulesForm.setValue(pendingIndex, value);
            else if (pendingKind === "search")
                page.sources.search(value);
            else if (pendingKind === "code")
                page.login.submit(value);
        }
        onDismissed: list.forceActiveFocus()
    }

    // Triggers cycle the section from anywhere, as they cycle the collection in the library.
    Keys.onPressed: function(event) {
        if (api.keys.isPageUp(event) || api.keys.isPageDown(event))
            event.accepted = true;
    }

    Keys.onReleased: function(event) {
        if (event.isAutoRepeat || sheet.open || picker.open)
            return;
        if (api.keys.isPageUp(event)) {
            event.accepted = true;
            chipBar.step(-1);
        } else if (api.keys.isPageDown(event)) {
            event.accepted = true;
            chipBar.step(1);
        }
    }
}
