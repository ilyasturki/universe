import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    focus: true

    property var args: ({})
    readonly property var game: args.game || null
    readonly property string slot: args.slot || ""
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
    readonly property int columns: currentSlot === "box_front" || currentSlot === "square" ? 4 : currentSlot === "background" ? 2 : 3
    readonly property int candCount: candidates.length + (form.more ? 1 : 0)

    signal closeRequested()
    signal message(string text)

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
        if (current && current.hasOverride)
            out.push({ glyph: "X", label: "Remove override" });
        else if (current && current.kind === "missing")
            out.push({ glyph: "X", label: "Fetch missing" });
        out.push({ glyph: "Y", label: "Change entry" });
        out.push({ glyph: "dpad", label: "Navigate" });
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
    Component.onDestruction: form.unload()
    onSlotsChanged: if (index >= slots.length) index = Math.max(0, slots.length - 1)

    function landOnSlot() {
        var i = slots.findIndex(function(s) { return s.slot === slot; });
        if (i >= 0)
            index = i;
    }

    onCurrentSlotChanged: {
        candIndex = 0;
        if (currentSlot !== "")
            form.loadCandidates(currentSlot);
    }

    function step(d) {
        index = Sound.stepped(index, d, slots.length);
    }

    function stepCand(d) {
        candIndex = Sound.stepped(candIndex, d, candCount);
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
        Sound.enter();
        candIndex >= candidates.length ? form.moreCandidates() : form.apply(currentSlot, candidates[candIndex].url);
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

    function leaveHits() {
        hitsPanel.open = false;
        zone = "slots";
    }

    Connections {
        target: page.form
        function onMessage(text) { page.message(text); }
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
                    leaveHits();
                }
            } else if (api.keys.isCancel(event)) {
                Sound.cancel();
                leaveHits();
            } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                page.hitIndex = Sound.stepped(page.hitIndex, event.key === Qt.Key_Up ? -1 : 1, hits.length);
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
        detail: page.form.entry !== "" ? "SteamGridDB · " + page.form.entry : ""
    }

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
                        text: modelData.kind === "picked" ? "Your pick" + (modelData.hasDefault ? " over the default" : "")
                            : modelData.kind === "default" ? "Default" + (modelData.originLabel !== "" ? " from " + modelData.originLabel : "")
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
                        : "Showing the default" + (page.current.originLabel !== "" ? " from " + page.current.originLabel : "")
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Bold
                    font.pixelSize: Theme.dp(27)
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: page.current ? "Shows on: " + page.current.use : ""
                    color: Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(20)
                    elide: Text.ElideRight
                }

                Text {
                    width: parent.width
                    text: page.current === null ? ""
                        : page.current.hasOverride ? (page.current.hasDefault ? "X takes the pick off and shows the default under it again." : "X takes the pick off; nothing under it.")
                        : page.current.kind === "missing" ? "Pick a candidate, or press X to fetch what SteamGridDB has."
                        : "A pick goes over the default; the default stays under it for when the pick comes off."
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
                    kind: "default"
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
                visible: page.form.entry !== ""
                text: page.form.entry
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(20)
                elide: Text.ElideRight
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: page.form.candidatesBusy && page.candidates.length === 0 ? "fetching…"
                    : page.candidates.length === 0 ? (page.form.sgdbId > 0 ? "nothing for this slot" : "no match — Y searches by name")
                    : page.candidates.length + (page.form.more ? "+" : "") + " for " + (page.current ? page.current.label.toLowerCase() : "") + " · Y if this is the wrong game"
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
