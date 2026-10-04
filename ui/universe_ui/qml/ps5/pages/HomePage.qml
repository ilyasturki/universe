import QtQuick
import Universe
import "../core"
import "../sound"
import "../ui"
import "../../core/Format.js" as Format
import "../../ui" as Base
import "Home.js" as Home

// The console's home: Games and Media, a row of tiles over the focused one's world, its hero, and under it the Game Hub.
FocusScope {
    id: page

    property var shell: null
    readonly property var hints: []

    property int tab: 0
    // "bar", "rail", "hero", "hub" or "welcome"
    property string zone: "rail"
    property int gamesIndex: 1
    property int mediaIndex: 0
    readonly property int index: tab === 0 ? gamesIndex : mediaIndex
    property int heroIndex: 0
    property int strip: 0
    property var cards: ({})
    property int barIndex: 0

    readonly property var session: api.universe.currentSession
    readonly property string playingId: session && session.id !== undefined ? session.id : ""
    readonly property bool empty: api.allGames.count === 0

    property var gameEntries: []
    property var mediaEntries: []
    readonly property var entries: tab === 0 ? gameEntries : mediaEntries
    readonly property var entry: index >= 0 && index < entries.length ? entries[index] : null
    readonly property var currentGame: entry && entry.kind === "game" ? entry.game : null

    // What the hero, the backdrop and the title show: the tile the row last came to rest on.
    property var rested: null
    property bool heroShown: false
    property bool sideShown: false
    property bool titleShown: false
    property var backdrop: ({
            scene: "welcome"
        })
    property var strips: []
    readonly property var restedGame: rested && rested.kind === "game" ? rested.game : null

    // The home rebuilding itself after a game, one beat at a time.
    property real railReveal: 1.0
    property real chromeReveal: 1.0
    property real worldReveal: 1.0

    readonly property real heroScroll: Theme.dp(156)
    readonly property real stripPitch: Theme.dp(400)
    readonly property real scroll: zone === "hero" ? heroScroll : zone === "hub" ? Math.max(heroScroll, Theme.dp(Theme.hubY) + strip * stripPitch - Theme.dp(206)) : zone === "welcome" ? Theme.dp(128) : 0
    readonly property bool deep: zone === "hero" || zone === "hub" || zone === "welcome"

    readonly property string playLabel: {
        var g = restedGame;
        if (!g)
            return "Play";
        if (g.installing)
            return "View Download";
        if (g.id === playingId)
            return "Resume";
        return "Play Game";
    }

    RecentGames {
        id: played
        sourceModel: api.allGames
        playingId: page.playingId
    }

    LimitedGames {
        id: recent
        sourceModel: played
        limit: 12
    }

    // A library that predates `added_at` has nothing recent to show; the newest releases stand in, as in Reprise.
    readonly property bool standingIn: recent.count < 5

    SortedGames {
        id: byRelease
        sourceModel: page.standingIn ? api.allGames : null
        sortRoleName: "releaseYear"
        descending: true
    }

    LimitedGames {
        id: newest
        sourceModel: byRelease
        limit: 12
    }

    // A store install under way sits first, as the console shows a download; its tile is the game's once it lands.
    HeadedGames {
        id: headed
        source: page.standingIn ? newest : recent
        head: api.screens.sources.arriving
    }

    Connections {
        target: headed
        function onCountChanged() {
            Qt.callLater(page.buildGames);
        }
        function onModelReset() {
            Qt.callLater(page.buildGames);
        }
        function onRowsMoved() {
            Qt.callLater(page.buildGames);
        }
        function onLayoutChanged() {
            Qt.callLater(page.buildGames);
        }
    }

    Connections {
        target: api.screens.media
        function onRowsChanged() {
            Qt.callLater(page.buildMedia);
        }
    }

    function keyOf(e) {
        return e ? e.kind + ":" + (e.game ? e.game.id : "") : "";
    }

    function rebuilt(list, at) {
        var was = keyOf(entries[at]);
        for (var i = 0; i < list.length; i++)
            if (keyOf(list[i]) === was)
                return i;
        return Math.max(0, Math.min(at, list.length - 1));
    }

    function buildGames() {
        var out = [
            {
                kind: "welcome",
                label: "Welcome"
            }
        ];
        var ids = {};
        for (var i = 0; i < headed.count; i++) {
            var g = headed.get(i);
            if (g) {
                ids[g.id] = true;
                out.push({
                    kind: "game",
                    game: g
                });
            }
        }
        // An empty library: first-run setup and a game file of this machine, first on the row, as Switch 2's discs.
        if (api.allGames.count === 0)
            out.push({
                kind: "setup",
                label: "Set Up",
                tagline: "What other launchers hold, your stores, a few choices",
                action: "Set Up"
            }, {
                kind: "add",
                label: "Add a Game",
                tagline: "A game file on this machine, a store's games, or your Lutris library",
                action: "Add a Game"
            });
        var guest = guestId !== "" && !ids[guestId] ? api.allGames.byId(guestId) : null;
        if (guest)
            out.splice(1, 0, {
                kind: "game",
                game: guest
            });
        out.push({
            kind: "store",
            label: "Store",
            tagline: "Browse and install from your stores' catalogues",
            action: "Open Store"
        }, {
            kind: "gallery",
            label: "Media Gallery",
            tagline: "Your screenshots and recordings",
            action: "Open"
        }, {
            kind: "library",
            label: "Game Library",
            tagline: Format.plural(api.allGames.count, "game", "games") + " in your collection",
            action: "Open"
        });
        var keep = tab === 0 ? rebuilt(out, gamesIndex) : Math.min(gamesIndex, out.length - 1);
        gameEntries = out;
        gamesIndex = keep;
        if (tab === 0 && !rebuildAnim.running)
            settle(true);
    }

    function buildMedia() {
        var rows = api.screens.media.rows;
        var counts = {};
        var order = [];
        rows.forEach(function (r) {
            if (!counts[r.gameId]) {
                counts[r.gameId] = {
                    shot: 0,
                    recording: 0,
                    journal: 0
                };
                order.push(r.gameId);
            }
            counts[r.gameId][r.kind] = (counts[r.gameId][r.kind] || 0) + 1;
        });
        var shots = rows.filter(function (r) {
            return r.kind !== "journal";
        }).length;
        var out = [
            {
                kind: "gallery",
                label: "Media Gallery",
                tagline: shots > 0 ? Format.plural(shots, "capture", "captures") + ", newest first" : "Your screenshots and recordings",
                action: "Open"
            },
            {
                kind: "journal",
                label: "Journal",
                tagline: "What each session was, written after it",
                action: "Read"
            }
        ];
        order.slice(0, 12).forEach(function (id) {
            var g = api.allGames.byId(id);
            if (!g)
                return;
            var c = counts[id];
            var bits = [];
            if (c.shot)
                bits.push(Format.plural(c.shot, "screenshot", "screenshots"));
            if (c.recording)
                bits.push(Format.plural(c.recording, "recording", "recordings"));
            if (c.journal)
                bits.push(Format.plural(c.journal, "journal entry", "journal entries"));
            out.push({
                kind: "channel",
                tile: "game",
                game: g,
                label: g.title,
                tagline: bits.join(" · "),
                action: "View"
            });
        });
        var keep = tab === 1 ? rebuilt(out, mediaIndex) : Math.min(mediaIndex, out.length - 1);
        mediaEntries = out;
        mediaIndex = keep;
        if (tab === 1 && !rebuildAnim.running)
            settle(true);
    }

    // A move: the hero and the name go at once, the world follows once the row rests, the hero after it, the side tile last.
    function moved() {
        heroShown = false;
        sideShown = false;
        titleShown = false;
        restTimer.restart();
        heroTimer.restart();
        sideTimer.stop();
    }

    // The same tile, its data changed under it: shown again without the wait.
    function settle(quiet) {
        if (quiet && keyOf(rested) === keyOf(entry) && heroShown)
            return;
        moved();
    }

    function targetFor(e) {
        if (!e)
            return {
                scene: "system"
            };
        if (e.game)
            return Home.art(e.game);
        return {
            scene: e.kind === "welcome" ? "welcome" : "system"
        };
    }

    Timer {
        id: restTimer
        interval: Theme.backdropRest
        onTriggered: {
            page.backdrop = page.targetFor(page.entry);
            page.titleShown = true;
            // Read now, while the world fades: the hero's own fade then waits on nothing.
            if (page.tab === 0 && page.currentGame)
                page.hubOf(page.currentGame);
            else if (page.entry && page.entry.kind === "welcome")
                welcome.refresh();
        }
    }

    Timer {
        id: heroTimer
        interval: Theme.heroRest
        onTriggered: {
            page.rested = page.entry;
            page.strips = page.tab === 0 && page.restedGame ? page.hubOf(page.restedGame) : [];
            page.heroShown = true;
            sideTimer.restart();
        }
    }

    Timer {
        id: sideTimer
        interval: Theme.sideRest
        onTriggered: page.sideShown = true
    }

    // While the home builds itself back, the beats say when each part shows.
    onIndexChanged: if (!rebuildAnim.running)
        moved()
    onTabChanged: if (!rebuildAnim.running)
        moved()

    Component.onCompleted: {
        buildGames();
        buildMedia();
        api.screens.media.load();
        restTimer.stop();
        heroTimer.stop();
        page.backdrop = targetFor(entry);
        page.rested = entry;
        page.strips = restedGame ? hubOf(restedGame) : [];
        page.titleShown = true;
        page.heroShown = true;
        page.sideShown = true;
    }

    // ---- The Game Hub ----

    // The strips by game id: read once, dropped when the game's sessions, captures or journal change.
    property var hubCache: ({})

    function hubOf(game) {
        var hit = hubCache[game.id];
        if (hit)
            return hit;
        hit = hubFor(game);
        hubCache[game.id] = hit;
        return hit;
    }

    // An empty list is the whole library.
    function forget(ids) {
        if (!ids || ids.length === 0)
            hubCache = {};
        else
            ids.forEach(function (id) {
                delete hubCache[id];
            });
        if (restedGame && heroShown && (!ids || ids.length === 0 || ids.indexOf(restedGame.id) >= 0))
            strips = tab === 0 ? hubOf(restedGame) : [];
    }

    Connections {
        target: api.universe
        function onLibraryChanged(ids) {
            page.forget(ids);
        }
        function onRecordingFiled(session, ident, path) {
            page.forget([ident]);
        }
        function onEntryWritten(session, ident) {
            page.forget([ident]);
        }
        function onMediaChanged(ident) {
            page.forget([ident]);
        }
        function onSessionEnded(sessionId, ident, duration, end) {
            page.forget([ident]);
        }
    }

    function hubFor(game) {
        var out = [];
        var media = api.universe.media(game.id) || [];
        var sessions = api.universe.sessions(game.id) || [];
        var journal = api.universe.journal(game.id) || [];
        var captures = media.filter(function (r) {
            return r.kind === "shot" || r.kind === "recording";
        });

        var cont = [];
        var last = sessions.length > 0 ? sessions[0] : null;
        if (last) {
            var ended = Home.date(last.ended_at || last.started_at);
            cont.push({
                badge: "clock",
                caption: ended ? "Last played " + Format.lastPlayed(ended).toLowerCase() : "Last session",
                title: Format.playTime(last.duration_s) || "A few moments",
                body: Format.sessions(game.playCount) + (game.playTime > 0 ? " · " + Home.hours(game.playTime) + " in all" : ""),
                open: {
                    page: "pages/PlayLogPage.qml",
                    args: {
                        gameId: game.id
                    }
                }
            });
        }
        var next = journal.filter(function (e) {
            return e.next_up;
        })[0];
        if (next)
            cont.push({
                badge: "journal",
                caption: "Next up",
                title: next.title || "From your journal",
                body: next.next_up,
                width: 640,
                open: {
                    page: "pages/ArticlePage.qml",
                    args: {
                        gameId: game.id,
                        session: next.session
                    },
                    load: "news"
                }
            });
        var rec = captures.filter(function (r) {
            return r.kind === "recording";
        })[0];
        if (rec)
            cont.push({
                badge: "film",
                caption: "Latest recording",
                title: Format.playTime(rec.duration_s) || "Recording",
                image: Home.art(game).source,
                playIcon: true,
                open: {
                    page: "pages/PlayerPage.qml",
                    args: {
                        gameId: game.id,
                        session: rec.session
                    },
                    load: "album"
                }
            });
        if (cont.length > 0)
            out.push({
                title: "Continue where you left off",
                cards: cont
            });

        if (game.achievementsTotal > 0) {
            var pct = Math.round(100 * game.achievementsUnlocked / game.achievementsTotal);
            out.push({
                title: "Trophies",
                cards: [
                    {
                        badge: "trophy",
                        caption: "Earned " + game.achievementsUnlocked + "/" + game.achievementsTotal,
                        title: pct + "%",
                        progress: pct / 100,
                        open: {
                            page: "pages/AchievementsPage.qml",
                            args: {
                                gameId: game.id
                            }
                        }
                    }
                ]
            });
        }

        if (captures.length > 0) {
            var caps = captures.slice(0, 8).map(function (r) {
                var shot = r.kind === "shot";
                return {
                    image: shot ? "file://" + r.path : Home.art(game).source,
                    playIcon: !shot,
                    caption: Format.lastPlayed(Home.date(r.date)),
                    title: shot ? "Screenshot" : Format.playTime(r.duration_s) || "Recording",
                    width: 480,
                    open: {
                        page: "pages/MediaGalleryPage.qml",
                        args: {
                            gameId: game.id,
                            path: r.path || "",
                            session: r.session || ""
                        }
                    }
                };
            });
            caps.push({
                badge: "gallery",
                caption: Format.plural(captures.length, "capture", "captures"),
                title: "Media Gallery",
                width: 320,
                open: {
                    page: "pages/MediaGalleryPage.qml",
                    args: {
                        gameId: game.id
                    }
                }
            });
            out.push({
                title: "Captures",
                cards: caps
            });
        }

        var written = journal.filter(function (e) {
            return (e.state || "written") === "written";
        });
        if (written.length > 0)
            out.push({
                title: "Journal",
                cards: written.slice(0, 6).map(function (e) {
                    var when = Home.date(e.written_at || e.started_at);
                    return {
                        badge: "journal",
                        caption: when ? Format.lastPlayed(when) : "",
                        title: e.title || "Session",
                        body: (e.paragraphs || []).join(" "),
                        open: {
                            page: "pages/ArticlePage.qml",
                            args: {
                                gameId: game.id,
                                session: e.session
                            },
                            load: "news"
                        }
                    };
                })
            });

        var about = [];
        var description = game.description || game.summary || "";
        var credits = [game.developerList.join(", "), game.releaseYear > 0 ? String(game.releaseYear) : ""].filter(Boolean).join(" · ");
        if (description !== "")
            about.push({
                caption: credits,
                title: "About",
                body: description,
                width: 1032,
                open: {
                    page: "pages/SoftwareInfoPage.qml",
                    args: {
                        gameId: game.id
                    }
                }
            });
        var facts = [];
        if (game.genreList.length > 0)
            facts.push(game.genreList.join(", "));
        if (game.players > 0)
            facts.push(game.players === 1 ? "Single player" : "Up to " + game.players + " players");
        facts.push((game.runnerName || game.runner || "") + (game.source ? " · " + game.source.toUpperCase() : ""));
        about.push({
            badge: "info",
            caption: "Information",
            title: game.publisherList.length > 0 ? game.publisherList[0] : game.title,
            body: facts.filter(Boolean).join("\n"),
            open: {
                page: "pages/SoftwareInfoPage.qml",
                args: {
                    gameId: game.id
                }
            }
        });
        (game.assets.screenshotList || []).slice(0, 6).forEach(function (s, i) {
            about.push({
                image: s,
                width: 480,
                open: {
                    page: "pages/SoftwareInfoPage.qml",
                    args: {
                        gameId: game.id,
                        shot: i
                    }
                }
            });
        });
        out.push({
            title: "About",
            cards: about
        });

        out.push({
            title: "Manage",
            cards: [
                {
                    badge: "sliders",
                    caption: "Launch options, runner, modules",
                    title: "Game Settings",
                    width: 400,
                    open: {
                        page: "pages/GameSettingsPage.qml",
                        args: {
                            gameId: game.id
                        }
                    }
                },
                {
                    badge: "image",
                    caption: "Covers, backgrounds, logos",
                    title: "Artwork",
                    width: 400,
                    open: {
                        page: "pages/ArtworkPage.qml",
                        args: {
                            gameId: game.id
                        }
                    }
                },
                {
                    badge: "clock",
                    caption: Format.sessions(game.playCount) || "No sessions yet",
                    title: "Play Log",
                    width: 400,
                    open: {
                        page: "pages/PlayLogPage.qml",
                        args: {
                            gameId: game.id
                        }
                    }
                },
                {
                    badge: "storage",
                    caption: "Saves, backups and the prefix",
                    title: "Saved Data and Storage",
                    width: 400,
                    open: {
                        page: "pages/DataPage.qml",
                        args: {
                            gameId: game.id
                        }
                    }
                }
            ]
        });
        return out;
    }

    function cardAt(k) {
        return cards[k] || 0;
    }

    function setCard(k, i) {
        var next = Object.assign({}, cards);
        next[k] = i;
        cards = next;
    }

    function openCard(card) {
        if (!card || !card.open) {
            Sound.play("edge");
            return;
        }
        Sound.play("ok");
        if (card.open.load === "news")
            api.screens.news.load(card.open.args.gameId);
        else if (card.open.load === "album")
            api.screens.album.load(card.open.args.gameId);
        shell.push(card.open.page, card.open.args);
    }

    // ---- Moving about ----

    function reset() {
        tab = 0;
        zone = "rail";
        heroIndex = 0;
        strip = 0;
    }

    // B on the home: back up to the row, as the console's circle does.
    function back() {
        if (zone === "rail" && tab === 1) {
            Sound.play("back");
            tab = 0;
            return true;
        }
        if (zone === "rail")
            return false;
        Sound.play("back");
        zone = "rail";
        forceActiveFocus();
        return true;
    }

    // A move while the home is still building itself: the rest comes in at once.
    function finishRebuild() {
        if (!rebuildAnim.running)
            return;
        rebuildAnim.stop();
        railReveal = 1;
        worldReveal = 1;
        chromeReveal = 1;
        titleShown = true;
        heroShown = true;
        sideShown = true;
    }

    function step(d) {
        finishRebuild();
        if (tab === 0)
            gamesIndex = Sound.stepped(gamesIndex, d, gameEntries.length);
        else
            mediaIndex = Sound.stepped(mediaIndex, d, mediaEntries.length);
    }

    function pointTile(i) {
        finishRebuild();
        if (zone === "rail" && i === index) {
            activate();
            return;
        }
        Sound.play("tick");
        zone = "rail";
        if (tab === 0)
            gamesIndex = i;
        else
            mediaIndex = i;
        forceActiveFocus();
    }

    function switchTab(t) {
        if (t === tab || t < 0 || t > 1) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        tab = t;
        if (zone !== "bar")
            zone = "rail";
        if (t === 1)
            api.screens.media.load();
    }

    // A game opened from elsewhere that the row does not hold sits first on it, as the console puts it there.
    property string guestId: ""

    function focusGame(id) {
        for (var pass = 0; pass < 2; pass++) {
            for (var i = 0; i < gameEntries.length; i++)
                if (gameEntries[i].game && gameEntries[i].game.id === id) {
                    tab = 0;
                    zone = "rail";
                    gamesIndex = i;
                    return true;
                }
            if (!api.allGames.byId(id))
                return false;
            guestId = id;
            buildGames();
        }
        return false;
    }

    function down() {
        if (!entry) {
            Sound.play("edge");
            return;
        }
        if (entry.kind === "welcome") {
            Sound.play("tick");
            welcome.enter();
            zone = "welcome";
            welcome.forceActiveFocus();
            return;
        }
        // The hero drawn is the one the row rested on: a quick move shows its own first.
        if (keyOf(rested) !== keyOf(entry)) {
            heroTimer.stop();
            restTimer.stop();
            backdrop = targetFor(entry);
            rested = entry;
            strips = tab === 0 && restedGame ? hubOf(restedGame) : [];
            titleShown = true;
            heroShown = true;
            sideShown = true;
        }
        Sound.play("tick");
        heroIndex = 0;
        zone = "hero";
    }

    function activate() {
        if (zone === "bar") {
            if (barIndex < 2) {
                switchTab(barIndex);
                return;
            }
            Sound.play("ok");
            if (barIndex === 2)
                shell.push("pages/SearchPage.qml", {});
            else if (barIndex === 3)
                shell.push("pages/SettingsPage.qml", {});
            else
                shell.askPower();
            return;
        }
        if (zone === "hero") {
            if (heroIndex === 0)
                primary(rested);
            else if (heroIndex === 1)
                gameMenu(restedGame, {
                    x: hero.moreButton.mapToItem(page, 0, 0).x - Theme.dp(20),
                    y: Theme.dp(260)
                });
            else
                openCard({
                    open: {
                        page: "pages/MediaGalleryPage.qml",
                        args: {
                            gameId: restedGame ? restedGame.id : ""
                        }
                    }
                });
            return;
        }
        if (zone === "hub") {
            openCard((strips[strip] || {
                    cards: []
                }).cards[cardAt(strip)]);
            return;
        }
        primary(entry);
    }

    function primary(e) {
        if (!e) {
            Sound.play("edge");
            return;
        }
        if (e.kind === "game") {
            var g = e.game;
            if (g.installing) {
                Sound.play("ok");
                shell.push("pages/InstallPage.qml", {
                    tab: 1
                });
            } else if (g.id === playingId) {
                shell.resume();
            } else {
                hero.playButton.flash();
                shell.launch(g);
            }
            return;
        }
        if (e.kind === "welcome") {
            down();
            return;
        }
        Sound.play("ok");
        if (e.kind === "store")
            shell.push("pages/InstallPage.qml", {});
        else if (e.kind === "gallery")
            shell.push("pages/MediaGalleryPage.qml", {});
        else if (e.kind === "library")
            shell.push("pages/LibraryPage.qml", {});
        else if (e.kind === "journal")
            shell.push("pages/NewsPage.qml", {});
        else if (e.kind === "setup")
            shell.push("pages/OnboardingPage.qml", {});
        else if (e.kind === "add")
            shell.push("pages/AddGamePage.qml", {});
        else if (e.kind === "channel")
            shell.push("pages/MediaGalleryPage.qml", {
                gameId: e.game.id
            });
    }

    // The console's "…": the tile menu, beside the tile or over the hero.
    function gameMenu(game, at) {
        if (!game || game.installing) {
            Sound.play("edge");
            return;
        }
        var items = Home.options(game, game.id === playingId);
        var id = game.id;
        shell.showMenu({
            items: items,
            at: at
        }, function (i) {
            if (i < 0)
                return;
            var g = api.allGames.byId(id);
            if (items[i].act === "play" && g)
                page.primary({
                    kind: "game",
                    game: g
                });
            else
                shell.gameOption(items[i].act, id);
        });
    }

    // ---- Back from a game: the home builds itself up again, as the console's ----

    // The game under the launcher when it went blank: the one to land on when it comes back, ended or not.
    property string lastSession: ""

    function blank() {
        if (playingId !== "")
            lastSession = playingId;
        rebuildAnim.stop();
        railReveal = 0;
        chromeReveal = 0;
        worldReveal = 0;
        heroShown = false;
        sideShown = false;
        titleShown = false;
    }

    function rebuild() {
        rebuildAnim.stop();
        reset();
        var back = playingId !== "" ? playingId : lastSession;
        if (back !== "")
            focusGame(back);
        restTimer.stop();
        heroTimer.stop();
        sideTimer.stop();
        blank();
        backdrop = targetFor(entry);
        rested = entry;
        strips = tab === 0 && restedGame ? hubOf(restedGame) : [];
        rebuildAnim.restart();
    }

    ParallelAnimation {
        id: rebuildAnim

        SequentialAnimation {
            PauseAnimation {
                duration: Theme.beatRail
            }
            NumberAnimation {
                target: page
                property: "railReveal"
                to: 1
                duration: 260
                easing.type: Easing.OutCubic
            }
        }
        SequentialAnimation {
            PauseAnimation {
                duration: Theme.beatBackdrop
            }
            NumberAnimation {
                target: page
                property: "worldReveal"
                to: 1
                duration: 220
                easing.type: Easing.InOutQuad
            }
        }
        SequentialAnimation {
            PauseAnimation {
                duration: Theme.beatChrome
            }
            NumberAnimation {
                target: page
                property: "chromeReveal"
                to: 1
                duration: Theme.durChrome
            }
            ScriptAction {
                script: page.titleShown = true
            }
        }
        SequentialAnimation {
            PauseAnimation {
                duration: Theme.beatHero
            }
            ScriptAction {
                script: {
                    page.backdrop = page.targetFor(page.entry);
                    page.rested = page.entry;
                    page.strips = page.tab === 0 && page.restedGame ? page.hubOf(page.restedGame) : [];
                    page.heroShown = true;
                }
            }
            PauseAnimation {
                duration: Theme.beatSide - Theme.beatHero
            }
            ScriptAction {
                script: page.sideShown = true
            }
        }
    }

    // ---- Keys ----

    Keys.onPressed: function (event) {
        var horizontal = event.key === Qt.Key_Left || event.key === Qt.Key_Right;
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        if (event.isAutoRepeat && !horizontal && !vertical)
            return;
        var d = event.key === Qt.Key_Left || event.key === Qt.Key_Up ? -1 : 1;
        if (api.keys.isPrevPage(event) || api.keys.isNextPage(event)) {
            event.accepted = true;
            switchTab(api.keys.isPrevPage(event) ? 0 : 1);
            return;
        }
        if (zone === "welcome")
            return;
        if (zone === "bar") {
            event.accepted = true;
            if (horizontal)
                barIndex = Sound.stepped(barIndex, d, topBar.count);
            else if (event.key === Qt.Key_Down) {
                Sound.play("tick");
                zone = "rail";
            } else if (event.key === Qt.Key_Up)
                Sound.play("edge");
            else if (api.keys.isAccept(event))
                activate();
            else
                event.accepted = false;
            return;
        }
        if (zone === "rail") {
            event.accepted = true;
            if (horizontal)
                step(d);
            else if (event.key === Qt.Key_Up) {
                Sound.play("tick");
                barIndex = tab;
                zone = "bar";
            } else if (event.key === Qt.Key_Down)
                down();
            else if (api.keys.isAccept(event))
                activate();
            else if (api.keys.isMenu(event)) {
                if (currentGame)
                    gameMenu(currentGame, {
                        x: Theme.dp(Theme.tileFocusX + Theme.tileFocus + 20),
                        y: Theme.dp(Theme.railY - 8)
                    });
                else
                    Sound.play("edge");
            } else if (api.keys.isDetails(event)) {
                if (currentGame)
                    down();
                else
                    Sound.play("edge");
            } else if (api.keys.isFilters(event)) {
                if (currentGame && !currentGame.installing) {
                    currentGame.favorite = !currentGame.favorite;
                    Sound.play("select");
                    shell.showToast(currentGame.favorite ? "Added " + currentGame.title + " to Favourites" : "Removed " + currentGame.title + " from Favourites");
                } else
                    Sound.play("edge");
            } else
                event.accepted = false;
            return;
        }
        if (zone === "hero") {
            event.accepted = true;
            if (horizontal)
                heroIndex = Sound.stepped(heroIndex, d, hero.buttons);
            else if (event.key === Qt.Key_Up) {
                Sound.play("tick");
                zone = "rail";
            } else if (event.key === Qt.Key_Down) {
                if (strips.length > 0) {
                    Sound.play("tick");
                    strip = 0;
                    zone = "hub";
                } else
                    Sound.play("edge");
            } else if (api.keys.isAccept(event))
                activate();
            else if (api.keys.isMenu(event))
                gameMenu(restedGame, {
                    x: Theme.dp(560),
                    y: Theme.dp(260)
                });
            else
                event.accepted = false;
            return;
        }
        if (zone === "hub") {
            event.accepted = true;
            var s = strips[strip];
            if (horizontal)
                setCard(strip, Sound.stepped(cardAt(strip), d, s ? s.cards.length : 0));
            else if (event.key === Qt.Key_Up) {
                Sound.play("tick");
                if (strip > 0)
                    strip--;
                else
                    zone = "hero";
            } else if (event.key === Qt.Key_Down) {
                if (strip < strips.length - 1) {
                    Sound.play("tick");
                    strip++;
                } else
                    Sound.play("edge");
            } else if (api.keys.isAccept(event))
                activate();
            else if (api.keys.isMenu(event))
                gameMenu(restedGame, {
                    x: Theme.dp(560),
                    y: Theme.dp(260)
                });
            else
                event.accepted = false;
        }
    }

    // ---- The wheel and the finger ----

    // A notch or a swipe up or down is the key it stands for: into the hero, the hub, back up to the row.
    Base.WheelKeys {
        z: -1
    }

    DragHandler {
        id: pageDrag
        property int taken: 0
        target: null
        xAxis.enabled: false
        acceptedDevices: PointerDevice.TouchScreen
        grabPermissions: PointerHandler.CanTakeOverFromItems | PointerHandler.CanTakeOverFromHandlersOfDifferentType
        onActiveChanged: taken = 0
        onTranslationChanged: {
            var steps = Math.trunc(-translation.y / Theme.dp(140));
            while (taken !== steps) {
                api.keys.press(steps > taken ? "Down" : "Up");
                taken += steps > taken ? 1 : -1;
            }
        }
    }

    // The row takes the wheel and a sideways swipe tile by tile.
    Item {
        x: 0
        y: Theme.dp(Theme.railY)
        width: parent.width
        height: Theme.dp(Theme.tileFocus)
        visible: !page.deep
        z: 20

        Base.WheelKeys {
            horizontal: true
        }

        DragHandler {
            property int taken: 0
            target: null
            yAxis.enabled: false
            acceptedDevices: PointerDevice.TouchScreen | PointerDevice.Mouse
            grabPermissions: PointerHandler.CanTakeOverFromItems | PointerHandler.CanTakeOverFromHandlersOfDifferentType
            onActiveChanged: taken = 0
            onTranslationChanged: {
                var steps = Math.trunc(-translation.x / Theme.dp(Theme.tileSize + Theme.tileGap));
                while (taken !== steps) {
                    api.keys.press(steps > taken ? "Right" : "Left");
                    taken += steps > taken ? 1 : -1;
                }
            }
        }
    }

    // ---- The picture ----

    ArtBackdrop {
        id: world
        anchors.fill: parent
        target: page.backdrop
        dim: page.zone === "hub" ? 0.55 : page.zone === "hero" ? 0.12 : page.zone === "welcome" ? 0.2 : 0
        opacity: page.worldReveal

        Behavior on dim {
            NumberAnimation {
                duration: Theme.durScroll
            }
        }
    }

    Item {
        id: content

        anchors.left: parent.left
        anchors.right: parent.right
        height: parent.height
        y: -page.scroll

        Behavior on y {
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        HomeHero {
            id: hero
            width: parent.width
            height: page.height
            visible: page.rested !== null && page.rested.kind !== "welcome"
            entry: page.rested
            playLabel: page.playLabel
            active: page.zone === "hero" && page.activeFocus
            index: page.heroIndex
            shown: page.heroShown && page.zone !== "hub"
            sideShown: page.sideShown
            onPointed: function (i) {
                if (page.zone === "hero" && page.heroIndex === i) {
                    page.activate();
                    return;
                }
                Sound.play("tick");
                page.zone = "hero";
                page.heroIndex = i;
                page.forceActiveFocus();
            }
        }

        Column {
            id: hub
            objectName: "hub"

            y: Theme.dp(Theme.hubY)
            width: parent.width
            spacing: page.stripPitch - Theme.dp(352)
            opacity: page.heroShown && page.tab === 0 && page.restedGame !== null ? 1.0 : 0.0
            visible: opacity > 0.01

            Behavior on opacity {
                NumberAnimation {
                    duration: page.heroShown ? Theme.durHero : Theme.durHeroOut
                }
            }

            Repeater {
                model: page.strips

                HubStrip {
                    objectName: "hubStrip"
                    width: hub.width
                    title: modelData.title
                    cards: modelData.cards
                    current: page.cardAt(index)
                    active: page.zone === "hub" && page.strip === index && page.activeFocus
                    opacity: page.zone !== "hub" ? 1.0 : index < page.strip ? 0.0 : index - page.strip <= 1 ? 1.0 : 0.35
                    onPointed: function (i) {
                        if (page.zone === "hub" && page.strip === index && page.cardAt(index) === i) {
                            page.activate();
                            return;
                        }
                        Sound.play("tick");
                        page.zone = "hub";
                        page.strip = index;
                        page.setCard(index, i);
                        page.forceActiveFocus();
                    }

                    Behavior on opacity {
                        NumberAnimation {
                            duration: Theme.durScroll
                        }
                    }
                }
            }
        }

        WelcomeHub {
            id: welcome

            y: Theme.dp(448)
            width: parent.width
            height: Theme.dp(640)
            shown: page.rested !== null && page.rested.kind === "welcome" && page.tab === 0
            visible: opacity > 0.01
            opacity: welcome.shown && page.heroShown ? 1.0 : 0.0
            focus: page.zone === "welcome"

            Behavior on opacity {
                NumberAnimation {
                    duration: page.heroShown ? Theme.durHero : Theme.durHeroOut
                }
            }

            onEscapedUp: {
                page.zone = "rail";
                page.forceActiveFocus();
            }
            onOpened: function (what, gameId) {
                if (what === "updates")
                    page.shell.push("pages/SettingsPage.qml", {
                        section: "updates"
                    });
                else if (what === "doctor")
                    page.shell.push("pages/SettingsPage.qml", {
                        section: "doctor"
                    });
                else if (what === "controllers")
                    page.shell.push("pages/ControllersPage.qml", {});
                else if (what === "store")
                    page.shell.push("pages/InstallPage.qml", {});
                else if (what === "storage")
                    page.shell.push("pages/StoragePage.qml", {});
                else if (what === "gallery")
                    page.shell.push("pages/MediaGalleryPage.qml", {});
                else if (what === "trophies" && gameId !== "")
                    page.shell.push("pages/AchievementsPage.qml", {
                        gameId: gameId
                    });
                else if (what === "playlog" && gameId !== "")
                    page.shell.push("pages/PlayLogPage.qml", {
                        gameId: gameId
                    });
                else
                    Sound.play("edge");
            }
        }
    }

    // ---- The chrome ----

    Item {
        id: chrome

        anchors.fill: parent
        opacity: (page.deep ? 0.0 : 1.0) * page.chromeReveal
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: page.deep ? 80 : Theme.durChrome
            }
        }

        TopBar {
            id: topBar
            width: parent.width
            tab: page.tab
            active: page.zone === "bar" && page.activeFocus
            index: page.barIndex
            onPointed: function (i) {
                if (page.zone === "bar" && page.barIndex === i) {
                    page.activate();
                    return;
                }
                Sound.play("tick");
                page.zone = "bar";
                page.barIndex = i;
                page.forceActiveFocus();
            }
        }
    }

    Item {
        anchors.fill: parent
        opacity: page.deep ? 0.0 : 1.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: page.deep ? 80 : Theme.durChrome
            }
        }

        HomeRail {
            id: gamesRail
            y: Theme.dp(Theme.railY)
            width: parent.width
            entries: page.gameEntries
            current: page.gamesIndex
            active: page.zone === "rail" && page.tab === 0 && page.activeFocus
            titleShown: page.titleShown
            reveal: page.railReveal
            opacity: page.tab === 0 ? 1.0 : 0.0
            visible: opacity > 0.01
            transform: Translate {
                x: page.tab === 0 ? 0 : -Theme.dp(60)

                Behavior on x {
                    NumberAnimation {
                        duration: Theme.durTab
                        easing.type: Easing.OutCubic
                    }
                }
            }
            onPointed: function (i) {
                page.pointTile(i);
            }

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.durTab
                    easing.type: Easing.OutCubic
                }
            }
        }

        HomeRail {
            id: mediaRail
            y: Theme.dp(Theme.railY)
            width: parent.width
            entries: page.mediaEntries.map(function (e) {
                return {
                    kind: e.tile || e.kind,
                    game: e.game,
                    label: e.label
                };
            })
            current: page.mediaIndex
            active: page.zone === "rail" && page.tab === 1 && page.activeFocus
            titleShown: page.titleShown
            reveal: page.railReveal
            opacity: page.tab === 1 ? 1.0 : 0.0
            visible: opacity > 0.01
            transform: Translate {
                x: page.tab === 1 ? 0 : Theme.dp(60)

                Behavior on x {
                    NumberAnimation {
                        duration: Theme.durTab
                        easing.type: Easing.OutCubic
                    }
                }
            }
            onPointed: function (i) {
                page.pointTile(i);
            }

            Behavior on opacity {
                NumberAnimation {
                    duration: Theme.durTab
                    easing.type: Easing.OutCubic
                }
            }
        }
    }

    Rectangle {
        objectName: "hubScrim"
        width: parent.width
        height: Theme.dp(Theme.headerY + 112)
        opacity: page.zone === "hub" ? 1.0 : 0.0
        visible: opacity > 0.01
        gradient: Gradient {
            GradientStop {
                position: 0.0
                color: Qt.rgba(0, 0, 0, 0.85)
            }
            GradientStop {
                position: 0.6
                color: Qt.rgba(0, 0, 0, 0.7)
            }
            GradientStop {
                position: 1.0
                color: Qt.rgba(0, 0, 0, 0)
            }
        }

        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durScroll
            }
        }
    }

    // Past the row, the tile and the name stand at the top left in its place.
    Item {
        id: header

        anchors.fill: parent
        opacity: page.deep && page.rested ? 1.0 : 0.0
        visible: opacity > 0.01

        Behavior on opacity {
            NumberAnimation {
                duration: page.deep ? Theme.durChrome : 80
            }
        }

        RailTile {
            x: Theme.dp(48)
            y: Theme.dp(48)
            width: Theme.dp(Theme.headerIcon)
            height: width
            game: page.rested ? page.rested.game || null : null
            kind: page.rested ? (page.rested.tile || page.rested.kind) : "game"
        }

        Label {
            x: Theme.dp(Theme.edge)
            y: Theme.dp(Theme.headerY) - height / 2
            width: parent.width - x - Theme.dp(Theme.columnRight)
            text: page.rested ? (page.rested.game ? page.rested.game.title : page.rested.label || "") : ""
            elide: Text.ElideRight
            font.weight: Font.Light
            font.pixelSize: Theme.dp(Theme.fontTitle)
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "#000000"
        opacity: 1.0 - page.worldReveal
        visible: opacity > 0.01
        z: -1
    }
}
