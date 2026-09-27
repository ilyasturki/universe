import QtQuick
import "../core"
import "../sound"
import "../ui"
import "../../ui" as Base
import "../../ui/PadHistory.js" as History

// Settings › Controllers: the pad's rows at the left, its art at the right; the live test takes the whole screen.
FocusScope {
    id: page

    property var shell: null
    // { key }: a search hit lands on that row, the Advanced row opened if it sits behind it; { walk }: the walk starts.
    property var args: ({})
    readonly property var controller: api.screens.controller
    readonly property bool strip: true

    readonly property bool testing: controller.testing
    readonly property bool walking: controller.walking
    readonly property bool learning: controller.learning !== ""
    readonly property var step: controller.walkStep
    property var pressed: ({})
    property var axes: ({})
    // Newest first: { slot, label, dt }.
    property var inputs: []
    property var log: History.fresh()

    readonly property var hints: testing ? [
        {
            glyph: "B",
            label: "Hold to finish"
        },
        {
            glyph: "Start",
            label: ""
        },
        {
            glyph: "Select",
            label: "Finish"
        }
    ] : walking ? [] : learning ? [
        {
            glyph: "B",
            label: "Stop learning"
        }
    ] : [
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: "OK"
        }
    ]

    signal closeRequested

    function startWalk() {
        if (controller.startWalk())
            Sound.play("ok");
        else
            Sound.play("edge");
    }

    focus: true

    readonly property var entries: {
        var out = [], source = controller.rows, buttons = [];
        for (var i = 0; i < source.length; i++) {
            var r = source[i];
            if (r.key === "test" || r.key === "walk")
                out.push({
                    key: r.key,
                    label: r.label,
                    type: "action",
                    display: "",
                    detail: r.display,
                    icon: "gamepad"
                });
            else if (r.key === "device")
                out.push({
                    key: "device",
                    label: r.label,
                    type: "enum",
                    display: r.display,
                    choices: r.choices,
                    value: r.value
                });
            else if (r.type === "info")
                out.push({
                    key: "",
                    label: r.label,
                    type: "note",
                    display: "",
                    detail: r.detail,
                    disabled: true
                });
            else if (r.slot !== undefined)
                buttons.push({
                    key: r.slot,
                    label: r.label,
                    type: "action",
                    display: "",
                    detail: r.display,
                    slot: r.slot,
                    family: r.family,
                    bound: r.bound,
                    press: r.press,
                    hold: r.hold
                });
        }
        if (buttons.length > 0) {
            out.push({
                heading: true,
                label: "Buttons",
                display: ""
            });
            out = out.concat(buttons);
        }
        // The Timing card behind the Advanced row.
        var gate = source.findIndex(function (r) {
            return r.key === "advanced";
        });
        if (gate >= 0) {
            out.push({
                key: "advanced",
                label: "Advanced",
                type: "action",
                icon: "settings",
                display: "",
                detail: source[gate].detail,
                form: gate
            });
            if (controller.showAdvanced) {
                out.push({
                    heading: true,
                    label: "Timing",
                    display: ""
                });
                source.forEach(function (r, i) {
                    if (r.advanced)
                        out.push({
                            key: r.key,
                            label: r.label,
                            type: r.type,
                            display: r.display,
                            detail: r.detail,
                            choices: r.choices,
                            value: r.value,
                            form: i
                        });
                });
            }
        }
        return out;
    }

    function landNow() {
        if (args && args.walk) {
            args = {};
            Qt.callLater(startWalk);
            return;
        }
        var key = args && args.key ? args.key : "";
        if (key === "")
            return;
        controller.reveal(key, "");
        Qt.callLater(function () {
            var at = entries.findIndex(function (e) {
                return e.key === key;
            });
            if (at >= 0)
                rows.index = at;
        });
    }

    readonly property string focusedSlot: rows.cursorShown && rows.currentRow && rows.currentRow.slot ? rows.currentRow.slot : ""

    function labelOf(slot) {
        var bound = controller.rows;
        var hit = bound.filter(function (r) {
            return r.slot === slot;
        })[0];
        return hit ? hit.label : slot;
    }

    function toggled(set, key, on) {
        var next = Object.assign({}, set);
        delete next[key];
        if (on)
            next[key] = true;
        return next;
    }

    function press(slot, down) {
        pressed = toggled(pressed, slot, down);
        var logged = History.press(log, slot, down, labelOf(slot), Date.now());
        if (logged)
            inputs = logged;
    }

    function axis(name, value) {
        var next = Object.assign({}, axes);
        next[name] = value;
        axes = next;
        var logged = History.axis(log, name, value, labelOf, Date.now());
        if (logged)
            inputs = logged;
    }

    function clearArt() {
        pressed = ({});
        axes = ({});
        log = History.fresh();
        inputs = [];
        holdOut.stop();
    }

    function activate(index, row) {
        if (row.key === "advanced") {
            Sound.play("ok");
            controller.showAdvanced = !controller.showAdvanced;
            if (controller.showAdvanced)
                Qt.callLater(function () {
                    rows.index = Math.min(entries.length - 1, index + 2);
                });
        } else if (String(row.key).indexOf("controller.") === 0) {
            rows.edit(row, function (value) {
                controller.setValue(row.form, value);
            });
        } else if (row.key === "test") {
            if (controller.setTesting(true))
                Sound.play("ok");
            else
                Sound.play("edge");
        } else if (row.key === "walk") {
            startWalk();
        } else if (row.key === "device") {
            Sound.play("ok");
            var choices = row.choices || [];
            shell.pick({
                title: "Controller",
                choices: choices,
                index: choices.indexOf(String(row.value)),
                at: rows.anchorFor(index)
            }, function (i) {
                if (i >= 0)
                    controller.setValue(index, choices[i]);
            });
        } else if (row.slot !== undefined) {
            Sound.play("ok");
            slotMenu(row);
        } else {
            Sound.play("edge");
        }
    }

    function slotMenu(row) {
        var items = row.bound ? [] : [
            {
                label: "Learn the button",
                glyph: "gamepad",
                act: "learn"
            }
        ];
        if (!row.home)
            items.push({
                label: "On press…",
                act: "press"
            }, {
                label: "On hold…",
                act: "hold"
            });
        if (row.press)
            items.push({
                label: "Clear press",
                act: "clear-press"
            });
        if (row.hold)
            items.push({
                label: "Clear hold",
                act: "clear-hold"
            });
        if (row.bound)
            items.push({
                label: "Learn the button again",
                glyph: "gamepad",
                act: "learn",
                gap: items.length > 0
            });
        shell.menu(row.label, items, function (act) {
            slotAction(row, act);
        });
    }

    function slotAction(row, action) {
        if (action === "learn") {
            if (controller.learn(row.slot)) {
                Sound.play("ok");
                shell.showToast("Press the button on the controller…");
            }
        } else if (action === "press" || action === "hold") {
            presetMenu(row, action);
        } else if (action === "clear-press" || action === "clear-hold") {
            Sound.play("select");
            controller.unbind(row.slot, action.substring(6));
        }
    }

    function presetMenu(row, trigger) {
        var items = controller.presets.filter(function (p) {
            return !(p.hold_only && trigger !== "hold") && p.id !== "keys" && p.id !== "command";
        }).map(function (p) {
            return {
                label: p.label,
                id: "preset:" + p.id
            };
        });
        items.push({
            label: "Key combo…",
            id: "keys"
        }, {
            label: "Command…",
            id: "command"
        });
        var labels = items.map(function (i) {
            return i.label;
        }), ids = items.map(function (i) {
            return i.id;
        });
        var current = trigger === "hold" ? row.hold : row.press;
        var index = current ? ids.indexOf(current.action === "keys" || current.action === "command" ? current.action : "preset:" + current.action) : 0;
        shell.pick({
            title: row.label + " · " + (trigger === "press" ? "On press" : "On hold"),
            choices: labels,
            index: Math.max(0, index)
        }, function (i) {
            if (i < 0)
                return;
            var id = ids[i];
            if (id.indexOf("preset:") === 0) {
                Sound.play("select");
                controller.bind(row.slot, trigger, id.substring(7), "", "");
            } else {
                var value = current && current.action === id ? current[id] : "";
                shell.prompt({
                    title: (id === "keys" ? "Key combo for " : "Command for ") + row.label,
                    value: value,
                    max: 200
                }, function (text) {
                    if (text !== null && text !== undefined && text !== "") {
                        Sound.play("select");
                        controller.bind(row.slot, trigger, id, id === "keys" ? text : "", id === "command" ? text : "");
                    }
                });
            }
        });
    }

    Component.onCompleted: {
        controller.showAdvanced = false;
        controller.load();
        controller.suspend();
        landNow();
    }
    Component.onDestruction: controller.resume()
    onArgsChanged: landNow()

    onTestingChanged: {
        clearArt();
        if (testing) {
            tester.forceActiveFocus();
            return;
        }
        var i = entries.map(function (e) {
            return e.key;
        }).indexOf("test");
        if (i >= 0)
            rows.index = i;
        rows.forceActiveFocus();
    }

    Timer {
        id: holdOut
        interval: 1000
        onTriggered: {
            Sound.play("back");
            page.controller.setTesting(false);
        }
    }

    Connections {
        target: page.controller
        function onButtonPressed(id, slot, isDown) {
            if (id !== page.controller.current)
                return;
            page.press(slot, isDown);
            if (!page.testing)
                return;
            if (slot === "east")
                holdOut.running = isDown;
            if (isDown && page.pressed.start && page.pressed.select) {
                Sound.play("back");
                page.controller.setTesting(false);
            }
        }
        function onAxisMoved(id, axis, value) {
            if (id === page.controller.current)
                page.axis(axis, value);
        }
        function onUnknownPressed(id, code) {
            if (!page.learning && id === page.controller.current)
                page.shell.showToast(code + " is not one of the pad's buttons yet: learn it from a row");
        }
        function onLearned(family, slot, code) {
            page.shell.showToast(page.labelOf(slot) + " is now " + code);
        }
        function onMessage(text) {
            page.shell.showToast(text);
        }
    }

    property real pulse: 0.15
    SequentialAnimation on pulse {
        running: page.learning
        loops: Animation.Infinite
        NumberAnimation {
            to: 0.55
            duration: 500
            easing.type: Easing.InOutSine
        }
        NumberAnimation {
            to: 0.15
            duration: 500
            easing.type: Easing.InOutSine
        }
    }

    Backdrop {
        anchors.fill: parent
    }

    PageTitle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        title: "Controllers"
    }

    readonly property real columnTop: header.height + Theme.dp(24)
    readonly property real room: width - Theme.dp(172) - Theme.dp(Theme.columnRight)
    readonly property real gutter: Theme.dp(64)
    readonly property real listWidth: Math.min(Theme.dp(760), (room - gutter) / 2)

    SettingsRows {
        id: rows

        shell: page.shell
        x: Theme.dp(172)
        y: page.columnTop
        width: page.listWidth
        height: parent.height - y - Theme.dp(96)
        focus: !page.testing
        model: page.entries
        opacity: page.testing ? 0.0 : 1.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }

        onActivated: function (index, row) {
            page.activate(index, row);
        }
        onEscapedLeft: Sound.play("edge")
        onEscapedDown: Sound.play("edge")

        Keys.onPressed: function (event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event) && page.walking) {
                event.accepted = true;
                Sound.play("back");
                page.controller.cancelWalk();
            } else if (event.key === Qt.Key_Left && page.walking) {
                event.accepted = true;
                Sound.play(page.controller.backStep() ? "tick" : "edge");
            } else if (api.keys.isCancel(event) && page.learning) {
                event.accepted = true;
                Sound.play("back");
                page.controller.cancelLearn();
            }
        }
    }

    Rectangle {
        id: panel

        readonly property real restX: rows.x + page.listWidth + page.gutter

        x: page.testing ? Theme.dp(172) : restX
        y: page.columnTop
        width: page.testing ? page.room : page.width - restX - Theme.dp(Theme.columnRight)
        height: page.testing ? page.height - y - Theme.dp(110) : Theme.dp(520)
        radius: Theme.dp(Theme.radiusCard + 2)
        color: Qt.rgba(0.07, 0.08, 0.1, 0.9)
        border.width: 1
        border.color: Theme.glassEdge

        Behavior on x {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }
        Behavior on width {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }
        Behavior on height {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }

        Base.PadArt {
            anchors.fill: parent
            anchors.margins: Theme.dp(40)
            anchors.rightMargin: page.testing ? history.width + Theme.dp(60) : Theme.dp(40)
            opacity: page.controller.connected ? 1.0 : 0.38
            family: page.controller.family
            focusedSlot: page.focusedSlot
            learningSlot: page.controller.learning
            unbound: page.controller.connected ? page.controller.unboundSlots : []
            pressed: page.pressed
            axes: page.axes
            pulse: page.pulse
            readouts: page.testing

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.durBackdrop
                }
            }
        }

        Rectangle {
            id: history

            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.right: parent.right
            anchors.margins: Theme.dp(24)
            width: Theme.dp(440)
            radius: Theme.dp(Theme.radiusCard)
            color: Qt.rgba(1, 1, 1, 0.05)
            visible: page.testing

            Label {
                id: historyTitle
                x: Theme.dp(24)
                y: Theme.dp(20)
                text: "History"
                color: Theme.textMuted
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Label {
                anchors.centerIn: parent
                visible: page.inputs.length === 0
                text: page.controller.connected ? "Press anything on the pad" : "Connect a controller"
                color: Theme.textMuted
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Column {
                anchors.top: historyTitle.bottom
                anchors.topMargin: Theme.dp(12)
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.margins: Theme.dp(12)
                spacing: Theme.dp(2)

                Repeater {
                    model: page.inputs

                    Rectangle {
                        readonly property bool newest: index === 0
                        readonly property var entry: modelData || ({})
                        readonly property color ink: newest ? Theme.text : Theme.textSecondary
                        // A gap under 30 ms is not a human's second press: the pad double-fired.
                        readonly property bool doubled: entry.dt !== null && entry.dt !== undefined && entry.dt < 30

                        width: parent.width
                        height: Theme.dp(50)
                        radius: Theme.dp(Theme.radiusRow)
                        color: newest ? Qt.rgba(1, 1, 1, 0.08) : "transparent"

                        Base.PadGlyph {
                            id: glyph
                            x: Theme.dp(12)
                            anchors.verticalCenter: parent.verticalCenter
                            family: page.controller.family
                            slot: entry.slot || ""
                            unit: Theme.dp(32)
                            ink: parent.ink
                        }

                        Label {
                            anchors.left: glyph.right
                            anchors.leftMargin: Theme.dp(12)
                            anchors.right: gap.left
                            anchors.rightMargin: Theme.dp(8)
                            anchors.verticalCenter: parent.verticalCenter
                            text: entry.label || ""
                            color: parent.ink
                            elide: Text.ElideRight
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }

                        Label {
                            id: gap
                            anchors.right: parent.right
                            anchors.rightMargin: Theme.dp(12)
                            anchors.verticalCenter: parent.verticalCenter
                            text: entry.dt === null || entry.dt === undefined ? "" : "+" + entry.dt + " ms"
                            color: parent.doubled ? Theme.plus : Theme.textMuted
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }
                    }
                }
            }
        }
    }

    Column {
        anchors.top: panel.bottom
        anchors.topMargin: Theme.dp(28)
        anchors.horizontalCenter: panel.horizontalCenter
        width: panel.width
        spacing: Theme.dp(10)
        visible: !page.testing

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: !page.walking
            text: page.controller.connected ? "Connected" : "Connect a controller"
            color: Theme.textMuted
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Repeater {
            model: page.walking ? [] : page.controller.devices

            Row {
                readonly property var battery: {
                    var sources = api.power.sources;
                    return sources.find(function (s) {
                        return s.inputs.indexOf(modelData.id) >= 0;
                    }) || null;
                }

                anchors.horizontalCenter: parent.horizontalCenter
                spacing: Theme.dp(14)

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(12)
                    height: width
                    radius: width / 2
                    color: modelData.id === page.controller.current ? Theme.okGreen : Theme.textDisabled
                }

                Label {
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.name + (modelData.bus ? " · " + (modelData.bus === "bluetooth" ? "Bluetooth" : modelData.bus === "usb" ? "USB" : modelData.bus) : "") + (parent.battery ? " · " + parent.battery.percent + "%" + (parent.battery.charging ? ", charging" : "") : "")
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }
            }
        }

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: page.controller.connected && page.learning && !page.walking
            text: "Press the button on the controller"
            color: Theme.accent
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            visible: page.walking
            text: page.walking ? page.step.prompt || "" : ""
            wrapMode: Text.WordWrap
            font.weight: Font.Light
            font.pixelSize: Theme.dp(44)
        }

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: page.walking
            text: page.walking && page.step.index !== undefined ? "Step " + page.step.index + " of " + page.step.count + " · skipped in " + page.step.seconds + " s · " + (page.step.back ? "← back · " : "") + "B stops" : ""
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            visible: page.controller.connected && !page.learning && page.controller.unboundSlots.length > 0
            text: "Dashed buttons have no code on this connection: learn them"
            color: Theme.textMuted
            wrapMode: Text.WordWrap
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }

    FocusScope {
        id: tester

        anchors.fill: panel
        focus: page.testing

        Keys.onPressed: function (event) {
            event.accepted = true;
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                Sound.play("back");
                page.controller.setTesting(false);
            }
        }
        Keys.onReleased: function (event) {
            event.accepted = true;
        }
    }
}
