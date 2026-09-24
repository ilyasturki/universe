import QtQuick
import "../sound"

// A question in the middle of the screen: the pop-up list with the two answers, no and yes. `done(yes)` hears either;
// B is no, and a hold of B that asked it asks nothing more.
ActionMenu {
    id: dialog

    readonly property string message: title
    property var pending: null

    signal closed

    function ask(spec, done) {
        pending = done || null;
        Sound.panel();
        show([
            {
                label: spec.no || "Cancel",
                action: "no"
            },
            {
                label: spec.yes || "OK",
                action: "yes"
            }
        ], null, Qt.rect(0, 0, 0, 0), spec.message || "", function (action) {
            action === "yes" ? Sound.enter() : Sound.cancel();
            answer(action === "yes");
        }, spec.index !== undefined ? spec.index : 1);
        note = spec.detail || "";
        asking = true;
    }

    function answer(yes) {
        var cb = pending;
        pending = null;
        closed();
        if (cb)
            cb(yes);
    }

    onDismissed: answer(false)

    Keys.onReleased: function (event) {
        event.accepted = true;
    }
}
