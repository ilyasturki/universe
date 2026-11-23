import QtQuick

// Keys pressed while `holding` wait here for `release()`: a page still incubating takes them before its loader has wired it.
// With `passTabKeys`, LB and RB leave at once, unless a key already waits: then they wait behind it, in order.
FocusScope {
    id: hold

    property bool holding: false
    property bool passTabKeys: false
    property var held: []
    // Until the replay a tick later, behind what the page put off to then (SettingsPage.open's cursor): new keys queue behind.
    property bool releasing: false
    readonly property bool taking: holding || releasing
    // Takes the focus for the replay, unless something took it meanwhile.
    property Item target: null

    function owes() {
        return taking || held.length > 0;
    }

    function release() {
        if (held.length === 0 || releasing)
            return;
        releasing = true;
        Qt.callLater(hold.replay);
    }

    function replay() {
        var keys = held;
        held = [];
        releasing = false;
        if (activeFocus && target && !target.activeFocus)
            target.forceActiveFocus();
        for (var i = 0; i < keys.length; i++)
            api.keys.replay(keys[i].key, keys[i].pressed, keys[i].text, keys[i].modifiers);
    }

    function take(event, pressed) {
        if (!taking || passTabKeys && held.length === 0 && (api.keys.isPrevPage(event) || api.keys.isNextPage(event))) {
            event.accepted = false;
            return;
        }
        // A release whose press went elsewhere (the A that closed a question over this page) is dropped, not queued ahead of RB.
        if (!event.isAutoRepeat && (pressed || held.some(k => k.pressed && k.key === event.key)))
            held.push({
                key: event.key,
                pressed: pressed,
                text: event.text,
                modifiers: event.modifiers
            });
        event.accepted = true;
    }

    onActiveFocusChanged: {
        if (!activeFocus && !releasing)
            held = [];
    }

    Keys.onPressed: event => take(event, true)
    Keys.onReleased: event => take(event, false)
}
