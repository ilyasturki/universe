import QtQuick

// One cover decoding at a time, in reading order: Qt's image reader takes the newest request first, so a view asking for all at once fills from its far end.
QtObject {
    id: queue

    // The cards inside it go first, top-left onwards; the ones in its cache buffer after.
    property Item view: null
    property var waiting: []
    property Item holder: null

    onHolderChanged: pump()

    function request(card) {
        if (card !== holder && waiting.indexOf(card) < 0)
            waiting.push(card);
        pump();
    }

    function release(card) {
        var at = waiting.indexOf(card);
        if (at >= 0)
            waiting.splice(at, 1);
        if (holder === card)
            holder = null;
    }

    function rank(card) {
        var p = card.mapToItem(view, 0, 0);
        var shown = p.x + card.width > 0 && p.x < view.width && p.y + card.height > 0 && p.y < view.height;
        return [shown ? 0 : 1, Math.round(p.y), Math.round(p.x)];
    }

    function before(a, b) {
        for (var i = 0; i < a.length; i++)
            if (a[i] !== b[i])
                return a[i] < b[i];
        return false;
    }

    function pump() {
        if (holder !== null || waiting.length === 0)
            return;
        var best = 0, bestRank = view ? rank(waiting[0]) : null;
        for (var i = 1; view && i < waiting.length; i++) {
            var r = rank(waiting[i]);
            if (before(r, bestRank)) {
                best = i;
                bestRank = r;
            }
        }
        holder = waiting.splice(best, 1)[0];
        holder.granted = true;
    }
}
