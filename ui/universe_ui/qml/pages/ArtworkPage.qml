import QtQuick
import "../core"
import "../sound"
import "../ui"

FocusScope {
    id: page

    objectName: "artworkPage"
    focus: true

    property var args: ({})
    readonly property var game: args.game || null
    readonly property string slot: args.slot || ""
    readonly property var form: api.screens.artwork
    readonly property var slots: form.slots
    property int index: 0
    property int lastTop: 0
    readonly property var current: index >= 0 && index < slots.length ? slots[index] : null
    readonly property string currentSlot: current ? current.slot : ""
    readonly property real currentAspect: current ? current.aspect : 1
    readonly property bool picked: current !== null && current.hasOverride
    property string level: "slots"
    readonly property bool browsing: level === "browser"
    readonly property var candidates: form.candidatesSlot === currentSlot ? form.candidates : []
    property int candIndex: 0
    readonly property int candCount: candidates.length + (form.more ? 1 : 0)
    readonly property int columns: currentAspect > 1.5 ? 4 : 7
    property int hitIndex: 0
    property string typing: "search"
    readonly property bool searching: form.searchBusy
    readonly property bool modal: menu.open || hits.open || keyboard.open || paths.open

    signal closeRequested()
    signal message(string text)

    readonly property var hints: {
        if (menu.open)
            return menu.hints;
        if (keyboard.open)
            return keyboard.hints;
        if (paths.open)
            return paths.hints;
        if (hits.open)
            return hits.hints;
        return [ { glyph: "A", label: browsing ? (candIndex < candidates.length ? "Use this" : "Load more") : "Open", dim: browsing && candCount === 0 },
                 { glyph: "X", label: "Back to default", dim: !picked },
                 { glyph: "Y", label: "Wrong game?" },
                 { glyph: "Start", label: "More" },
                 { glyph: "B", label: "Back" } ];
    }

    readonly property real sideMargin: Theme.dp(90)
    readonly property real topRow: Theme.dp(350)
    readonly property real bottomRow: Theme.dp(260)
    readonly property real cardGap: Theme.dp(40)
    readonly property string entryLine: form.entry !== "" ? "SteamGridDB · " + form.entry : ""

    onGameChanged: {
        index = 0;
        lastTop = 0;
        candIndex = 0;
        level = "slots";
        if (game) {
            form.load(game.id);
            land();
        }
    }

    onSlotChanged: land()
    Component.onDestruction: form.unload()
    onSlotsChanged: if (index >= slots.length) index = Math.max(0, slots.length - 1)
    onSearchingChanged: {
        if (searching)
            return;
        hitIndex = Math.max(0, form.hits.findIndex(function(h) { return h.current; }));
        hits.open = true;
        hits.forceActiveFocus();
    }

    function land() {
        var i = slots.findIndex(function(s) { return s.slot === slot; });
        if (i >= 0) {
            index = i;
            openBrowser(true);
        }
    }

    function openBrowser(quiet) {
        if (!current)
            return;
        if (!quiet)
            Sound.enter();
        candIndex = 0;
        level = "browser";
        form.loadCandidates(currentSlot);
    }

    function closeBrowser() {
        Sound.cancel();
        level = "slots";
    }

    // The cards sit in two rows: box front, square, banner; background, logo.
    readonly property var rowOf: [0, 0, 0, 1, 1]

    function moveAcross(d) {
        var next = index + d;
        if (next < 0 || next >= slots.length || rowOf[next] !== rowOf[index]) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
    }

    function moveDown() {
        if (rowOf[index] === 1) {
            Sound.edge();
            return;
        }
        Sound.tick();
        lastTop = index;
        index = index === 2 ? 4 : 3;
    }

    function moveUp() {
        if (rowOf[index] === 0) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = index === 4 ? 2 : (lastTop <= 1 ? lastTop : 0);
    }

    function stepCand(d) {
        var next = candIndex + d;
        if (next < 0 || next >= candCount) {
            Sound.edge();
            return;
        }
        Sound.tick();
        candIndex = next;
    }

    function pick() {
        if (candCount === 0) {
            Sound.edge();
            return;
        }
        Sound.enter();
        candIndex >= candidates.length ? form.moreCandidates() : form.apply(currentSlot, candidates[candIndex].url);
    }

    function backToDefault() {
        if (!picked) {
            Sound.edge();
            return;
        }
        Sound.cancel();
        form.removeOverride(currentSlot);
    }

    function askWrongGame() {
        Sound.panel();
        typing = "search";
        keyboard.show("Search SteamGridDB", form.title, "text");
    }

    function menuAnchor() {
        if (browsing)
            return nowFrame;
        var row = rowOf[index] === 0 ? topCards : bottomCards;
        var item = row.itemAt(index - (rowOf[index] === 0 ? 0 : 3));
        return item ? item.art : page;
    }

    function openMenu() {
        if (!current) {
            Sound.edge();
            return;
        }
        Sound.panel();
        var anchor = menuAnchor();
        var items = [ { icon: "download", label: "Fetch missing art", action: "fetch" },
                      { icon: "folder", label: "Use a file…", action: "file" } ];
        menu.show(items, anchor, Qt.rect(0, 0, anchor.width, anchor.height), current.label, function(action) {
            if (action === "fetch") {
                Sound.enter();
                form.refresh();
                page.forceActiveFocus();
            } else if (action === "file") {
                paths.show("Use a file for the " + current.label.toLowerCase(), "", true);
            } else {
                page.forceActiveFocus();
            }
        });
    }

    Connections {
        target: page.form
        function onMessage(text) { page.message(text); }
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat && (api.keys.isAccept(event) || api.keys.isCancel(event)))
            return;
        if (page.modal)
            return;
        event.accepted = true;
        if (api.keys.isAccept(event)) {
            page.browsing ? pick() : openBrowser(false);
        } else if (api.keys.isCancel(event)) {
            page.browsing ? closeBrowser() : page.closeRequested();
        } else if (api.keys.isDetails(event)) {
            backToDefault();
        } else if (api.keys.isFilters(event)) {
            askWrongGame();
        } else if (api.keys.isMenu(event)) {
            openMenu();
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            var down = event.key === Qt.Key_Down;
            if (page.browsing)
                stepCand(down ? page.columns : -page.columns);
            else
                down ? moveDown() : moveUp();
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            var d = event.key === Qt.Key_Left ? -1 : 1;
            page.browsing ? stepCand(d) : moveAcross(d);
        } else {
            event.accepted = false;
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
        label: page.browsing && page.current ? page.current.label.toUpperCase() : "ARTWORK"
        detail: page.browsing && page.current ? page.current.use + (page.entryLine !== "" ? " · " + page.entryLine : "") : page.entryLine
    }

    component SlotCard: Item {
        id: card

        required property var modelData
        required property int index
        property int slotIndex: 0
        property real rowHeight: 0
        readonly property bool focused: slotIndex === page.index
        readonly property Item art: artFrame

        width: artFrame.width
        height: rowHeight + Theme.dp(14) + caption.height
        opacity: focused || page.browsing ? 1.0 : Theme.idleOpacity

        Behavior on opacity { Ease { duration: Theme.durQuick } }

        Item {
            id: artFrame
            width: Math.round(card.rowHeight * card.modelData.aspect)
            height: card.rowHeight
            scale: card.focused && !page.browsing ? 1.02 : 1.0

            Behavior on scale { Ease { easing.type: Easing.OutQuint } }

            Loader {
                anchors.fill: parent
                active: card.focused && !page.browsing
                sourceComponent: FocusRing { cornerRadius: Theme.dp(12) }
            }

            SlotArt {
                anchors.fill: parent
                row: card.modelData
                badge: true
            }
        }

        // A narrow card's caption may run into the gap after it, never into the next card.
        Column {
            id: caption
            anchors.top: artFrame.bottom
            anchors.topMargin: Theme.dp(14)
            width: artFrame.width + page.cardGap - Theme.dp(16)
            spacing: Theme.dp(3)

            Text {
                width: parent.width
                text: card.modelData.label
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(24)
                elide: Text.ElideRight
            }

            Text {
                width: parent.width
                text: card.modelData.use
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(19)
                elide: Text.ElideRight
            }
        }
    }

    component SlotArt: RoundedMask {
        id: slotArt

        property var row: null
        property bool dim: false
        property bool badge: false
        readonly property bool empty: row === null || row.url === ""

        radius: Theme.dp(12)
        opacity: dim ? 0.6 : 1.0

        Rectangle {
            anchors.fill: parent
            color: slotArt.empty ? Qt.rgba(0.88, 0.40, 0.35, 0.08) : slotArt.row && slotArt.row.slot === "logo" ? Qt.rgba(1, 1, 1, 0.05) : Theme.surface
            border.width: slotArt.empty ? 2 : 0
            border.color: Qt.rgba(0.88, 0.40, 0.35, 0.5)
        }

        Image {
            anchors.fill: parent
            source: slotArt.row ? slotArt.row.url : ""
            fillMode: slotArt.row && slotArt.row.slot === "logo" ? Image.PreserveAspectFit : Image.PreserveAspectCrop
            asynchronous: true
            sourceSize.width: 1200
        }

        Text {
            anchors.centerIn: parent
            visible: slotArt.empty
            text: "Nothing yet"
            color: "#e0655a"
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(20)
        }

        KindBadge {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.margins: Theme.dp(12)
            visible: slotArt.badge && slotArt.row !== null && slotArt.row.kindLabel !== undefined
            kind: slotArt.row ? slotArt.row.kind : "missing"
            label: slotArt.row && slotArt.row.kindLabel ? slotArt.row.kindLabel : ""
        }
    }

    Item {
        id: gallery

        anchors.top: header.bottom
        anchors.topMargin: Theme.dp(36)
        anchors.left: parent.left
        anchors.leftMargin: page.sideMargin
        anchors.right: parent.right
        anchors.rightMargin: page.sideMargin
        anchors.bottom: hintBar.top
        visible: opacity > 0.01
        opacity: page.browsing ? 0.0 : 1.0

        Behavior on opacity { Ease { duration: Theme.durView } }

        Row {
            id: topRowItems
            spacing: page.cardGap

            Repeater {
                id: topCards
                model: page.slots.slice(0, 3)

                SlotCard {
                    slotIndex: index
                    rowHeight: page.topRow
                }
            }
        }

        Row {
            anchors.top: topRowItems.bottom
            anchors.topMargin: Theme.dp(44)
            spacing: page.cardGap

            Repeater {
                id: bottomCards
                model: page.slots.slice(3)

                SlotCard {
                    slotIndex: index + 3
                    rowHeight: page.bottomRow
                }
            }
        }
    }

    Item {
        id: browser

        anchors.fill: gallery
        visible: opacity > 0.01
        opacity: page.browsing ? 1.0 : 0.0

        Behavior on opacity { Ease { duration: Theme.durView } }

        readonly property real nowHeight: Theme.dp(200)

        Row {
            id: showing
            spacing: Theme.dp(48)

            Column {
                spacing: Theme.dp(12)

                Item {
                    id: nowFrame
                    width: Math.round(browser.nowHeight * page.currentAspect)
                    height: browser.nowHeight

                    SlotArt {
                        anchors.fill: parent
                        row: page.current
                        badge: true
                    }
                }

                CapsLabel {
                    text: "SHOWING NOW"
                }
            }

            Column {
                spacing: Theme.dp(12)
                visible: page.picked && page.current.hasDefault

                Item {
                    width: nowFrame.width
                    height: browser.nowHeight

                    SlotArt {
                        anchors.fill: parent
                        row: page.current ? { slot: page.current.slot, url: page.current.defaultUrl, kind: "default", kindLabel: page.current.defaultOriginLabel } : null
                        dim: true
                        badge: true
                    }
                }

                CapsLabel {
                    text: "UNDER IT"
                }
            }
        }

        Row {
            id: candHead

            anchors.top: showing.bottom
            anchors.topMargin: Theme.dp(34)
            spacing: Theme.dp(14)

            CapsLabel {
                anchors.verticalCenter: parent.verticalCenter
                text: "STEAMGRIDDB"
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
                    : page.candidates.length === 0 ? (page.form.sgdbId > 0 ? "nothing for this slot" : "no match")
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
            currentIndex: page.browsing ? page.candIndex : -1
            cellWidth: Math.floor(width / page.columns)
            cellHeight: Math.round((cellWidth - Theme.dp(20)) / page.currentAspect) + Theme.dp(20)
            preferredHighlightBegin: 0
            preferredHighlightEnd: height
            highlightRangeMode: GridView.ApplyRange
            highlightFollowsCurrentItem: true

            delegate: Item {
                readonly property bool isMore: index >= page.candidates.length
                readonly property var cand: isMore ? null : page.candidates[index]
                readonly property bool focused: page.browsing && index === page.candIndex

                width: grid.cellWidth
                height: grid.cellHeight

                Item {
                    id: cell
                    anchors.fill: parent
                    anchors.margins: Theme.dp(10)
                    scale: focused ? 1.03 : 1.0

                    Behavior on scale { Ease { easing.type: Easing.OutQuint } }

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

    Sheet {
        id: hits

        anchors.fill: parent
        z: 4
        title: "Which game is it on SteamGridDB?"
        innerMax: Theme.dp(1000)
        contentHeight: Theme.dp(12) + note.height + Theme.dp(18) + hitsList.height

        readonly property var hints: [ { glyph: "A", label: "Use this game", dim: page.form.hits.length === 0 },
                                       { glyph: "Y", label: "Search again" }, { glyph: "B", label: "Close" } ]

        function close() {
            open = false;
            focus = false;
            page.forceActiveFocus();
        }

        Keys.onPressed: function(event) {
            event.accepted = true;
            if (event.isAutoRepeat)
                return;
            var list = page.form.hits;
            if (api.keys.isAccept(event)) {
                if (list.length === 0) {
                    Sound.edge();
                    return;
                }
                Sound.enter();
                page.form.pin(list[page.hitIndex].id);
                hits.close();
            } else if (api.keys.isCancel(event)) {
                Sound.cancel();
                hits.close();
            } else if (api.keys.isFilters(event)) {
                hits.close();
                page.askWrongGame();
            } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                page.hitIndex = Sound.stepped(page.hitIndex, event.key === Qt.Key_Up ? -1 : 1, list.length);
            }
        }

        Text {
            id: note
            anchors.top: hits.head.bottom
            anchors.topMargin: Theme.dp(12)
            anchors.horizontalCenter: parent.horizontalCenter
            width: hits.inner
            text: page.form.searchError !== "" ? page.form.searchError
                : page.form.hits.length === 0 ? "Nothing matched — Y searches with another name."
                : "The candidates and the next fetch follow the one you pick."
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(21)
            wrapMode: Text.WordWrap
        }

        ListView {
            id: hitsList
            anchors.top: note.bottom
            anchors.topMargin: Theme.dp(18)
            anchors.horizontalCenter: parent.horizontalCenter
            width: hits.inner
            height: Math.min(count, 5) * Theme.dp(96)
            model: page.form.hits
            currentIndex: page.hitIndex
            interactive: false
            clip: true
            preferredHighlightBegin: 0
            preferredHighlightEnd: height
            highlightRangeMode: ListView.ApplyRange
            highlightFollowsCurrentItem: true

            delegate: Item {
                readonly property bool lit: index === page.hitIndex && hits.open

                width: hitsList.width
                height: Theme.dp(96)

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: Theme.dp(12)
                    radius: Theme.dp(14)
                    color: lit ? Theme.text : Theme.surface

                    Loader {
                        anchors.fill: parent
                        active: lit
                        sourceComponent: FocusRing { cornerRadius: Theme.dp(14) }
                    }

                    Row {
                        anchors.left: parent.left
                        anchors.leftMargin: Theme.dp(22)
                        anchors.right: hitMeta.left
                        anchors.rightMargin: Theme.dp(16)
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: Theme.dp(14)

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.name
                            color: lit ? Theme.onLight : Theme.text
                            font.family: Theme.sans
                            font.weight: Font.DemiBold
                            font.pixelSize: Theme.dp(24)
                            elide: Text.ElideRight
                        }

                        KindBadge {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: modelData.current
                            kind: "default"
                            label: "Current"
                            onLight: lit
                        }
                    }

                    Text {
                        id: hitMeta
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(22)
                        anchors.verticalCenter: parent.verticalCenter
                        text: (modelData.year > 0 ? modelData.year + "  " : "") + (modelData.verified ? "✓  " : "") + "#" + modelData.id
                        color: lit ? Qt.rgba(0.063, 0.067, 0.086, 0.7) : Theme.textMuted
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(19)
                    }
                }
            }
        }
    }

    ActionMenu {
        id: menu

        anchors.fill: parent
        z: 5
        copyMargin: Theme.dp(10)

        onDismissed: page.forceActiveFocus()
    }

    PathSheet {
        id: paths

        anchors.fill: parent
        z: 4

        onAccepted: function(path) {
            page.form.useFile(page.currentSlot, path);
            page.forceActiveFocus();
        }
        onTypeRequested: function(path) {
            page.typing = "path";
            keyboard.show("Path of the file", path, "path");
        }
        onDismissed: page.forceActiveFocus()
    }

    HintBar {
        id: hintBar
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        z: 6
        sideMargin: page.sideMargin
        showClock: true
        hints: page.hints
    }

    KeyboardSheet {
        id: keyboard
        anchors.fill: parent
        z: 5

        onAccepted: function(value) {
            if (page.typing === "path")
                page.form.useFile(page.currentSlot, value);
            else
                page.form.search(value);
            page.forceActiveFocus();
        }
        onDismissed: page.forceActiveFocus()
    }
}
