use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS feeds (
    id INTEGER PRIMARY KEY,
    url TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    site_url TEXT,
    last_fetched_at TEXT,
    last_error TEXT
);

CREATE TABLE IF NOT EXISTS entries (
    id INTEGER PRIMARY KEY,
    feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    guid TEXT NOT NULL,
    title TEXT NOT NULL,
    link TEXT,
    content_html TEXT NOT NULL,
    published_at TEXT,
    is_read INTEGER NOT NULL DEFAULT 0,
    UNIQUE(feed_id, guid)
);

CREATE INDEX IF NOT EXISTS idx_entries_feed ON entries(feed_id, published_at DESC);
CREATE INDEX IF NOT EXISTS idx_entries_read ON entries(is_read, published_at DESC);
";

/// Create the schema in an already-open connection.
pub fn init_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(SCHEMA)
}

/// Open (creating if needed) the application database at `path` and apply the schema.
pub fn open(path: &Path) -> Result<Connection, rusqlite::Error> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    init_schema(&conn)?;
    Ok(conn)
}

/// Open an in-memory database with the schema applied (for tests).
#[cfg(test)]
pub fn open_memory() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    init_schema(&conn)?;
    Ok(conn)
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedDto {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub site_url: Option<String>,
    pub last_fetched_at: Option<String>,
    pub last_error: Option<String>,
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryDto {
    pub id: i64,
    pub feed_id: i64,
    pub title: String,
    pub link: Option<String>,
    pub content_html: String,
    pub published_at: Option<String>,
    pub is_read: bool,
}

/// Insert a feed or update its title/site_url if the URL already exists. Returns the feed id.
pub fn insert_or_update_feed(
    conn: &Connection,
    url: &str,
    title: &str,
    site_url: Option<&str>,
) -> Result<i64, rusqlite::Error> {
    conn.execute(
        "INSERT INTO feeds (url, title, site_url) VALUES (?1, ?2, ?3)
         ON CONFLICT(url) DO UPDATE SET title = ?2, site_url = COALESCE(?3, site_url)",
        params![url, title, site_url],
    )?;
    conn.query_row("SELECT id FROM feeds WHERE url = ?1", params![url], |r| {
        r.get(0)
    })
}

/// Insert an entry, or update the existing one for the same (feed, guid).
/// `guid` must be non-empty; callers pass the link when the feed omits a GUID.
pub fn upsert_entry(
    conn: &Connection,
    feed_id: i64,
    guid: &str,
    title: &str,
    link: Option<&str>,
    content_html: &str,
    published_at: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO entries (feed_id, guid, title, link, content_html, published_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(feed_id, guid) DO UPDATE SET
           title = excluded.title,
           link = excluded.link,
           content_html = excluded.content_html,
           published_at = COALESCE(excluded.published_at, entries.published_at)",
        params![feed_id, guid, title, link, content_html, published_at],
    )?;
    Ok(())
}

/// All feeds with unread entry counts, newest-fetch order not assumed (UI sorts if needed).
pub fn list_feeds(conn: &Connection) -> Result<Vec<FeedDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.url, f.title, f.site_url, f.last_fetched_at, f.last_error,
                (SELECT COUNT(*) FROM entries e WHERE e.feed_id = f.id AND e.is_read = 0)
         FROM feeds f ORDER BY f.title COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(FeedDto {
            id: r.get(0)?,
            url: r.get(1)?,
            title: r.get(2)?,
            site_url: r.get(3)?,
            last_fetched_at: r.get(4)?,
            last_error: r.get(5)?,
            unread_count: r.get(6)?,
        })
    })?;
    rows.collect()
}

/// List entries. `view` is "all" or "unread"; when `feed_id` is given only that
/// feed's entries are returned (all states), otherwise `view` filters globally.
pub fn list_entries(
    conn: &Connection,
    view: &str,
    feed_id: Option<i64>,
) -> Result<Vec<EntryDto>, String> {
    let sql: &str = match (view, feed_id) {
        (_, Some(_)) =>
            "SELECT id, feed_id, title, link, content_html, published_at, is_read
             FROM entries WHERE feed_id = ?1 ORDER BY published_at DESC, id DESC",
        ("unread", None) =>
            "SELECT id, feed_id, title, link, content_html, published_at, is_read
             FROM entries WHERE is_read = 0 ORDER BY published_at DESC, id DESC",
        ("all", None) =>
            "SELECT id, feed_id, title, link, content_html, published_at, is_read
             FROM entries ORDER BY published_at DESC, id DESC",
        _ => return Err(format!("unknown view: {view}")),
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = match feed_id {
        Some(id) => stmt
            .query_map(params![id], map_entry)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?,
        None => stmt
            .query_map([], map_entry)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?,
    };
    Ok(rows)
}

fn map_entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<EntryDto> {
    let is_read: i64 = r.get(6)?;
    Ok(EntryDto {
        id: r.get(0)?,
        feed_id: r.get(1)?,
        title: r.get(2)?,
        link: r.get(3)?,
        content_html: r.get(4)?,
        published_at: r.get(5)?,
        is_read: is_read != 0,
    })
}

pub fn set_read(
    conn: &Connection,
    entry_id: i64,
    is_read: bool,
) -> Result<(), String> {
    let n = conn
        .execute(
            "UPDATE entries SET is_read = ?1 WHERE id = ?2",
            params![is_read as i64, entry_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("entry {entry_id} not found"));
    }
    Ok(())
}

/// Delete a feed; entries cascade (verified by schema test).
pub fn delete_feed(conn: &Connection, feed_id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM feeds WHERE id = ?1", params![feed_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Record the outcome of a refresh attempt for one feed.
pub fn set_feed_fetch_result(
    conn: &Connection,
    feed_id: i64,
    fetched_at: &str,
    error: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE feeds SET last_fetched_at = ?1, last_error = ?2 WHERE id = ?3",
        params![fetched_at, error, feed_id],
    )?;
    Ok(())
}

pub fn get_feed(conn: &Connection, feed_id: i64) -> Result<Option<FeedDto>, rusqlite::Error> {
    conn.query_row(
        "SELECT f.id, f.url, f.title, f.site_url, f.last_fetched_at, f.last_error,
                (SELECT COUNT(*) FROM entries e WHERE e.feed_id = f.id AND e.is_read = 0)
         FROM feeds f WHERE f.id = ?1",
        params![feed_id],
        |r| {
            Ok(FeedDto {
                id: r.get(0)?,
                url: r.get(1)?,
                title: r.get(2)?,
                site_url: r.get(3)?,
                last_fetched_at: r.get(4)?,
                last_error: r.get(5)?,
                unread_count: r.get(6)?,
            })
        },
    )
    .optional()
}

pub fn feed_url_by_id(conn: &Connection, feed_id: i64) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row("SELECT url FROM feeds WHERE id = ?1", params![feed_id], |r| r.get(0))
        .optional()
}

pub fn feed_id_by_url(conn: &Connection, url: &str) -> Result<Option<i64>, rusqlite::Error> {
    conn.query_row("SELECT id FROM feeds WHERE url = ?1", params![url], |r| r.get(0))
        .optional()
}

pub fn all_feed_ids_and_urls(conn: &Connection) -> Result<Vec<(i64, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT id, url FROM feeds")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_creates_both_tables() {
        let conn = open_memory().unwrap();
        let feeds: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('feeds','entries')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(feeds, 2);
    }

    #[test]
    fn open_creates_file_with_schema() {
        let dir = std::env::temp_dir().join(format!("rss-schema-test-{}", std::process::id()));
        let path = dir.join("reader.db");
        let _ = std::fs::remove_file(&path);

        let conn = open(&path).unwrap();
        assert!(path.exists(), "database file should be created");
        let entries: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='entries'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(entries, 1);
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn guid_is_unique_per_feed() {
        let conn = open_memory().unwrap();
        conn.execute(
            "INSERT INTO feeds (url, title) VALUES ('https://a.example/feed', 'A')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO entries (feed_id, guid, title, content_html) VALUES (1, 'g1', 'T', '<p>x</p>')",
            [],
        )
        .unwrap();
        let dup = conn.execute(
            "INSERT INTO entries (feed_id, guid, title, content_html) VALUES (1, 'g1', 'T2', '<p>y</p>')",
            [],
        );
        assert!(dup.is_err(), "same guid twice in one feed must violate UNIQUE");
    }

    #[test]
    fn deleting_feed_cascades_to_entries() {
        let conn = open_memory().unwrap();
        conn.execute(
            "INSERT INTO feeds (url, title) VALUES ('https://a.example/feed', 'A')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO entries (feed_id, guid, title, content_html) VALUES (1, 'g1', 'T', '<p>x</p>')",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM feeds WHERE id = 1", []).unwrap();
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn pragma_foreign_keys_enabled_in_open() {
        let conn = open_memory().unwrap();
        let on: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(on, 1);
    }

    #[test]
    fn upsert_does_not_duplicate_entries_with_same_guid() {
        let conn = open_memory().unwrap();
        let feed_id = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();

        upsert_entry(&conn, feed_id, "g1", "Title v1", Some("https://a/1"), "<p>one</p>", Some("2026-01-01T00:00:00Z")).unwrap();
        upsert_entry(&conn, feed_id, "g1", "Title v2", Some("https://a/1"), "<p>two</p>", Some("2026-01-02T00:00:00Z")).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "re-fetch must not duplicate entries");
        let title: String = conn
            .query_row("SELECT title FROM entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Title v2", "existing entry is updated in place");
    }

    #[test]
    fn same_guid_in_different_feeds_is_allowed() {
        let conn = open_memory().unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        upsert_entry(&conn, a, "g1", "T", None, "<p>x</p>", None).unwrap();
        upsert_entry(&conn, b, "g1", "T", None, "<p>x</p>", None).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn delete_feed_removes_its_entries_only() {
        let conn = open_memory().unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        upsert_entry(&conn, a, "g1", "T", None, "<p>x</p>", None).unwrap();
        upsert_entry(&conn, b, "g2", "T", None, "<p>x</p>", None).unwrap();

        delete_feed(&conn, a).unwrap();

        let feeds: i64 = conn
            .query_row("SELECT COUNT(*) FROM feeds", [], |r| r.get(0))
            .unwrap();
        let entries: i64 = conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(feeds, 1);
        assert_eq!(entries, 1, "only the deleted feed's entries are removed");
    }

    #[test]
    fn list_entries_filters_and_orders_newest_first() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        upsert_entry(&conn, f, "g1", "Old", None, "<p>x</p>", Some("2026-01-01T00:00:00Z")).unwrap();
        upsert_entry(&conn, f, "g2", "New", None, "<p>x</p>", Some("2026-02-01T00:00:00Z")).unwrap();
        upsert_entry(&conn, f, "g3", "Mid", None, "<p>x</p>", Some("2026-01-15T00:00:00Z")).unwrap();

        let all = list_entries(&conn, "all", None).unwrap();
        let titles: Vec<&str> = all.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["New", "Mid", "Old"]);

        set_read(&conn, all[0].id, true).unwrap();
        let unread = list_entries(&conn, "unread", None).unwrap();
        assert_eq!(unread.len(), 2);
        assert!(unread.iter().all(|e| !e.is_read));

        let per_feed = list_entries(&conn, "all", Some(f)).unwrap();
        assert_eq!(per_feed.len(), 3, "feed view includes read entries");

        assert!(list_entries(&conn, "bogus", None).is_err());
    }

    #[test]
    fn unread_counts_are_per_feed() {
        let conn = open_memory().unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        upsert_entry(&conn, a, "g1", "T", None, "<p>x</p>", None).unwrap();
        upsert_entry(&conn, a, "g2", "T", None, "<p>x</p>", None).unwrap();
        upsert_entry(&conn, b, "g3", "T", None, "<p>x</p>", None).unwrap();

        let feeds = list_feeds(&conn).unwrap();
        let counts: Vec<(i64, i64)> = feeds.iter().map(|f| (f.id, f.unread_count)).collect();
        assert!(counts.contains(&(a, 2)));
        assert!(counts.contains(&(b, 1)));

        set_read(&conn, 1, true).unwrap();
        let feeds = list_feeds(&conn).unwrap();
        let a_feed = feeds.iter().find(|f| f.id == a).unwrap();
        assert_eq!(a_feed.unread_count, 1);
    }

    #[test]
    fn fetch_result_records_error_and_clears_it() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();

        set_feed_fetch_result(&conn, f, "2026-01-01T00:00:00Z", Some("timeout")).unwrap();
        let feed = get_feed(&conn, f).unwrap().unwrap();
        assert_eq!(feed.last_error.as_deref(), Some("timeout"));
        assert_eq!(feed.last_fetched_at.as_deref(), Some("2026-01-01T00:00:00Z"));

        set_feed_fetch_result(&conn, f, "2026-01-02T00:00:00Z", None).unwrap();
        let feed = get_feed(&conn, f).unwrap().unwrap();
        assert!(feed.last_error.is_none(), "successful refresh clears the error");
    }
}
