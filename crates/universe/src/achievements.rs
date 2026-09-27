use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::game::Game;

/// `games/<id>/achievements.json`: the list the game's source last gave, with the unlocks a session saw since.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Cache {
    pub source: String,
    pub fetched_at: String,
    pub items: Vec<Achievement>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Achievement {
    pub key: String,
    pub name: String,
    pub description: String,
    /// RFC 3339; empty while locked.
    pub unlocked_at: String,
    /// The store withholds the name and description until it is unlocked.
    pub hidden: bool,
    pub icon: String,
    pub icon_locked: String,
    /// Percent of the store's players who have it.
    pub rarity: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
pub struct Summary {
    pub total: usize,
    pub unlocked: usize,
}

impl Cache {
    pub fn summary(&self) -> Summary {
        Summary { total: self.items.len(), unlocked: self.items.iter().filter(|a| !a.unlocked_at.is_empty()).count() }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let s = self.summary();
        serde_json::json!({
            "source": self.source,
            "fetched_at": self.fetched_at,
            "total": s.total,
            "unlocked": s.unlocked,
            "items": self.items,
        })
    }

    /// Marks `item` unlocked, adding it when the list lacks it; false when it already was.
    pub fn unlock(&mut self, item: Achievement) -> bool {
        match self.items.iter_mut().find(|a| a.key == item.key) {
            Some(a) if !a.unlocked_at.is_empty() => false,
            Some(a) => {
                a.unlocked_at = item.unlocked_at;
                true
            }
            None => {
                self.items.push(item);
                true
            }
        }
    }

    /// A fresh list keeps an unlock the store has not heard of yet: a session offline, a sync that failed.
    pub fn refreshed(&self, mut fresh: Cache) -> Cache {
        for a in fresh.items.iter_mut().filter(|a| a.unlocked_at.is_empty()) {
            if let Some(old) = self.items.iter().find(|o| o.key == a.key && !o.unlocked_at.is_empty()) {
                a.unlocked_at = old.unlocked_at.clone();
            }
        }
        fresh
    }
}

pub fn path(game: &Game) -> PathBuf {
    game.dir().join("achievements.json")
}

pub fn read(path: &Path) -> Option<Cache> {
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).inspect_err(|e| tracing::warn!("{}: {e}", path.display())).ok()
}

pub fn write(path: &Path, cache: &Cache) -> crate::Result<()> {
    crate::game::atomic_write(path, serde_json::to_string_pretty(cache)?.as_bytes())
}

/// `unlocked_at` comes as the store gives it (`2024-05-01T20:11:04+0000` for GOG); RFC 3339 when it parses.
pub fn normalized_time(s: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(s)
        .or_else(|_| chrono::DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z"))
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|_| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(key: &str, at: &str) -> Achievement {
        Achievement { key: key.into(), name: key.to_uppercase(), unlocked_at: at.into(), ..Default::default() }
    }

    #[test]
    fn an_unlock_lands_once_and_a_refresh_keeps_it() {
        let mut c = Cache { source: "gog".into(), items: vec![item("a", ""), item("b", "")], ..Default::default() };
        assert!(c.unlock(item("a", "2026-09-24T20:00:00+02:00")));
        assert!(!c.unlock(item("a", "2026-09-24T21:00:00+02:00")), "the first unlock stands");
        assert!(c.unlock(item("new", "2026-09-24T21:00:00+02:00")), "a key the list lacked is added");
        assert_eq!(c.summary(), Summary { total: 3, unlocked: 2 });
        assert_eq!(c.items[0].unlocked_at, "2026-09-24T20:00:00+02:00");

        let fresh = Cache { source: "gog".into(), items: vec![item("a", ""), item("b", "2026-09-20T10:00:00+00:00")], ..Default::default() };
        let merged = c.refreshed(fresh);
        assert_eq!(merged.items[0].unlocked_at, "2026-09-24T20:00:00+02:00", "the store has not synced it yet");
        assert_eq!(merged.items[1].unlocked_at, "2026-09-20T10:00:00+00:00");
        assert_eq!(merged.items.len(), 2, "the store's list is the list");
    }

    #[test]
    fn gog_times_become_rfc3339() {
        assert_eq!(normalized_time("2024-05-01T20:11:04+0000"), "2024-05-01T20:11:04+00:00");
        assert_eq!(normalized_time("2024-05-01T20:11:04+02:00"), "2024-05-01T20:11:04+02:00");
        assert_eq!(normalized_time(""), "");
    }

    #[test]
    fn the_cache_round_trips_and_a_bad_file_reads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("achievements.json");
        let c = Cache { source: "gog".into(), fetched_at: "t".into(), items: vec![Achievement { rarity: Some(12.5), ..item("a", "") }] };
        write(&p, &c).unwrap();
        assert_eq!(read(&p), Some(c));
        std::fs::write(&p, "{").unwrap();
        assert_eq!(read(&p), None);
    }
}
