use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use adw::prelude::*;
use gtk::gio;

use crate::backend;
use crate::game::{GameObject, Row};
use universe::library::Resolved;

/// Every game the window shows, one object per game for as long as it stays, so a bound widget follows it.
pub struct Library {
    pub store: gio::ListStore,
    index: RefCell<HashMap<String, GameObject>>,
    listeners: RefCell<Vec<Box<dyn Fn()>>>,
}

impl Default for Library {
    fn default() -> Library {
        Library { store: gio::ListStore::new::<GameObject>(), index: RefCell::default(), listeners: RefCell::default() }
    }
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Library").field("games", &self.store.n_items()).finish()
    }
}

impl Library {
    pub fn get(&self, id: &str) -> Option<GameObject> {
        self.index.borrow().get(id).cloned()
    }

    pub fn games(&self) -> Vec<GameObject> {
        self.index.borrow().values().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.index.borrow().is_empty()
    }

    /// Called after every batch: a filter or a sorter reads the games' rows, not their properties.
    pub fn connect_updated(&self, f: impl Fn() + 'static) {
        self.listeners.borrow_mut().push(Box::new(f));
    }

    /// `ids` empty reads the whole library again; otherwise those games and the ones on their prefix, a missing one leaving.
    pub async fn refresh(&self, ids: &[String]) {
        let whole = ids.is_empty();
        let wanted: Vec<String> = ids.to_vec();
        let rows = backend::run(async move {
            let core = backend::core();
            let config = core.config.read().await.clone();
            let games = core.games.read().await;
            let prefix = |r: &Resolved| universe::launcher::prefix_of(&r.game, &config);
            let touched: Vec<PathBuf> = games.iter().filter(|r| wanted.contains(&r.game.id)).map(prefix).collect();
            games
                .iter()
                .filter(|r| r.game.removed_at.is_empty() && (whole || wanted.contains(&r.game.id) || touched.contains(&prefix(r))))
                .map(|r| Row::of(r, &games, &config))
                .collect::<Vec<Row>>()
        })
        .await;
        let present: BTreeSet<String> = rows.iter().map(|r| r.id.clone()).collect();
        let gone: Vec<String> = if whole {
            self.index.borrow().keys().filter(|id| !present.contains(*id)).cloned().collect()
        } else {
            ids.iter().filter(|id| !present.contains(*id)).cloned().collect()
        };
        self.apply(rows, &gone);
    }

    fn apply(&self, rows: Vec<Row>, gone: &[String]) {
        for id in gone {
            if let Some(game) = self.index.borrow_mut().remove(id) {
                if let Some(pos) = self.store.find(&game) {
                    self.store.remove(pos);
                }
            }
        }
        let mut added = Vec::new();
        for row in rows {
            let existing = self.index.borrow().get(&row.id).cloned();
            match existing {
                Some(game) => game.update(row),
                None => {
                    let game = GameObject::new(row);
                    self.index.borrow_mut().insert(game.id(), game.clone());
                    added.push(game);
                }
            }
        }
        if !added.is_empty() {
            self.store.splice(self.store.n_items(), 0, &added);
        }
        for f in self.listeners.borrow().iter() {
            f();
        }
    }
}

/// Folded for matching: lower case, accents off.
pub fn fold(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    text.nfd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)).collect::<String>().to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    LastPlayed,
    AToZ,
    ZToA,
    Newest,
    Oldest,
    MostPlayed,
}

impl Sort {
    pub fn parse(s: &str) -> Sort {
        match s {
            "a-z" => Sort::AToZ,
            "z-a" => Sort::ZToA,
            "newest" => Sort::Newest,
            "oldest" => Sort::Oldest,
            "most-played" => Sort::MostPlayed,
            _ => Sort::LastPlayed,
        }
    }

    pub fn order(self, a: &Row, b: &Row) -> std::cmp::Ordering {
        let by_title = || fold(&a.sort_title).cmp(&fold(&b.sort_title));
        match self {
            Sort::LastPlayed => b.last_played.cmp(&a.last_played).then_with(by_title),
            Sort::AToZ => by_title(),
            Sort::ZToA => by_title().reverse(),
            Sort::Newest => b.added.cmp(&a.added).then_with(|| b.last_played.cmp(&a.last_played)).then_with(by_title),
            Sort::Oldest => a.added.cmp(&b.added).then_with(by_title),
            Sort::MostPlayed => b.hours.total_cmp(&a.hours).then_with(by_title),
        }
    }
}

/// What the sidebar picked: every game, a source's, a platform's, the favourites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    All,
    Favorites,
    Source(String),
    Platform(String),
}

impl View {
    pub fn parse(key: &str) -> View {
        match key.split_once(':') {
            Some(("source", kind)) => View::Source(kind.into()),
            Some(("platform", name)) => View::Platform(name.into()),
            _ if key == "favorites" => View::Favorites,
            _ => View::All,
        }
    }

    pub fn key(&self) -> String {
        match self {
            View::All => "all".into(),
            View::Favorites => "favorites".into(),
            View::Source(kind) => format!("source:{kind}"),
            View::Platform(name) => format!("platform:{name}"),
        }
    }

    pub fn holds(&self, row: &Row) -> bool {
        match self {
            View::All => true,
            View::Favorites => row.favorite,
            View::Source(kind) => row.source == *kind,
            View::Platform(name) => row.platform == *name,
        }
    }
}

/// A search matches every word somewhere in the title, the developer, the publisher or the platform.
pub fn matches(row: &Row, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let hay = fold(&format!("{} {} {} {}", row.title, row.developer, row.publisher, platform_name(&row.platform)));
    words.iter().all(|w| hay.contains(w.as_str()))
}

pub fn search_words(text: &str) -> Vec<String> {
    fold(text).split_whitespace().map(String::from).collect()
}

/// The ids a search finds, best first: a title that starts with the words, then one with a word starting with each, then
/// the rest; the last played first among equals. None for no words.
pub fn rank(rows: &[Row], words: &[String]) -> Vec<String> {
    if words.is_empty() {
        return Vec::new();
    }
    let whole = words.join(" ");
    let mut hits: Vec<(u8, &Row)> = rows
        .iter()
        .filter(|r| matches(r, words))
        .map(|r| {
            let title = fold(&r.title);
            let tier = if title.starts_with(&whole) {
                0
            } else if words.iter().all(|w| title.split_whitespace().any(|t| t.starts_with(w.as_str()))) {
                1
            } else {
                2
            };
            (tier, r)
        })
        .collect();
    hits.sort_by(|(ta, a), (tb, b)| ta.cmp(tb).then(b.last_played.cmp(&a.last_played)).then_with(|| fold(&a.sort_title).cmp(&fold(&b.sort_title))));
    hits.into_iter().map(|(_, r)| r.id.clone()).collect()
}

pub fn platform_name(platform: &str) -> String {
    match platform {
        "windows" => "Windows".into(),
        "linux" => "Linux".into(),
        "" => String::new(),
        other => other.into(),
    }
}

pub fn source_name(kind: &str) -> String {
    match kind {
        "gog" => "GOG".into(),
        "epic" => "Epic Games".into(),
        "itch" => "itch.io".into(),
        "steam" => "Steam".into(),
        "lutris" => "Lutris".into(),
        "manual" => gettextrs::gettext("Added"),
        other => {
            let mut chars = other.chars();
            chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
        }
    }
}

pub fn platform_icon(platform: &str) -> &'static str {
    match platform {
        "windows" | "linux" | "MS-DOS" | "ScummVM" => "computer-symbolic",
        _ => "input-gaming-symbolic",
    }
}

pub fn source_icon(kind: &str) -> &'static str {
    match kind {
        "gog" => "source-gog-symbolic",
        "epic" => "source-epic-symbolic",
        "itch" => "source-itch-symbolic",
        "steam" => "source-steam-symbolic",
        "lutris" => "source-lutris-symbolic",
        _ => "folder-new-symbolic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: &str, last_played: i64, added: i64, hours: f64) -> Row {
        Row { id: fold(title), title: title.into(), sort_title: title.into(), last_played, added, hours, ..Row::default() }
    }

    #[test]
    fn sorts_put_the_never_played_last_and_break_ties_by_title() {
        let mut rows = vec![row("Zelda", 0, 5, 1.0), row("Celeste", 300, 0, 9.0), row("Ábzû", 0, 10, 2.0), row("Hades", 100, 5, 30.0)];
        let titles = |rows: &[Row]| rows.iter().map(|r| r.title.clone()).collect::<Vec<_>>();
        rows.sort_by(|a, b| Sort::LastPlayed.order(a, b));
        assert_eq!(titles(&rows), ["Celeste", "Hades", "Ábzû", "Zelda"]);
        rows.sort_by(|a, b| Sort::AToZ.order(a, b));
        assert_eq!(titles(&rows), ["Ábzû", "Celeste", "Hades", "Zelda"], "accents fold before they sort");
        rows.sort_by(|a, b| Sort::Newest.order(a, b));
        assert_eq!(titles(&rows), ["Ábzû", "Hades", "Zelda", "Celeste"]);
        rows.sort_by(|a, b| Sort::MostPlayed.order(a, b));
        assert_eq!(titles(&rows), ["Hades", "Celeste", "Ábzû", "Zelda"]);
    }

    #[test]
    fn a_search_folds_accents_and_wants_every_word() {
        let mut hades = row("Hades II", 0, 0, 0.0);
        hades.developer = "Supergiant Games".into();
        hades.platform = "windows".into();
        assert!(matches(&hades, &search_words("hades supergiant")));
        assert!(matches(&hades, &search_words("  WINDOWS ")));
        assert!(!matches(&hades, &search_words("hades nintendo")));
        assert!(matches(&row("Ōkami", 0, 0, 0.0), &search_words("okami")));
    }

    #[test]
    fn a_search_ranks_titles_that_start_with_it_first() {
        let mut xeno = row("Xenoblade Chronicles X", 50, 0, 0.0);
        xeno.developer = "Monolith Soft".into();
        let rows = [row("Super Mario Odyssey", 300, 0, 0.0), row("Mario Kart 8 Deluxe", 100, 0, 0.0), row("Paper Mario", 200, 0, 0.0), xeno];
        assert_eq!(rank(&rows, &search_words("mario")), ["mario kart 8 deluxe", "super mario odyssey", "paper mario"]);
        assert_eq!(rank(&rows, &search_words("mar kart")), ["mario kart 8 deluxe"]);
        assert_eq!(rank(&rows, &search_words("monolith")), ["xenoblade chronicles x"], "the developer counts, last");
        assert!(rank(&rows, &[]).is_empty());
    }

    #[test]
    fn a_view_round_trips_its_key() {
        for key in ["all", "favorites", "source:gog", "platform:Nintendo Switch"] {
            assert_eq!(View::parse(key).key(), key);
        }
        assert_eq!(View::parse("nonsense"), View::All);
    }
}
