import QtQuick
import "../core"
import "../sound"
import "../../core/Format.js" as Format

// The Welcome hub under the first tile: what needs attention as pills, then one fixed layout of widgets.
FocusScope {
    id: hub

    // "pills" or "grid"
    property string zone: "grid"
    property int pill: 0
    property string cell: "gallery"
    property bool shown: false

    signal opened(string what, string gameId)
    signal escapedUp

    readonly property real leftEdge: Theme.dp(180)
    readonly property real gap: Theme.dp(24)
    readonly property real colW: Math.floor((width - leftEdge - Theme.dp(Theme.columnRight + 8) - gap * 2) / 3)
    readonly property real topEdge: Theme.dp(80)

    readonly property var sources: api.screens.sources
    readonly property var doctor: api.screens.modules.doctor
    readonly property int failing: doctor.filter(function (c) {
        return c.value !== true;
    }).length
    readonly property var pills: {
        var out = [];
        if (sources.updates.length > 0)
            out.push({
                id: "updates",
                glyph: "download",
                label: Format.plural(sources.updates.length, "update", "updates") + " available"
            });
        if (failing > 0)
            out.push({
                id: "doctor",
                glyph: "doctor",
                label: Format.plural(failing, "check needs", "checks need") + " attention"
            });
        if (out.length === 0)
            out.push({
                id: "updates",
                glyph: "check",
                label: "Everything is up to date"
            });
        return out;
    }

    readonly property var captures: api.screens.media.rows.filter(function (r) {
        return r.kind === "shot" || r.kind === "recording";
    })
    readonly property var latest: captures.length > 0 ? captures[0] : null

    // The three most recently played games that count trophies.
    property var trophyGames: []
    property int trophyTotal: 0
    // Seconds played on each of the last seven days, oldest first.
    property var week: [0, 0, 0, 0, 0, 0, 0]
    property string weekTop: ""

    readonly property var pad: {
        var sources = api.power.sources.filter(function (s) {
            return s.kind === "pad";
        });
        return sources.length > 0 ? sources[0] : null;
    }
    readonly property var device: {
        var c = api.screens.controller;
        return c.devices.filter(function (d) {
            return d.id === c.current;
        })[0] || null;
    }

    // When the widgets were last read: the row's rest reads them, the hub shown a moment later reuses that.
    property double readAt: 0

    function refresh() {
        readAt = Date.now();
        var games = [];
        var total = 0;
        for (var i = 0; i < api.allGames.count; i++) {
            var g = api.allGames.get(i);
            if (!g || g.achievementsTotal <= 0)
                continue;
            total += g.achievementsUnlocked;
            games.push(g);
        }
        games.sort(function (a, b) {
            var x = a.lastPlayed ? new Date(a.lastPlayed).getTime() || 0 : 0;
            var y = b.lastPlayed ? new Date(b.lastPlayed).getTime() || 0 : 0;
            return y - x;
        });
        trophyGames = games.slice(0, 3).map(function (g) {
            return {
                id: g.id,
                title: g.title,
                unlocked: g.achievementsUnlocked,
                total: g.achievementsTotal
            };
        });
        trophyTotal = total;

        var days = [0, 0, 0, 0, 0, 0, 0];
        var start = new Date();
        start.setHours(0, 0, 0, 0);
        var from = start.getTime() - 6 * 86400000;
        var best = "", bestTime = 0;
        for (i = 0; i < api.allGames.count; i++) {
            g = api.allGames.get(i);
            var last = g && g.lastPlayed ? new Date(g.lastPlayed).getTime() : NaN;
            if (!g || isNaN(last) || last < from)
                continue;
            var mine = 0;
            (api.universe.sessions(g.id) || []).forEach(function (s) {
                var at = new Date(s.started_at || s.ended_at || "").getTime();
                if (isNaN(at) || at < from)
                    return;
                var d = Math.min(6, Math.floor((at - from) / 86400000));
                var secs = Number(s.duration_s || 0);
                days[d] += secs;
                mine += secs;
            });
            if (mine > bestTime) {
                bestTime = mine;
                best = g.id;
            }
        }
        week = days;
        weekTop = best;
    }

    onShownChanged: {
        if (!shown)
            return;
        if (Date.now() - readAt > 2000)
            refresh();
        api.screens.media.load();
        if (api.screens.modules.doctor.length === 0)
            api.screens.modules.loadDoctor();
    }

    readonly property var moves: ({
            gallery: {
                right: "trophies",
                up: "pills"
            },
            trophies: {
                left: "gallery",
                right: "storage",
                down: "controller",
                up: "pills"
            },
            controller: {
                left: "gallery",
                right: "time",
                up: "trophies"
            },
            storage: {
                left: "trophies",
                down: "time",
                up: "pills"
            },
            time: {
                left: "controller",
                up: "storage"
            }
        })

    function enter() {
        zone = "grid";
        cell = "gallery";
    }

    function move(dir) {
        if (zone === "pills") {
            if (dir === "left" || dir === "right") {
                pill = Sound.stepped(pill, dir === "left" ? -1 : 1, pills.length);
            } else if (dir === "down") {
                Sound.play("tick");
                zone = "grid";
            } else {
                Sound.play("tick");
                hub.escapedUp();
            }
            return;
        }
        var next = moves[cell][dir];
        if (next === "pills") {
            Sound.play("tick");
            zone = "pills";
            pill = Math.min(pill, pills.length - 1);
        } else if (next) {
            Sound.play("tick");
            cell = next;
        } else {
            Sound.play("edge");
        }
    }

    function activate() {
        Sound.play("ok");
        if (zone === "pills") {
            hub.opened(pills[pill].id, "");
            return;
        }
        if (cell === "gallery")
            hub.opened("gallery", "");
        else if (cell === "trophies")
            hub.opened("trophies", trophyGames.length > 0 ? trophyGames[0].id : "");
        else if (cell === "controller")
            hub.opened("controllers", "");
        else if (cell === "storage")
            hub.opened("store", "");
        else
            hub.opened("playlog", weekTop);
    }

    Keys.onLeftPressed: move("left")
    Keys.onRightPressed: move("right")
    Keys.onUpPressed: move("up")
    Keys.onDownPressed: move("down")
    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            activate();
        } else if (api.keys.isCancel(event)) {
            event.accepted = true;
            Sound.play("back");
            hub.escapedUp();
        }
    }

    component Widget: Item {
        id: widget

        property string cellId: ""
        property string glyph: ""
        property string title: ""
        property string trailing: ""
        readonly property bool focused: hub.activeFocus && hub.zone === "grid" && hub.cell === cellId
        default property alias body: content.data

        Rectangle {
            anchors.fill: parent
            radius: Theme.dp(Theme.radiusCard + 2)
            color: Qt.rgba(0.1, 0.13, 0.2, 0.84)
            border.width: 1
            border.color: Theme.glassEdge
        }

        Row {
            x: Theme.dp(20)
            y: Theme.dp(22)
            spacing: Theme.dp(12)

            Glyph {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(30)
                height: width
                kind: widget.glyph
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: widget.title
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }

        Label {
            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(20)
            y: Theme.dp(24)
            text: widget.trailing
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Item {
            id: content
            anchors.fill: parent
            anchors.margins: Theme.dp(20)
            anchors.topMargin: Theme.dp(72)
        }

        FocusFrame {
            target: widget
            shown: widget.focused
            radius: Theme.dp(Theme.radiusCard + 2)
            gap: Theme.dp(3)
            line: Theme.dp(3)
        }

        Touch {
            current: widget.focused
            onPicked: {
                Sound.play("tick");
                hub.zone = "grid";
                hub.cell = widget.cellId;
                hub.forceActiveFocus();
            }
        }
    }

    Row {
        id: pillRow
        x: hub.leftEdge
        y: 0
        spacing: Theme.dp(20)

        Repeater {
            model: hub.pills

            Rectangle {
                id: pillItem

                readonly property bool focused: hub.activeFocus && hub.zone === "pills" && index === hub.pill

                width: pillText.implicitWidth + Theme.dp(88)
                height: Theme.dp(56)
                radius: height / 2
                color: focused ? "#ffffff" : Qt.rgba(0.08, 0.1, 0.16, 0.8)

                Behavior on color {
                    ColorAnimation {
                        duration: Theme.durFocus
                    }
                }

                Glyph {
                    x: Theme.dp(20)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(28)
                    height: width
                    kind: modelData.glyph
                    tint: pillItem.focused ? Theme.onLight : Theme.plus
                }

                Label {
                    id: pillText
                    x: Theme.dp(62)
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.label
                    color: pillItem.focused ? Theme.onLight : Theme.plus
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }

                Touch {
                    current: pillItem.focused
                    onPicked: {
                        Sound.play("tick");
                        hub.zone = "pills";
                        hub.pill = index;
                        hub.forceActiveFocus();
                    }
                }
            }
        }
    }

    Widget {
        id: gallery
        cellId: "gallery"
        x: hub.leftEdge
        y: hub.topEdge
        width: hub.colW
        height: Theme.dp(502)
        glyph: "gallery"
        title: "Media Gallery"
        trailing: hub.captures.length > 0 ? String(hub.captures.length) : ""

        Image {
            parent: gallery
            anchors.fill: parent
            anchors.margins: 1
            z: -1
            source: hub.latest ? (hub.latest.kind === "shot" ? hub.latest.url : hub.latest.image) : ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            sourceSize.width: 720
            opacity: 0.55
            visible: status === Image.Ready
        }

        Column {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            spacing: Theme.dp(6)

            Label {
                width: parent.width
                text: hub.latest ? (hub.latest.kind === "shot" ? "Latest screenshot" : "Latest recording") : "No captures yet"
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Label {
                width: parent.width
                text: hub.latest ? hub.latest.gameTitle + " · " + hub.latest.dateText : "Screenshots and recordings of your sessions land here."
                color: Theme.textSecondary
                wrapMode: Text.WordWrap
                maximumLineCount: 2
                elide: Text.ElideRight
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }

    Widget {
        cellId: "trophies"
        x: hub.leftEdge + hub.colW + hub.gap
        y: hub.topEdge
        width: hub.colW
        height: Theme.dp(328)
        glyph: "trophy"
        title: "Trophies"
        trailing: "Total: " + hub.trophyTotal

        Column {
            anchors.fill: parent
            spacing: Theme.dp(16)

            Label {
                visible: hub.trophyGames.length === 0
                width: parent.width
                text: "Games that count achievements show their progress here."
                color: Theme.textSecondary
                wrapMode: Text.WordWrap
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Repeater {
                model: hub.trophyGames

                Column {
                    width: parent.width
                    spacing: Theme.dp(8)

                    Item {
                        width: parent.width
                        height: gameName.height

                        Label {
                            id: gameName
                            width: parent.width - count.width - Theme.dp(16)
                            text: modelData.title
                            elide: Text.ElideRight
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }

                        Label {
                            id: count
                            anchors.right: parent.right
                            text: modelData.unlocked + "/" + modelData.total
                            color: Theme.textSecondary
                            font.pixelSize: Theme.dp(Theme.fontSmall)
                        }
                    }

                    Rectangle {
                        width: parent.width
                        height: Theme.dp(6)
                        radius: height / 2
                        color: Qt.rgba(1, 1, 1, 0.16)

                        Rectangle {
                            width: parent.width * (modelData.total > 0 ? modelData.unlocked / modelData.total : 0)
                            height: parent.height
                            radius: parent.radius
                            color: Theme.trophyGold
                        }
                    }
                }
            }
        }
    }

    Widget {
        cellId: "controller"
        x: hub.leftEdge + hub.colW + hub.gap
        y: hub.topEdge + Theme.dp(328) + hub.gap
        width: hub.colW
        height: Theme.dp(150)
        glyph: "gamepad"
        title: hub.device ? hub.device.name : "Controller"

        Row {
            anchors.left: parent.left
            anchors.bottom: parent.bottom
            spacing: Theme.dp(16)

            Item {
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(120)
                height: Theme.dp(14)
                visible: hub.pad !== null

                Rectangle {
                    anchors.fill: parent
                    radius: height / 2
                    color: Qt.rgba(1, 1, 1, 0.16)
                }

                Rectangle {
                    width: parent.width * (hub.pad ? Math.max(0, Math.min(100, hub.pad.percent)) / 100 : 0)
                    height: parent.height
                    radius: height / 2
                    color: hub.pad && hub.pad.percent <= 20 ? Theme.danger : Theme.okGreen
                }
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: hub.pad ? hub.pad.percent + "%" + (hub.pad.charging ? " · Charging" : "") : hub.device ? "Connected" : "None connected"
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }

    Widget {
        cellId: "storage"
        x: hub.leftEdge + (hub.colW + hub.gap) * 2
        y: hub.topEdge
        width: hub.colW
        height: Theme.dp(152)
        glyph: "storage"
        title: "Console Storage"

        Item {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: freeLabel.height

            Row {
                spacing: Theme.dp(10)

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(14)
                    height: width
                    radius: width / 2
                    color: Qt.rgba(1, 1, 1, 0.3)
                }

                Label {
                    id: freeLabel
                    text: "Free space"
                    color: Theme.textSecondary
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }
            }

            Label {
                anchors.right: parent.right
                text: hub.sources.freeSpace > 0 ? Format.bytes(hub.sources.freeSpace) : "—"
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }
        }
    }

    Widget {
        cellId: "time"
        x: hub.leftEdge + (hub.colW + hub.gap) * 2
        y: hub.topEdge + Theme.dp(152) + hub.gap
        width: hub.colW
        height: Theme.dp(326)
        glyph: "clock"
        title: "Play Time"
        trailing: {
            var total = hub.week.reduce(function (a, b) {
                return a + b;
            }, 0);
            return total > 0 ? Format.playTime(total) : "";
        }

        Label {
            y: -Theme.dp(8)
            text: "This week"
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Row {
            id: bars
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: Theme.dp(150)
            spacing: Theme.dp(10)

            readonly property real most: Math.max(1, Math.max.apply(null, hub.week))

            Repeater {
                model: 7

                Item {
                    width: (bars.width - bars.spacing * 6) / 7
                    height: bars.height

                    Rectangle {
                        anchors.bottom: dayName.top
                        anchors.bottomMargin: Theme.dp(8)
                        width: parent.width
                        height: Math.max(Theme.dp(4), (parent.height - Theme.dp(40)) * hub.week[index] / bars.most)
                        radius: Theme.dp(3)
                        color: index === 6 ? Theme.text : Qt.rgba(1, 1, 1, 0.4)
                    }

                    Label {
                        id: dayName
                        anchors.bottom: parent.bottom
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: Qt.formatDate(new Date(Date.now() - (6 - index) * 86400000), "ddd").charAt(0)
                        color: Theme.textMuted
                        font.pixelSize: Theme.dp(Theme.fontTiny)
                    }
                }
            }
        }
    }
}
