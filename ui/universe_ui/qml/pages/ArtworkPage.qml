import QtQuick
import "../core"
import "../sound"
import "../ui"

// One game's artwork: the five slots on the left, each with what shows and where it came from;
// on the right the focused slot large, the fetched default under a pick, and SteamGridDB's
// candidates for it. A on a candidate downloads it over the slot as an override; X takes an
// override off again (the default under it comes back) or fetches what is missing; Y searches
// SteamGridDB by name and pins the game to the right entry when the match was wrong.
FocusScope {
    id: page

    focus: true

    property var game: null
    // The slot to land on, set by the shell when the overview opens the page.
    property string slot: ""
    readonly property var form: api.screens.artwork
    readonly property var slots: form.slots
    property int index: 0
    readonly property var current: index >= 0 && index < slots.length ? slots[index] : null
    readonly property string currentSlot: current ? current.slot : ""
    readonly property var candidates: form.candidatesSlot === currentSlot ? form.candidates : []
    property int candIndex: 0
    // "slots" | "cands" | "hits"
    property string zone: "slots"
    property int hitIndex: 0
    readonly property bool inCands: zone === "cands"
    readonly property bool inHits: zone === "hits"
    // Columns of the candidates grid, so a cover stays a cover and a hero stays wide.
    readonly property int columns: currentSlot === "box_front" || currentSlot === "square" ? 4 : currentSlot === "background" ? 2 : 3
    // The "more" tile stands past the last candidate.
    readonly property int candCount: candidates.length + (form.more ? 1 : 0)

    signal closeRequested()

    readonly property var hints: {
        if (keyboard.open)
            return keyboard.hints;
        if (inHits)
            return [ { glyph: "A", label: "Use this game" }, { glyph: "dpad", label: "Navigate" }, { glyph: "B", label: "Close" } ];
        var out = [];
        if (inCands)
            out.push({ glyph: "A", label: candIndex < candidates.length ? "Pick" : "Load more" });
        else if (candidates.length > 0)
            out.push({ glyph: "A", label: "Candidates" });
        out.push({ glyph: "dpad", label: "Navigate" });
        if (current && current.hasOverride)
            out.push({ glyph: "X", label: "Remove override" });
        else if (current && current.kind === "missing")
            out.push({ glyph: "X", label: "Fetch missing" });
        out.push({ glyph: "Y", label: "Wrong game?" });
        out.push({ glyph: "B", label: inCands ? "Back to slots" : "Back" });
        return out;
    }

    readonly property real sideMargin: Theme.dp(90)
    readonly property real listWidth: Theme.dp(600)
    readonly property real gap: Theme.dp(48)

    onGameChanged: {
        index = 0;
        candIndex = 0;
        zone = "slots";
        hitsPanel.open = false;
        if (game) {
            form.load(game.id);
            landOnSlot();
        }
    }

    onSlotChanged: landOnSlot()
    // Off screen the form stops following the library.
    Component.onDestruction: form.unload()
    onSlotsChanged: if (index >= slots.length) index = Math.max(0, slots.length - 1)

    function landOnSlot() {
        for (var i = 0; i < slots.length; i++)
            if (slots[i].slot === slot) {
                index = i;
                return;
            }
    }

    onCurrentSlotChanged: {
        candIndex = 0;
        if (currentSlot !== "")
            form.loadCandidates(currentSlot);
    }

    function step(d) {
        var next = Math.max(0, Math.min(slots.length - 1, index + d));
        next === index ? Sound.edge() : Sound.tick();
        index = next;
    }

    function stepCand(d) {
        var next = Math.max(0, Math.min(candCount - 1, candIndex + d));
        next === candIndex ? Sound.edge() : Sound.tick();
        candIndex = next;
    }

    function enterCands() {
        if (candCount === 0) {
            Sound.edge();
            return;
        }
        Sound.panel();
        zone = "cands";
        candIndex = Math.min(candIndex, candCount - 1);
    }

    function pick() {
        if (candIndex >= candidates.length) {
            Sound.enter();
            form.moreCandidates();
            return;
        }
        Sound.enter();
        form.apply(currentSlot, candidates[candIndex].url);
    }

    function removeOrFetch() {
        if (!current)
            return;
        if (current.hasOverride) {
            Sound.cancel();
            form.removeOverride(current.slot);
        } else if (current.kind === "missing") {
            Sound.enter();
            form.refresh();
        } else {
            Sound.edge();
        }
    }

    function askWrongGame() {
        Sound.panel();
        keyboard.show("Search SteamGridDB", form.title, "text");
    }

    Connections {
        target: page.form
        function onMessage(text) { toast.show(text); }
        function onHitsChanged() {
            if (!page.form.searchBusy && hitsPanel.pending) {
                hitsPanel.pending = false;
                hitsPanel.open = true;
                page.hitIndex = 0;
                page.zone = "hits";
            }
        }
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat && (api.keys.isAccept(event) || api.keys.isCancel(event)))
            return;
        if (keyboard.open)
            return;
        if (page.inHits) {
            event.accepted = true;
            var hits = page.form.hits;
            if (api.keys.isAccept(event)) {
                if (hits.length > 0) {
                    Sound.enter();
                    page.form.pin(hits[page.hitIndex].id);
                    hitsPanel.open = false;
                    page.zone = "slots";
                }
            } else if (api.keys.isCancel(event)) {
                Sound.cancel();
                hitsPanel.open = false;
                page.zone = "slots";
            } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                var n = Math.max(0, Math.min(hits.length - 1, page.hitIndex + (event.key === Qt.Key_Up ? -1 : 1)));
                n === page.hitIndex ? Sound.edge() : Sound.tick();
                page.hitIndex = n;
            }
            return;
        }
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            page.inCands ? pick() : enterCands();
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            if (page.inCands) {
                Sound.cancel();
                page.zone = "slots";
            } else {
                page.closeRequested();
            }
        } else if (api.keys.isDetails(event)) {
            event.accepted = true;
            removeOrFetch();
        } else if (api.keys.isFilters(event)) {
            event.accepted = true;
            askWrongGame();
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            event.accepted = true;
            var d = event.key === Qt.Key_Up ? -1 : 1;
            page.inCands ? stepCand(d * page.columns) : step(d);
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            event.accepted = true;
            if (page.inCands) {
                if (event.key === Qt.Key_Left && page.candIndex % page.columns === 0) {
                    Sound.panel();
                    page.zone = "slots";
                } else {
                    stepCand(event.key === Qt.Key_Left ? -1 : 1);
                }
            } else if (event.key === Qt.Key_Right) {
                enterCands();
            } else {
                Sound.edge();
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.ground
    }

    GameBackdrop {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        game: page.game
    }

    GameHeader {
        id: header

        anchors.top: parent.top
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: page.sideMargin
        anchors.rightMargin: page.sideMargin
        game: page.game
        label: "ARTWORK"
        detail: page.form.sgdbId > 0 ? "SteamGridDB #" + page.form.sgdbId : ""
    }

    // -- the slots ---------------------------------------------------------------------------

    Column {
        id: slotList

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(32)
        anchors.left: parent.left
        anchors.leftMargin: page.sideMargin
        width: page.listWidth
        spacing: Theme.dp(12)
        opacity: page.inCands || page.inHits ? 0.55 : 1.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        Repeater {
            model: page.slots

            Rectangle {
                readonly property bool focused: index === page.index
                readonly property bool lit: focused && page.zone === "slots"
                readonly property real thumbHeight: Theme.dp(74)

                width: slotList.width
                height: Theme.dp(104)
                radius: Theme.dp(16)
                color: lit ? Theme.text : Theme.surface

                Behavior on color {
                    ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
                }

                Loader {
                    anchors.fill: parent
                    active: focused
                    sourceComponent: FocusRing { cornerRadius: Theme.dp(16) }
                }

                RoundedMask {
                    id: thumb
                    x: Theme.dp(15)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(Theme.dp(150), thumbHeight * modelData.aspect)
                    height: thumbHeight
                    radius: Theme.dp(8)

                    Rectangle {
                        anchors.fill: parent
                        color: lit ? Qt.rgba(0, 0, 0, 0.12) : Qt.rgba(1, 1, 1, 0.06)
                    }

                    Image {
                        anchors.fill: parent
                        source: modelData.url
                        fillMode: modelData.slot === "logo" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
                        asynchronous: true
                        cache: false
                        sourceSize.width: 400
                    }
                }

                Column {
                    anchors.left: thumb.right
                    anchors.leftMargin: Theme.dp(20)
                    anchors.right: badge.left
                    anchors.rightMargin: Theme.dp(16)
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Theme.dp(5)

                    Text {
                        width: parent.width
                        text: modelData.label
                        color: lit ? Theme.onLight : Theme.text
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.pixelSize: Theme.dp(24)
                        elide: Text.ElideRight
                    }

                    Text {
                        width: parent.width
                        text: modelData.kind === "picked" ? "Your pick" + (modelData.hasDefault ? " · a default under it" : "")
                            : modelData.kind === "fetched" ? "From " + modelData.originLabel
                            : modelData.kind === "guessed" ? "On disk, source unrecorded"
                            : "Nothing yet"
                        color: lit ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textSecondary
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(19)
                        elide: Text.ElideRight
                    }
                }

                KindBadge {
                    id: badge
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.dp(18)
                    anchors.verticalCenter: parent.verticalCenter
                    kind: modelData.kind
                    label: modelData.kindLabel
                    onLight: lit
                }
            }
        }
    }

    // The screenshots, to see: what the journal and the detail page show, and where they came from.
    Item {
        id: shotsStrip

        anchors.top: slotList.bottom
        anchors.topMargin: Theme.dp(28)
        anchors.left: slotList.left
        width: slotList.width
        height: shotsHead.height + Theme.dp(12) + shotsRow.height
        visible: y + height < hintBar.y
        opacity: slotList.opacity

        Row {
            id: shotsHead
            spacing: Theme.dp(14)

            CapsLabel {
                text: "SCREENSHOTS"
                anchors.verticalCenter: parent.verticalCenter
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: page.form.shots.count > 0
                    ? page.form.shots.count + (page.form.shots.kind === "picked" ? " · " + page.form.shots.overrideCount + " yours" : page.form.shots.originLabel !== "" ? " from " + page.form.shots.originLabel : "")
                    : "none"
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(19)
            }
        }

        Row {
            id: shotsRow
            anchors.top: shotsHead.bottom
            anchors.topMargin: Theme.dp(12)
            spacing: Theme.dp(10)
            height: Theme.dp(72)

            Repeater {
                model: page.form.screenshots.slice(0, 4)

                RoundedMask {
                    width: Theme.dp(128)
                    height: Theme.dp(72)
                    radius: Theme.dp(8)

                    Image {
                        anchors.fill: parent
                        source: modelData
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        sourceSize.width: 300
                    }
                }
            }
        }
    }

    // -- the dock: the slot large, then its candidates --------------------------------------

    Item {
        id: dock

        anchors.top: slotList.top
        anchors.left: slotList.right
        anchors.leftMargin: page.gap
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        anchors.bottom: hintBar.top
        anchors.bottomMargin: Theme.dp(16)

        readonly property real previewHeight: Theme.dp(300)

        // What shows now, and beside it the default a pick sits over.
        Item {
            id: preview

            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: dock.previewHeight

            ArtFrame {
                id: nowFrame
                anchors.top: parent.top
                anchors.left: parent.left
                height: parent.height
                width: Math.min(parent.width * 0.55, height * (page.current ? page.current.aspect : 1))
                source: page.current ? page.current.url : ""
                logo: page.currentSlot === "logo"
                caption: page.current ? page.current.label : ""
                kind: page.current ? page.current.kind : "missing"
                kindLabel: page.current ? page.current.kindLabel : ""
            }

            Column {
                anchors.left: nowFrame.right
                anchors.leftMargin: Theme.dp(32)
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.topMargin: Theme.dp(4)
                spacing: Theme.dp(10)

                Text {
                    width: parent.width
                    text: page.current === null ? ""
                        : page.current.kind === "picked" ? "Showing your pick"
                        : page.current.kind === "missing" ? "Nothing in this slot"
                        : page.current.kind === "guessed" ? "Showing what is on disk"
                        : "Showing the " + page.current.originLabel + " default"
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Bold
                    font.pixelSize: Theme.dp(27)
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: page.current === null ? ""
                        : page.current.hasOverride ? (page.current.hasDefault ? "X takes the pick off and shows the default under it again." : "X takes the pick off; nothing was fetched under it.")
                        : page.current.kind === "missing" ? "Pick a candidate, or press X to fetch what SteamGridDB has."
                        : "Pick a candidate to put your own over it; the default stays on disk under the pick."
                    color: Theme.textMuted
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(20)
                    wrapMode: Text.WordWrap
                }

                ArtFrame {
                    height: preview.height - y - Theme.dp(4)
                    width: Math.min(parent.width, (height - captionHeight) * (page.current ? page.current.aspect : 1))
                    visible: page.current !== null && page.current.hasOverride && page.current.hasDefault
                    source: page.current ? page.current.defaultUrl : ""
                    logo: page.currentSlot === "logo"
                    caption: "Default"
                    kind: "fetched"
                    kindLabel: page.current ? page.current.defaultOriginLabel : ""
                    dim: true
                }
            }
        }

        Row {
            id: candHead

            anchors.top: preview.bottom
            anchors.topMargin: Theme.dp(30)
            spacing: Theme.dp(14)

            CapsLabel {
                text: "STEAMGRIDDB"
                anchors.verticalCenter: parent.verticalCenter
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: page.form.candidatesBusy && page.candidates.length === 0 ? "fetching…"
                    : page.candidates.length === 0 ? (page.form.sgdbId > 0 ? "nothing for this slot" : "no match — press Y to search")
                    : page.candidates.length + (page.form.more ? "+" : "") + " for " + (page.current ? page.current.label.toLowerCase() : "")
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(19)
            }
        }

        GridView {
            id: grid

            anchors.top: candHead.bottom
            anchors.topMargin: Theme.dp(14)
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            clip: true
            interactive: false
            model: page.candCount
            currentIndex: page.inCands ? page.candIndex : -1
            cellWidth: Math.floor(width / page.columns)
            cellHeight: Math.round(cellWidth / (page.current ? page.current.aspect : 1)) + Theme.dp(20)
            // The focused row sits at the top: no half row peeking over it.
            preferredHighlightBegin: 0
            preferredHighlightEnd: cellHeight
            highlightRangeMode: GridView.ApplyRange
            highlightFollowsCurrentItem: true
            opacity: page.inHits ? 0.4 : 1.0

            delegate: Item {
                readonly property bool isMore: index >= page.candidates.length
                readonly property var cand: isMore ? null : page.candidates[index]
                readonly property bool focused: page.inCands && index === page.candIndex

                width: grid.cellWidth
                height: grid.cellHeight

                Item {
                    id: cell
                    anchors.fill: parent
                    anchors.margins: Theme.dp(10)
                    scale: focused ? 1.03 : 1.0

                    Behavior on scale {
                        NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
                    }

                    Loader {
                        anchors.fill: parent
                        active: focused
                        sourceComponent: FocusRing { cornerRadius: Theme.dp(12) }
                    }

                    RoundedMask {
                        anchors.fill: parent
                        radius: Theme.dp(12)

                        Rectangle {
                            anchors.fill: parent
                            color: Theme.surface
                        }

                        Image {
                            anchors.fill: parent
                            visible: !isMore
                            source: cand ? cand.thumb : ""
                            fillMode: page.currentSlot === "logo" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
                            asynchronous: true
                            sourceSize.width: 640
                        }

                        Column {
                            anchors.centerIn: parent
                            visible: isMore
                            spacing: Theme.dp(6)

                            Text {
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: page.form.candidatesBusy ? "…" : "+"
                                color: Theme.text
                                font.family: Theme.sans
                                font.weight: Font.Bold
                                font.pixelSize: Theme.dp(44)
                            }

                            Text {
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: "more"
                                color: Theme.textSecondary
                                font.family: Theme.sans
                                font.pixelSize: Theme.dp(19)
                            }
                        }

                        Rectangle {
                            anchors.left: parent.left
                            anchors.bottom: parent.bottom
                            anchors.margins: Theme.dp(8)
                            visible: !isMore && cand && cand.votes > 0
                            width: votes.width + Theme.dp(16)
                            height: Theme.dp(26)
                            radius: height / 2
                            color: Qt.rgba(0, 0, 0, 0.6)

                            Text {
                                id: votes
                                anchors.centerIn: parent
                                text: cand ? "▲ " + cand.votes : ""
                                color: Theme.text
                                font.family: Theme.sans
                                font.weight: Font.DemiBold
                                font.pixelSize: Theme.dp(15)
                            }
                        }
                    }
                }
            }
        }
    }

    // -- the wrong game: SteamGridDB's entries for a name, one to pin ------------------------

    Item {
        id: hitsPanel

        property bool open: false
        // Set when the keyboard is done, cleared when the hits arrive.
        property bool pending: false

        anchors.fill: dock
        visible: opacity > 0.01
        opacity: open ? 1.0 : 0.0
        z: 2

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(24)
            color: "#181920"
            border.width: 1
            border.color: Theme.surfaceBorder
        }

        Column {
            anchors.fill: parent
            anchors.margins: Theme.dp(28)
            spacing: Theme.dp(18)

            Text {
                text: "Which game is it on SteamGridDB?"
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(30)
            }

            Text {
                width: parent.width
                text: page.form.searchError !== "" ? page.form.searchError
                    : page.form.hits.length === 0 ? "Nothing matched — press Y to search with another name."
                    : "The candidates and the next refresh follow the one you pick."
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(21)
                wrapMode: Text.WordWrap
            }

            ListView {
                id: hitsList
                width: parent.width
                height: parent.height - y
                model: page.form.hits
                currentIndex: page.hitIndex
                interactive: false
                clip: true
                spacing: Theme.dp(10)
                preferredHighlightBegin: 0
                preferredHighlightEnd: height
                highlightRangeMode: ListView.ApplyRange
                highlightFollowsCurrentItem: true

                delegate: Rectangle {
                    readonly property bool lit: index === page.hitIndex && page.inHits

                    width: hitsList.width
                    height: Theme.dp(78)
                    radius: Theme.dp(14)
                    color: lit ? Theme.text : Theme.surface

                    Loader {
                        anchors.fill: parent
                        active: lit
                        sourceComponent: FocusRing { cornerRadius: Theme.dp(14) }
                    }

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: Theme.dp(22)
                        anchors.right: hitMeta.left
                        anchors.rightMargin: Theme.dp(16)
                        anchors.verticalCenter: parent.verticalCenter
                        text: modelData.name
                        color: lit ? Theme.onLight : Theme.text
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.pixelSize: Theme.dp(24)
                        elide: Text.ElideRight
                    }

                    Text {
                        id: hitMeta
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(22)
                        anchors.verticalCenter: parent.verticalCenter
                        text: (modelData.year > 0 ? modelData.year + "  " : "") + (modelData.verified ? "✓  " : "") + (modelData.current ? "current  " : "") + "#" + modelData.id
                        color: lit ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textMuted
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(19)
                    }
                }
            }
        }
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        z: 3
        sideMargin: page.sideMargin
        showClock: true
        hints: page.hints
    }

    Toast {
        id: toast
        z: 4
    }

    KeyboardSheet {
        id: keyboard
        anchors.fill: parent
        z: 2

        onAccepted: function(value) {
            hitsPanel.pending = true;
            page.form.search(value);
            page.forceActiveFocus();
        }
        onDismissed: page.forceActiveFocus()
    }
}
