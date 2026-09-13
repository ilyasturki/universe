import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    property var shell: null
    readonly property var controller: api.screens.controller

    readonly property bool testing: controller.testing
    readonly property bool learning: controller.learning !== ""
    property var pressed: ({})
    property var axes: ({})
    property string lastSlot: ""
    property var held: ({})

    readonly property var hints: testing
        ? [ { glyph: "B", label: "Hold to finish" }, { glyph: "Start", label: "" }, { glyph: "Select", label: "Finish" } ]
        : learning
        ? [ { glyph: "B", label: "Stop learning" } ]
        : [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]

    signal closeRequested()

    focus: true

    readonly property var entries: {
        var out = [], source = controller.rows, buttons = [];
        for (var i = 0; i < source.length; i++) {
            var r = source[i];
            if (r.key === "test")
                out.push({ key: "test", label: r.label, type: "action", display: "", detail: r.display, icon: "gamepad" });
            else if (r.key === "device")
                out.push({ key: "device", label: r.label, type: "enum", display: r.display, choices: r.choices, value: r.value });
            else if (r.type === "info")
                out.push({ key: "", label: r.label, type: "note", display: "", detail: r.detail, disabled: true });
            else if (r.slot !== undefined)
                buttons.push({ key: r.slot, label: r.label, type: "action", display: "", detail: r.display, slot: r.slot, family: r.family,
                               bound: r.bound, press: r.press, hold: r.hold });
        }
        if (buttons.length > 0) {
            out.push({ heading: true, label: "Buttons", display: "" });
            out = out.concat(buttons);
        }
        return out;
    }

    readonly property string focusedSlot: rows.cursorShown && rows.currentRow && rows.currentRow.slot ? rows.currentRow.slot : ""

    function labelOf(slot) {
        var hit = controller.rows.filter(function(r) { return r.slot === slot; })[0];
        return hit ? hit.label : slot;
    }

    function toggled(set, key, on) {
        var next = {};
        for (var k in set)
            if (k !== key)
                next[k] = true;
        if (on)
            next[key] = true;
        return next;
    }

    function press(slot, down) {
        pressed = toggled(pressed, slot, down);
        if (down)
            lastSlot = slot;
    }

    function axis(name, value) {
        var next = {};
        for (var k in axes)
            next[k] = axes[k];
        next[name] = value;
        axes = next;
    }

    function clearArt() {
        pressed = ({});
        axes = ({});
        lastSlot = "";
        held = ({});
        holdOut.stop();
    }

    function activate(index, row) {
        if (row.key === "test") {
            if (controller.setTesting(true))
                Sound.ok();
            else
                Sound.edge();
        } else if (row.key === "device") {
            Sound.ok();
            var choices = row.choices || [];
            shell.pick({ title: "Controller", choices: choices, index: choices.indexOf(String(row.value)) }, function(i) {
                if (i >= 0)
                    controller.setValue(index, choices[i]);
            });
        } else if (row.slot !== undefined) {
            Sound.ok();
            slotMenu(row);
        } else {
            Sound.edge();
        }
    }

    function slotMenu(row) {
        var items = row.bound ? [] : [{ label: "Learn the button", act: "learn" }];
        items.push({ label: "On press…", act: "press" }, { label: "On hold…", act: "hold" });
        if (row.press)
            items.push({ label: "Clear press", act: "clear-press" });
        if (row.hold)
            items.push({ label: "Clear hold", act: "clear-hold" });
        if (row.bound)
            items.push({ label: "Learn the button again", act: "learn" });
        shell.pick({ title: row.label, choices: items.map(function(i) { return i.label; }), index: 0 }, function(i) {
            if (i >= 0)
                slotAction(row, items[i].act);
        });
    }

    function slotAction(row, action) {
        if (action === "learn") {
            if (controller.learn(row.slot)) {
                Sound.ok();
                shell.showToast("Press the button on the controller…");
            }
        } else if (action === "press" || action === "hold") {
            presetMenu(row, action);
        } else if (action === "clear-press") {
            Sound.select();
            controller.unbind(row.slot, "press");
        } else if (action === "clear-hold") {
            Sound.select();
            controller.unbind(row.slot, "hold");
        }
    }

    function presetMenu(row, trigger) {
        var items = controller.presets.filter(function(p) { return !(p.hold_only && trigger !== "hold") && p.id !== "keys" && p.id !== "command"; })
            .map(function(p) { return { label: p.label, id: "preset:" + p.id }; });
        items.push({ label: "Key combo…", id: "keys" }, { label: "Command…", id: "command" });
        var labels = items.map(function(i) { return i.label; }), ids = items.map(function(i) { return i.id; });
        var current = trigger === "hold" ? row.hold : row.press;
        var index = current ? ids.indexOf(current.action === "keys" || current.action === "command" ? current.action : "preset:" + current.action) : 0;
        shell.pick({ title: row.label + " · " + (trigger === "press" ? "On press" : "On hold"), choices: labels, index: Math.max(0, index) }, function(i) {
            if (i < 0)
                return;
            var id = ids[i];
            if (id.indexOf("preset:") === 0) {
                Sound.select();
                controller.bind(row.slot, trigger, id.substring(7), "", "");
            } else {
                var value = current && current.action === id ? current[id] : "";
                shell.prompt({ title: (id === "keys" ? "Key combo for " : "Command for ") + row.label, value: value, max: 200 }, function(text) {
                    if (text !== null && text !== undefined && text !== "") {
                        Sound.select();
                        controller.bind(row.slot, trigger, id, id === "keys" ? text : "", id === "command" ? text : "");
                    }
                });
            }
        });
    }

    Component.onCompleted: {
        controller.load();
        controller.suspend();
    }
    Component.onDestruction: controller.resume()

    onTestingChanged: {
        clearArt();
        if (testing) {
            tester.forceActiveFocus();
            return;
        }
        var i = entries.map(function(e) { return e.key; }).indexOf("test");
        if (i >= 0)
            rows.index = i;
        rows.forceActiveFocus();
    }

    Timer {
        id: holdOut
        interval: 1000
        onTriggered: {
            Sound.back();
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
            var next = page.toggled(page.held, slot, isDown);
            page.held = next;
            if (slot === "east") {
                if (isDown)
                    holdOut.restart();
                else
                    holdOut.stop();
            }
            if (isDown && next.start && next.select) {
                Sound.back();
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
        function onMessage(text) { page.shell.showToast(text); }
    }

    property real pulse: 0.15
    SequentialAnimation on pulse {
        running: page.learning
        loops: Animation.Infinite
        NumberAnimation { to: 0.55; duration: 500; easing.type: Easing.InOutSine }
        NumberAnimation { to: 0.15; duration: 500; easing.type: Easing.InOutSine }
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
        icon: "controllers"
        title: "Controllers"
    }

    Rectangle {
        id: panel

        x: page.testing ? Theme.dp(Theme.edgeMargin) : Theme.dp(120)
        y: Theme.dp(180)
        width: page.testing ? parent.width - Theme.dp(Theme.edgeMargin) * 2 : Theme.dp(1000)
        height: page.testing ? parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(30) : Theme.dp(560)
        radius: Theme.dp(12)
        color: "#2d2d2d"

        Behavior on x { NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic } }
        Behavior on width { NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic } }
        Behavior on height { NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic } }

        Loader {
            id: art

            anchors.fill: parent
            anchors.margins: Theme.dp(40)
            anchors.bottomMargin: page.testing && page.lastSlot !== "" ? Theme.dp(110) : Theme.dp(40)
            source: "../../ui/PadArt.qml"
            opacity: page.controller.connected ? 1.0 : 0.38

            Behavior on opacity {
                NumberAnimation { duration: Theme.durFade; easing.type: Easing.OutCubic }
            }

            onLoaded: {
                item.family = Qt.binding(function() { return page.controller.family; });
                item.focusedSlot = Qt.binding(function() { return page.focusedSlot; });
                item.learningSlot = Qt.binding(function() { return page.controller.learning; });
                item.unbound = Qt.binding(function() { return page.controller.connected ? page.controller.unboundSlots : []; });
                item.pressed = Qt.binding(function() { return page.pressed; });
                item.axes = Qt.binding(function() { return page.axes; });
                item.pulse = Qt.binding(function() { return page.pulse; });
            }
        }

        Row {
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.dp(34)
            anchors.horizontalCenter: parent.horizontalCenter
            visible: page.testing && page.lastSlot !== ""
            spacing: Theme.dp(18)

            Loader {
                anchors.verticalCenter: parent.verticalCenter
                source: "../../ui/PadGlyph.qml"
                onLoaded: {
                    item.family = Qt.binding(function() { return page.controller.family; });
                    item.slot = Qt.binding(function() { return page.lastSlot; });
                    item.unit = Qt.binding(function() { return Theme.dp(48); });
                    item.ink = "#f2f2f2";
                }
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: page.labelOf(page.lastSlot)
                color: "#f2f2f2"
                font.family: Theme.sans
                font.pixelSize: Theme.dp(Theme.fontBody)
            }
        }
    }

    Column {
        anchors.top: panel.bottom
        anchors.topMargin: Theme.dp(30)
        anchors.horizontalCenter: panel.horizontalCenter
        spacing: Theme.dp(8)
        visible: !page.testing

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: page.controller.connected ? "Controllers" : "Connect a controller"
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Repeater {
            model: page.controller.devices

            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: Theme.dp(14)

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(12)
                    height: width
                    radius: Theme.dp(2)
                    color: modelData.id === page.controller.current ? Theme.okGreen : Theme.textDisabled
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.name + (modelData.bus ? " · " + (modelData.bus === "bluetooth" ? "Bluetooth" : modelData.bus === "usb" ? "USB" : modelData.bus) : "")
                    color: Theme.text
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(Theme.fontBody)
                }
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: page.controller.connected && page.learning
            text: "Press the button on the controller"
            color: Theme.accent
            font.family: Theme.sans
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: page.controller.connected && !page.learning && page.controller.unboundSlots.length > 0
            text: "Dashed buttons have no code on this connection: learn them"
            color: Theme.textMuted
            font.family: Theme.sans
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }
    }

    Rectangle {
        x: Theme.dp(1180)
        y: Theme.dp(160)
        width: 1
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        color: Theme.hairline
        visible: !page.testing
    }

    SettingsRows {
        id: rows

        x: Theme.dp(1230)
        y: Theme.dp(170)
        width: Theme.dp(560)
        height: parent.height - y - Theme.dp(Theme.hintBarHeight) - Theme.dp(20)
        focus: !page.testing
        model: page.entries
        opacity: page.testing ? 0.0 : 1.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic }
        }

        onActivated: function(index, row) { page.activate(index, row); }
        onEscapedLeft: Sound.edge()
        onEscapedUp: Sound.edge()

        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event) && page.learning) {
                event.accepted = true;
                Sound.back();
                page.controller.cancelLearn();
            }
        }
    }

    FocusScope {
        id: tester

        anchors.fill: panel
        focus: page.testing

        Keys.onPressed: function(event) {
            event.accepted = true;
            if (event.isAutoRepeat)
                return;
            if (api.keys.isCancel(event)) {
                Sound.back();
                page.controller.setTesting(false);
            }
        }
        Keys.onReleased: function(event) { event.accepted = true; }
    }
}
