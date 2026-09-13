use rusqlite::{params, Connection};

use crate::library::Resolved;

pub const SCHEMA_VERSION: i64 = 1;

pub struct Index {
    conn: Connection,
}

impl Index {
    pub fn open_memory() -> crate::Result<Index> {
        let idx = Index { conn: Connection::open_in_memory()? };
        idx.ensure_schema()?;
        Ok(idx)
    }

    fn ensure_schema(&self) -> crate::Result<()> {
        let v: i64 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if v != SCHEMA_VERSION {
            self.conn.execute_batch(
                "DROP TABLE IF EXISTS games; DROP TABLE IF EXISTS sessions; DROP TABLE IF EXISTS recordings;
                 DROP TABLE IF EXISTS media; DROP TABLE IF EXISTS journal; DROP TABLE IF EXISTS games_fts;",
            )?;
        }
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS games (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, sort_title TEXT, platform TEXT, release_year INTEGER,
                hidden INTEGER, favorite INTEGER, removed INTEGER, source_kind TEXT, source_id TEXT, installed INTEGER,
                hours REAL, play_count INTEGER, last_played TEXT, genres TEXT, json TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS sessions (session TEXT, game TEXT, started_at TEXT, ended_at TEXT, duration_s INTEGER, source TEXT, recording TEXT, PRIMARY KEY (game, session));
             CREATE TABLE IF NOT EXISTS recordings (game TEXT, session TEXT, path TEXT PRIMARY KEY, size INTEGER, created_at TEXT);
             CREATE TABLE IF NOT EXISTS media (game TEXT, slot TEXT, path TEXT, PRIMARY KEY (game, slot, path));
             CREATE TABLE IF NOT EXISTS journal (game TEXT, session TEXT, title TEXT, written_at TEXT, lang TEXT, PRIMARY KEY (game, session));
             CREATE VIRTUAL TABLE IF NOT EXISTS games_fts USING fts5(id UNINDEXED, title, genres);",
        )?;
        self.conn.execute(&format!("PRAGMA user_version = {SCHEMA_VERSION}"), [])?;
        Ok(())
    }

    pub fn rebuild(&mut self, games: &[Resolved]) -> crate::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM games; DELETE FROM sessions; DELETE FROM recordings; DELETE FROM media; DELETE FROM journal; DELETE FROM games_fts;")?;
        for r in games {
            insert_game(&tx, r)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn upsert(&mut self, r: &Resolved) -> crate::Result<()> {
        let tx = self.conn.transaction()?;
        for t in ["games", "sessions", "recordings", "media", "journal", "games_fts"] {
            let col = if t == "games" || t == "games_fts" { "id" } else { "game" };
            tx.execute(&format!("DELETE FROM {t} WHERE {col} = ?1"), params![r.game.id])?;
        }
        insert_game(&tx, r)?;
        tx.commit()?;
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> crate::Result<()> {
        for t in ["games", "sessions", "recordings", "media", "journal", "games_fts"] {
            let col = if t == "games" || t == "games_fts" { "id" } else { "game" };
            self.conn.execute(&format!("DELETE FROM {t} WHERE {col} = ?1"), params![id])?;
        }
        Ok(())
    }

    pub fn search(&self, query: &str) -> crate::Result<Vec<String>> {
        let q: String = query.split_whitespace().map(|w| format!("\"{}\"*", w.replace('"', ""))).collect::<Vec<_>>().join(" ");
        let mut st = self.conn.prepare("SELECT id FROM games_fts WHERE games_fts MATCH ?1 ORDER BY rank")?;
        let rows = st.query_map(params![q], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn count(&self) -> crate::Result<i64> {
        Ok(self.conn.query_row("SELECT count(*) FROM games", [], |r| r.get(0))?)
    }
}

fn insert_game(tx: &rusqlite::Transaction, r: &Resolved) -> crate::Result<()> {
    let g = &r.game;
    tx.execute(
        "INSERT INTO games (id, title, sort_title, platform, release_year, hidden, favorite, removed, source_kind, source_id, installed, hours, play_count, last_played, genres, json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            g.id, g.title, if g.sort_title.is_empty() { &g.title } else { &g.sort_title }, r.effective.platform, g.release_year, g.hidden as i32, g.favorite as i32,
            (!g.removed_at.is_empty()) as i32, g.source.kind, g.source.gog_id, g.is_installed() as i32, r.stats.hours, r.stats.play_count as i64,
            r.stats.last_played, g.metadata.genres.join(", "), r.to_json().to_string()
        ],
    )?;
    tx.execute("INSERT INTO games_fts (id, title, genres) VALUES (?1, ?2, ?3)", params![g.id, g.title, g.metadata.genres.join(" ")])?;
    for s in &r.sessions {
        tx.execute(
            "INSERT OR REPLACE INTO sessions (session, game, started_at, ended_at, duration_s, source, recording) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![s.session, g.id, s.started_at, s.ended_at, s.duration_s as i64, s.source, s.recording],
        )?;
        if let Some(rec) = &s.recording {
            let size = std::fs::metadata(rec).map(|m| m.len() as i64).unwrap_or(0);
            tx.execute("INSERT OR REPLACE INTO recordings (game, session, path, size, created_at) VALUES (?1, ?2, ?3, ?4, ?5)", params![g.id, s.session, rec, size, s.ended_at])?;
        }
    }
    for (slot, path) in &r.media {
        tx.execute("INSERT OR REPLACE INTO media (game, slot, path) VALUES (?1, ?2, ?3)", params![g.id, slot, path])?;
    }
    for e in &r.journal {
        tx.execute("INSERT OR REPLACE INTO journal (game, session, title, written_at, lang) VALUES (?1, ?2, ?3, ?4, ?5)", params![g.id, e.session, e.title, e.written_at, e.lang])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn rebuild_and_search() {
        let mut idx = Index::open_memory().unwrap();
        let mut g = Game::new("The Technomancer");
        g.metadata.genres = vec!["RPG".into()];
        let r = Resolved { game: g, ..Default::default() };
        let r2 = Resolved { game: Game::new("Mini Metro"), ..Default::default() };
        idx.rebuild(&[r, r2]).unwrap();
        assert_eq!(idx.count().unwrap(), 2);
        assert_eq!(idx.search("techno").unwrap(), vec!["the-technomancer"]);
        assert_eq!(idx.search("rpg").unwrap(), vec!["the-technomancer"]);
        idx.remove("mini-metro").unwrap();
        assert_eq!(idx.count().unwrap(), 1);
    }
}
