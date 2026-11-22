use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
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

/// Folders group feeds; a NULL `feeds.folder_id` means uncategorized.
pub const FOLDERS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);
";

/// Columns added after the original schema shipped. Applied on every open;
/// each statement is skipped when the column already exists.
const COLUMN_MIGRATIONS: &[&str] = &[
    "ALTER TABLE feeds ADD COLUMN folder_id INTEGER REFERENCES folders(id)",
    "ALTER TABLE entries ADD COLUMN is_starred INTEGER NOT NULL DEFAULT 0",
];

/// Indexes that depend on migrated columns.
const POST_MIGRATION: &str = "
CREATE INDEX IF NOT EXISTS idx_entries_starred ON entries(is_starred, published_at DESC);
";

/// Create the schema in an already-open connection. Safe to call on every open:
/// all statements are idempotent and column migrations tolerate being re-run.
pub fn init_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(SCHEMA)?;
    conn.execute_batch(FOLDERS_TABLE)?;
    for sql in COLUMN_MIGRATIONS {
        match conn.execute_batch(sql) {
            Ok(()) => {}
            Err(e) if e.to_string().contains("duplicate column name") => {}
            Err(e) => return Err(e),
        }
    }
    conn.execute_batch(POST_MIGRATION)
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
    /// NULL means uncategorized.
    pub folder_id: Option<i64>,
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderDto {
    pub id: i64,
    pub name: String,
    pub unread_count: i64,
    pub feeds: Vec<FeedDto>,
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
    pub is_starred: bool,
}

/// What set of entries to list. Serialized from the frontend as
/// `{kind:"all"}` / `{kind:"feed",id}` / `{kind:"folder",id}` /
/// `{kind:"starred"}` — the starred view is global and never combines with
/// a feed or folder selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EntryScope {
    All,
    Feed { id: i64 },
    Folder { id: i64 },
    Starred,
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
        "SELECT f.id, f.url, f.title, f.site_url, f.last_fetched_at, f.last_error, f.folder_id,
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
            folder_id: r.get(6)?,
            unread_count: r.get(7)?,
        })
    })?;
    rows.collect()
}

/// List entries for a scope combined with a view (`"all"` or `"unread"`).
/// Newest first. The starred scope is independent of feed/folder selection.
pub fn list_entries(
    conn: &Connection,
    view: &str,
    scope: EntryScope,
) -> Result<Vec<EntryDto>, String> {
    if view != "all" && view != "unread" {
        return Err(format!("unknown view: {view}"));
    }
    let mut conditions: Vec<&str> = Vec::new();
    let mut id_params: Vec<i64> = Vec::new();
    match scope {
        EntryScope::All => {}
        EntryScope::Feed { id } => {
            conditions.push("feed_id = ?");
            id_params.push(id);
        }
        EntryScope::Folder { id } => {
            conditions.push("feed_id IN (SELECT id FROM feeds WHERE folder_id = ?)");
            id_params.push(id);
        }
        EntryScope::Starred => conditions.push("is_starred = 1"),
    }
    if view == "unread" {
        conditions.push("is_read = 0");
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    let sql = format!(
        "SELECT id, feed_id, title, link, content_html, published_at, is_read, is_starred
         FROM entries {where_clause} ORDER BY published_at DESC, id DESC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let param_refs: Vec<&dyn rusqlite::ToSql> =
        id_params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    stmt.query_map(param_refs.as_slice(), map_entry)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn map_entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<EntryDto> {
    let is_read: i64 = r.get(6)?;
    let is_starred: i64 = r.get(7)?;
    Ok(EntryDto {
        id: r.get(0)?,
        feed_id: r.get(1)?,
        title: r.get(2)?,
        link: r.get(3)?,
        content_html: r.get(4)?,
        published_at: r.get(5)?,
        is_read: is_read != 0,
        is_starred: is_starred != 0,
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

/// Star or unstar an entry. Independent of read state.
pub fn set_starred(
    conn: &Connection,
    entry_id: i64,
    is_starred: bool,
) -> Result<(), String> {
    let n = conn
        .execute(
            "UPDATE entries SET is_starred = ?1 WHERE id = ?2",
            params![is_starred as i64, entry_id],
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
        "SELECT f.id, f.url, f.title, f.site_url, f.last_fetched_at, f.last_error, f.folder_id,
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
                folder_id: r.get(6)?,
                unread_count: r.get(7)?,
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

/// Create a folder. The name MUST be non-empty.
pub fn create_folder(conn: &Connection, name: &str) -> Result<FolderDto, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("folder name is empty".into());
    }
    conn.execute("INSERT INTO folders (name) VALUES (?1)", params![name])
        .map_err(|e| e.to_string())?;
    let id = conn
        .query_row("SELECT last_insert_rowid()", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    Ok(FolderDto { id, name: name.into(), unread_count: 0, feeds: vec![] })
}

/// Rename a folder. The new name MUST be non-empty.
pub fn rename_folder(conn: &Connection, folder_id: i64, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("folder name is empty".into());
    }
    let n = conn
        .execute(
            "UPDATE folders SET name = ?1 WHERE id = ?2",
            params![name, folder_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("folder {folder_id} not found"));
    }
    Ok(())
}

/// Delete a folder. Its feeds move to uncategorized (folder_id = NULL);
/// feeds and entries are never removed by this operation.
pub fn delete_folder(conn: &Connection, folder_id: i64) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE feeds SET folder_id = NULL WHERE folder_id = ?1",
        params![folder_id],
    )
    .map_err(|e| e.to_string())?;
    let n = tx
        .execute("DELETE FROM folders WHERE id = ?1", params![folder_id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("folder {folder_id} not found"));
    }
    tx.commit().map_err(|e| e.to_string())
}

/// Move a feed to a folder, or to uncategorized when `folder_id` is None.
pub fn move_feed_to_folder(
    conn: &Connection,
    feed_id: i64,
    folder_id: Option<i64>,
) -> Result<(), String> {
    if let Some(fid) = folder_id {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM folders WHERE id = ?1",
                params![fid],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            > 0;
        if !exists {
            return Err(format!("folder {fid} not found"));
        }
    }
    let n = conn
        .execute(
            "UPDATE feeds SET folder_id = ?1 WHERE id = ?2",
            params![folder_id, feed_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("feed {feed_id} not found"));
    }
    Ok(())
}

/// Folders ordered by name using default string comparison (case-sensitive,
/// with `id` as a deterministic tie-break), each with its feeds (title order)
/// nested and an unread count aggregated over those feeds in SQL. Uncategorized
/// feeds are not included here; they are the feeds with `folder_id == None`.
pub fn list_folders(conn: &Connection) -> Result<Vec<FolderDto>, String> {
    let mut folders: Vec<FolderDto> = {
        let mut stmt = conn
            .prepare(
                "SELECT f.id, f.name,
                        (SELECT COUNT(*) FROM entries e
                          JOIN feeds fd ON e.feed_id = fd.id
                          WHERE fd.folder_id = f.id AND e.is_read = 0)
                 FROM folders f ORDER BY f.name, f.id",
            )
            .map_err(|e| e.to_string())?;
        stmt
            .query_map([], |r| {
                Ok(FolderDto {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    unread_count: r.get(2)?,
                    feeds: vec![],
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?
    };

    let feeds = list_feeds(conn).map_err(|e| e.to_string())?;
    for feed in feeds {
        if let Some(folder_id) = feed.folder_id {
            if let Some(folder) = folders.iter_mut().find(|f| f.id == folder_id) {
                folder.feeds.push(feed);
            }
        }
    }
    Ok(folders)
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
    fn schema_creates_folders_and_migrated_columns() {
        let conn = open_memory().unwrap();
        let folders: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='folders'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(folders, 1);

        let folder_cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(folders)").unwrap();
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            rows
        };
        assert!(
            !folder_cols.contains(&"position".to_string()),
            "position column is gone; folders are ordered by name"
        );

        let feed_cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(feeds)").unwrap();
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            rows
        };
        assert!(feed_cols.contains(&"folder_id".to_string()));

        let entry_cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(entries)").unwrap();
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            rows
        };
        assert!(entry_cols.contains(&"is_starred".to_string()));
    }

    #[test]
    fn init_schema_is_idempotent_on_existing_databases() {
        // Simulate a pre-upgrade database: original schema only, no folders.
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // Two upgrade passes must both succeed (duplicate column tolerated).
        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap();

        let starred: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('entries') WHERE name='is_starred'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(starred, 1);
    }

    #[test]
    fn folder_crud_and_feed_assignment() {
        let conn = open_memory().unwrap();
        let tech = create_folder(&conn, "Tech").unwrap();
        let blogs = create_folder(&conn, " Blogs ").unwrap();
        assert_eq!(blogs.name, "Blogs", "name is trimmed");

        // Name order (default string comparison), independent of creation order.
        let folders = list_folders(&conn).unwrap();
        let ids: Vec<i64> = folders.iter().map(|f| f.id).collect();
        assert_eq!(ids, vec![blogs.id, tech.id], "\"Blogs\" sorts before \"Tech\"");

        assert!(create_folder(&conn, "   ").is_err(), "empty name rejected");
        assert!(rename_folder(&conn, tech.id, "").is_err(), "empty rename rejected");
        rename_folder(&conn, tech.id, "Technology").unwrap();
        assert!(rename_folder(&conn, 999, "X").is_err(), "unknown folder rejected");

        let feed = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        move_feed_to_folder(&conn, feed, Some(tech.id)).unwrap();
        let a = get_feed(&conn, feed).unwrap().unwrap();
        assert_eq!(a.folder_id, Some(tech.id));

        assert!(move_feed_to_folder(&conn, feed, Some(999)).is_err(), "unknown folder rejected");
        assert!(move_feed_to_folder(&conn, 999, None).is_err(), "unknown feed rejected");

        // Deleting the folder reassigns its feed to uncategorized.
        delete_folder(&conn, tech.id).unwrap();
        let a = get_feed(&conn, feed).unwrap().unwrap();
        assert_eq!(a.folder_id, None, "feed moves to uncategorized");
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0, "no entries touched");
        let feed_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM feeds", [], |r| r.get(0))
            .unwrap();
        assert_eq!(feed_rows, 1, "feed survives folder deletion");
        assert!(delete_folder(&conn, tech.id).is_err(), "double delete rejected");
    }

    #[test]
    fn list_folders_orders_by_name_with_default_comparison() {
        let conn = open_memory().unwrap();
        let names = |c: &Connection| -> Vec<String> {
            list_folders(c).unwrap().into_iter().map(|f| f.name).collect()
        };

        // Creation order does not matter: name order wins.
        let tech = create_folder(&conn, "Tech").unwrap();
        create_folder(&conn, "Books").unwrap();
        assert_eq!(names(&conn), vec!["Books", "Tech"]);

        // Default string comparison is case-sensitive: 'B' (66) sorts before 'a' (97).
        create_folder(&conn, "apple").unwrap();
        create_folder(&conn, "Banana").unwrap();
        assert_eq!(names(&conn), vec!["Banana", "Books", "Tech", "apple"]);

        // Renaming moves the folder to the position implied by its new name.
        rename_folder(&conn, tech.id, "Arts").unwrap();
        assert_eq!(names(&conn), vec!["Arts", "Banana", "Books", "apple"]);
    }

    #[test]
    fn list_folders_aggregates_unread_and_excludes_uncategorized() {
        let conn = open_memory().unwrap();
        let tech = create_folder(&conn, "Tech").unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        let c = insert_or_update_feed(&conn, "https://c.example/feed", "C", None).unwrap();
        move_feed_to_folder(&conn, a, Some(tech.id)).unwrap();
        move_feed_to_folder(&conn, b, Some(tech.id)).unwrap();

        upsert_entry(&conn, a, "g1", "T", None, "<p>x</p>", None).unwrap();
        upsert_entry(&conn, a, "g2", "T", None, "<p>x</p>", None).unwrap();
        upsert_entry(&conn, b, "g3", "T", None, "<p>x</p>", None).unwrap();
        upsert_entry(&conn, c, "g4", "T", None, "<p>x</p>", None).unwrap();

        let folders = list_folders(&conn).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].unread_count, 3, "aggregated over the folder's feeds");
        assert_eq!(folders[0].feeds.len(), 2, "both assigned feeds nested");

        // Uncategorized feed is visible via list_feeds, not via any folder.
        let uncategorized: Vec<_> = list_feeds(&conn)
            .unwrap()
            .into_iter()
            .filter(|f| f.folder_id.is_none())
            .collect();
        assert_eq!(uncategorized.len(), 1);
        assert_eq!(uncategorized[0].id, c);

        // Unread count tracks read state.
        set_read(&conn, 1, true).unwrap();
        let folders = list_folders(&conn).unwrap();
        assert_eq!(folders[0].unread_count, 2);
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

        let all = list_entries(&conn, "all", EntryScope::All).unwrap();
        let titles: Vec<&str> = all.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["New", "Mid", "Old"]);

        set_read(&conn, all[0].id, true).unwrap();
        let unread = list_entries(&conn, "unread", EntryScope::All).unwrap();
        assert_eq!(unread.len(), 2);
        assert!(unread.iter().all(|e| !e.is_read));

        let per_feed = list_entries(&conn, "all", EntryScope::Feed { id: f }).unwrap();
        assert_eq!(per_feed.len(), 3, "feed view includes read entries");

        assert!(list_entries(&conn, "bogus", EntryScope::All).is_err());
    }

    #[test]
    fn list_entries_scopes_combine_with_views() {
        let conn = open_memory().unwrap();
        let tech = create_folder(&conn, "Tech").unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        let c = insert_or_update_feed(&conn, "https://c.example/feed", "C", None).unwrap();
        move_feed_to_folder(&conn, a, Some(tech.id)).unwrap();
        move_feed_to_folder(&conn, b, Some(tech.id)).unwrap();
        // c stays uncategorized.
        upsert_entry(&conn, a, "g1", "E1", None, "<p>x</p>", Some("2026-01-01T00:00:00Z")).unwrap();
        upsert_entry(&conn, a, "g2", "E2", None, "<p>x</p>", Some("2026-01-02T00:00:00Z")).unwrap();
        upsert_entry(&conn, b, "g3", "E4", None, "<p>x</p>", Some("2026-01-04T00:00:00Z")).unwrap();
        upsert_entry(&conn, c, "g4", "E3", None, "<p>x</p>", Some("2026-01-03T00:00:00Z")).unwrap();

        // e1 (feed a): starred + unread; e2: read; e4 (feed b): unread; e3 (feed c): starred + read.
        set_starred(&conn, 1, true).unwrap();
        set_starred(&conn, 4, true).unwrap();
        set_read(&conn, 2, true).unwrap();
        set_read(&conn, 4, true).unwrap();

        let titles = |entries: Vec<EntryDto>| -> Vec<String> {
            entries.into_iter().map(|e| e.title).collect()
        };

        assert_eq!(
            titles(list_entries(&conn, "all", EntryScope::All).unwrap()),
            vec!["E4", "E3", "E2", "E1"],
            "newest first"
        );
        assert_eq!(
            titles(list_entries(&conn, "all", EntryScope::Folder { id: tech.id }).unwrap()),
            vec!["E4", "E2", "E1"],
            "folder scope covers its feeds only"
        );
        assert_eq!(
            titles(list_entries(&conn, "all", EntryScope::Feed { id: c }).unwrap()),
            vec!["E3"],
            "feed scope unchanged"
        );
        assert_eq!(
            titles(list_entries(&conn, "all", EntryScope::Starred).unwrap()),
            vec!["E3", "E1"],
            "starred scope is global across feeds and folders"
        );
        assert_eq!(
            titles(list_entries(&conn, "unread", EntryScope::Starred).unwrap()),
            vec!["E1"],
            "view combines with starred scope"
        );
        assert_eq!(
            titles(list_entries(&conn, "unread", EntryScope::Folder { id: tech.id }).unwrap()),
            vec!["E4", "E1"]
        );
    }

    #[test]
    fn set_starred_is_independent_of_read_state_and_persists() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        upsert_entry(&conn, f, "g1", "T", None, "<p>x</p>", None).unwrap();

        set_read(&conn, 1, true).unwrap();
        set_starred(&conn, 1, true).unwrap();
        let entry = &list_entries(&conn, "all", EntryScope::All).unwrap()[0];
        assert!(entry.is_starred, "star persisted");
        assert!(entry.is_read, "read state untouched by starring");

        set_starred(&conn, 1, false).unwrap();
        let entry = &list_entries(&conn, "all", EntryScope::All).unwrap()[0];
        assert!(!entry.is_starred);
        assert!(entry.is_read, "read state untouched by unstarring");

        assert!(set_starred(&conn, 999, true).is_err(), "unknown entry rejected");
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
