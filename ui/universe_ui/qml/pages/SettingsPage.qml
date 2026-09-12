import QtQuick
import "../core"
import "../sound"
import "../ui"

// The Settings tab: modules and their global settings, a source's library to install from,
// pending updates, the login flow, and doctor's checks. One set of cards, five row sources.
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

    readonly property real sideMargin: Theme.dp(80)

    readonly property var currentSource: {
        for (var i = 0; i < sources.sources.length; i++)
            if (sources.sources[i].id === sources.source)
                return sources.sources[i];
        return null;
    }
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

    // The rows and the cards that arrange them for the open section, from the host's data.
    readonly property var content: {
        var rows = [], groups = [];
        if (section === 0)
            return { rows: modulesForm.rows, groups: modulesForm.groups };
        if (section === 1) {
            rows.push({ section: sourceName, key: "search", label: "Search " + sourceName, type: "search",
                        display: sources.query || "", choices: [], detail: "" });
            groups.push({ span: true, rows: [0] });
            var installed = [], owned = [], all = [];
            for (var i = 0; i < sources.rows.length; i++) {
                var g = sources.rows[i];
                var installable = g.pending || !g.installed;
                rows.push({ section: sourceName, key: "game", label: g.title, type: "action",
                            display: g.status, choices: [], detail: "", row: i, installed: g.installed,
                            pending: g.pending, action: installable ? g.action : "" });
                all.push(rows.length - 1);
                (g.installed ? installed : owned).push(rows.length - 1);
            }
            if (sources.query)
                groups.push({ title: "Results", meta: games(all.length) + " · “" + sources.query + "”", rows: all });
            else {
                var where = currentSource && currentSource.games_dir ? " · " + currentSource.games_dir : "";
                groups.push({ title: "Installed", meta: games(installed.length) + where, rows: installed });
                groups.push({ title: "Owned, not installed", meta: games(owned.length), rows: owned });
            }
            return { rows: rows, groups: groups };
        }
        if (section === 2) {
            var n = sources.updates.length;
            if (n === 0) {
                rows.push({ section: "Updates", key: "", label: "Everything is up to date", type: "info", value: true, detail: "" });
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
        if (section === 3) {
            rows.push({ section: sourceName, key: "", label: "Signed in", type: "info", value: loggedIn, detail: loggedIn ? "yes" : "no" });
            rows.push({ section: sourceName, key: "link", label: "Get a sign-in link", type: "action", display: login.url ? "ready" : "", detail: "" });
            rows.push({ section: sourceName, key: "code", label: "Enter the code", type: "action", display: "", detail: "" });
            groups.push({ title: sourceName, meta: sourceMeta, rows: [0, 1, 2] });
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
                picker.show(cards, row.choices.map(function(c) { return { label: c }; }), Math.max(0, row.choices.indexOf(row.value)));
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

    // The cards' rows rebind on the same signal; the cursor resets once they have.
    onSectionChanged: {
        refresh();
        Qt.callLater(cards.reset);
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
                        badge: index === 2 && page.sources.updates.length > 0 ? page.sources.updates.length.toString() : ""
                        active: index === page.section
                        focused: chipBar.activeFocus && index === page.section
                    }
                }
            }

            Keys.onLeftPressed: chipBar.step(-1)
            Keys.onRightPressed: chipBar.step(1)
            Keys.onUpPressed: page.chromeRequested()
            Keys.onDownPressed: function(event) {
                Sound.panel();
                cards.forceActiveFocus();
            }

            Keys.onPressed: function(event) {
                if (event.isAutoRepeat)
                    return;
                if (api.keys.isAccept(event)) {
                    event.accepted = true;
                    Sound.panel();
                    cards.forceActiveFocus();
                    return;
                }
                if (api.keys.isCancel(event)) {
                    event.accepted = true;
                    Sound.cancel();
                    cards.forceActiveFocus();
                    return;
                }
            }
        }
    }

    // Above the cards: a running job's message and bar, and on Doctor the tally of checks.
    Column {
        id: above

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(30)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
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

            visible: page.section === 4 && page.modulesForm.doctor.length > 0
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

        anchors.top: above.bottom
        anchors.topMargin: above.height > 0 ? Theme.dp(32) : 0
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        focus: true
        rows: page.content.rows
        groups: page.content.groups
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

    // The sign-in link as a card in the right column: the code to scan, the link, the state.
    Item {
        id: loginCard

        x: cards.x + cards.columnX(1)
        y: cards.y
        width: cards.columnWidth
        height: Math.max(Theme.dp(74), loginHead.height) + loginBody.height + Theme.dp(8) * 2 + 2
        visible: page.section === 3 && (page.login.url !== "" || page.login.status !== "")

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

    ChipPicker {
        id: picker

        property int pendingIndex: -1

        // Drops from the focused row, its right edge on the row's value.
        x: cards.x + cards.focusRect.x + cards.focusRect.width - Theme.dp(16) - width
        y: Math.min(page.height - height - Theme.dp(20),
                    cards.y + cards.focusRect.y + cards.focusRect.height + Theme.dp(8))
        z: 3

        onChosen: function(index) {
            var choices = page.modulesForm.row(pendingIndex).choices || [];
            picker.hide();
            cards.forceActiveFocus();
            if (index >= 0 && index < choices.length) {
                Sound.sort();
                page.modulesForm.setValue(pendingIndex, choices[index]);
            }
        }
        onDismissed: {
            picker.hide();
            cards.forceActiveFocus();
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
            cards.forceActiveFocus();
            if (pendingKind === "module")
                page.modulesForm.setValue(pendingIndex, value);
            else if (pendingKind === "search")
                page.sources.search(value);
            else if (pendingKind === "code")
                page.login.submit(value);
        }
        onDismissed: cards.forceActiveFocus()
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
