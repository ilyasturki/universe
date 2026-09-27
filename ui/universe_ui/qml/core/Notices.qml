pragma Singleton
import QtQuick

// One message on screen, at most one waiting: a newer one takes the waiting place unless an info would bury an error; a keyed one replaces its follow-up anywhere.
QtObject {
    id: n

    // { text, error, key }; null while nothing is shown.
    property var current: null
    property var waiting: []
    property int infoMs: 4000
    property int errorMs: 8000

    // The last message's, kept after it goes so a view fades out with its words.
    property string text: ""
    property bool error: false
    onCurrentChanged: {
        if (current) {
            text = current.text;
            error = current.error;
        }
    }

    function show(text, key) {
        post(text, false, key);
    }

    function fail(text, key) {
        post(text, true, key);
    }

    function post(text, error, key) {
        if (!text)
            return;
        var msg = {
            text: text,
            error: error,
            key: key || ""
        };
        if (msg.key && current && current.key === msg.key) {
            current = msg;
            hold();
            return;
        }
        for (var i = 0; msg.key && i < waiting.length; i++)
            if (waiting[i].key === msg.key) {
                var w = waiting.slice();
                w[i] = msg;
                waiting = w;
                return;
            }
        var last = waiting.length ? waiting[0] : current;
        if (last && last.text === msg.text && last.error === msg.error) {
            if (last === current)
                hold();
            return;
        }
        if (waiting.length && waiting[0].error && !msg.error)
            return;
        waiting = [msg];
        if (!current && !n.gap.running)
            next();
    }

    function next() {
        current = waiting.length ? waiting[0] : null;
        waiting = waiting.slice(1);
        if (current)
            hold();
    }

    function hold() {
        n.shown.interval = current.error ? n.errorMs : n.infoMs;
        n.shown.restart();
    }

    readonly property Timer shown: Timer {
        onTriggered: {
            n.current = null;
            if (n.waiting.length)
                n.gap.restart();
        }
    }

    // Long enough for the slot to fade out, so the next message reads as a new one.
    readonly property Timer gap: Timer {
        interval: 320
        onTriggered: n.next()
    }
}
