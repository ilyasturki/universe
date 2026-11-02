import QtQuick

// Keys pressed while `holding` wait here for `release()`: a page still incubating takes them before its loader has wired it.
FocusScope {
    id: hold

    property bool holding: false
    property var held: []

    function release() {
        var keys = held;
        held = [];
        for (var i = 0; i < keys.length; i++)
            api.keys.replay(keys[i].key, keys[i].pressed, keys[i].text, keys[i].modifiers);
    }

    function take(event, pressed) {
        if (!holding)
            return;
        if (!event.isAutoRepeat)
            held.push({
                key: event.key,
                pressed: pressed,
                text: event.text,
                modifiers: event.modifiers
            });
        event.accepted = true;
    }

    onActiveFocusChanged: {
        if (!activeFocus)
            held = [];
    }

    Keys.onPressed: event => take(event, true)
    Keys.onReleased: event => take(event, false)
}
