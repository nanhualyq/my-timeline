use chrono::Utc;
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
    "ALTER TABLE feeds ADD COLUMN custom_title TEXT",
    // Arrival time. Needed for a total ordering: `published_at` is nullable,
    // and keyset paging cannot compare against NULL.
    "ALTER TABLE entries ADD COLUMN created_at TEXT",
    // Row detail: a plain-text snippet and an absolute cover image URL, both
    // derived at ingest. NULL means "stored before these existed", which is
    // exactly what the one-time backfill looks for.
    "ALTER TABLE entries ADD COLUMN excerpt TEXT",
    "ALTER TABLE entries ADD COLUMN image_url TEXT",
    // Shared-state bookkeeping. `uuid` is the identity a folder keeps when it
    // is copied to another installation; the ids above are local and never
    // leave this database. Each independently settable field carries the
    // counter of the edit that set it, so two installations can resolve a
    // conflict without comparing clocks.
    "ALTER TABLE folders ADD COLUMN uuid TEXT",
    "ALTER TABLE folders ADD COLUMN name_ctr INTEGER NOT NULL DEFAULT 0",
    "ALTER TABLE folders ADD COLUMN sync_ctr INTEGER NOT NULL DEFAULT 0",
    "ALTER TABLE feeds ADD COLUMN sync_ctr INTEGER NOT NULL DEFAULT 0",
    "ALTER TABLE feeds ADD COLUMN custom_title_ctr INTEGER NOT NULL DEFAULT 0",
    "ALTER TABLE feeds ADD COLUMN folder_ctr INTEGER NOT NULL DEFAULT 0",
];

/// Indexes that depend on migrated columns.
const POST_MIGRATION: &str = "
CREATE INDEX IF NOT EXISTS idx_entries_starred ON entries(is_starred, published_at DESC);
-- Ordering position for every paginated query. Must be an expression index
-- because the key coalesces two columns; the query text has to match it
-- verbatim to be usable.
CREATE INDEX IF NOT EXISTS idx_entries_sort
  ON entries(COALESCE(published_at, created_at) DESC, id DESC);
";

/// Tables that carry state shared between installations. Created on every open;
/// every statement is idempotent.
pub const SYNC_TABLES: &str = "
CREATE TABLE IF NOT EXISTS app_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- One row per entry carrying shared intent. Keyed by the copyable identity
-- rather than a local id, so a mark can be recorded before the entry itself has
-- been fetched. read and star are nullable on purpose: NULL means never set,
-- which is different from an explicit 0 and is what lets a mark-unread made on
-- one installation survive a merge instead of being reinstated.
CREATE TABLE IF NOT EXISTS sync_marks (
    feed_url TEXT NOT NULL,
    guid TEXT NOT NULL,
    day TEXT NOT NULL,
    read INTEGER,
    read_ctr INTEGER NOT NULL DEFAULT 0,
    star INTEGER,
    star_ctr INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (feed_url, guid)
);

CREATE INDEX IF NOT EXISTS idx_sync_marks_day ON sync_marks(day);

-- Removals. A feed or folder survives a merge only when it was created at a
-- higher counter than the removal that names it.
CREATE TABLE IF NOT EXISTS sync_tombstones (
    kind TEXT NOT NULL,
    key TEXT NOT NULL,
    ctr INTEGER NOT NULL,
    day TEXT NOT NULL,
    PRIMARY KEY (kind, key)
);
";

/// How long a read mark stays shareable. Stars are exempt.
pub const READ_WINDOW_DAYS: i64 = 7;

/// How long a removal is remembered. Must outlive the longest realistic gap
/// between two refreshes of the same library, so an installation that has been
/// out of use still learns about the removals it missed.
pub const TOMBSTONE_RETENTION_DAYS: i64 = 90;

pub const TOMBSTONE_FEED: &str = "feed";
pub const TOMBSTONE_FOLDER: &str = "folder";

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
    // Arrival time for entries stored before the column existed. This only
    // ever fills rows that still have none, so it is safe to run on every open.
    conn.execute(
        "UPDATE entries SET created_at = datetime('now') WHERE created_at IS NULL",
        [],
    )?;
    conn.execute_batch(POST_MIGRATION)?;
    conn.execute_batch(SYNC_TABLES)?;
    backfill_folder_uuids(conn)?;
    ensure_device_id(conn)?;
    seed_marks_once(conn)?;
    // Derive row detail for entries stored before those columns existed. Only
    // touches rows whose excerpt is still NULL, so it self-terminates and is
    // safe to call on every open.
    backfill_row_details(conn)?;
    Ok(())
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

// --- Shared-state bookkeeping -------------------------------------------

/// Calendar day, as the short ISO prefix every stored timestamp shares.
fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

fn days_ago(days: i64) -> String {
    (Utc::now() - chrono::Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

fn parse_day(text: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

/// Give every folder an identity that survives being copied to another
/// installation, and enforce uniqueness now that none are NULL. Runs on every
/// open; the `WHERE uuid IS NULL` guard makes it a no-op after the first.
fn backfill_folder_uuids(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "UPDATE folders SET uuid = 'f-' || lower(hex(randomblob(8))) WHERE uuid IS NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_folders_uuid ON folders(uuid);",
    )
}

fn ensure_device_id(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO app_meta (key, value)
         VALUES ('device_id', 'dev-' || lower(hex(randomblob(6))))",
        [],
    )?;
    Ok(())
}

pub fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT value FROM app_meta WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .optional()
}

pub fn meta_set(conn: &Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO app_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// This installation's identity. Written once and then stable for the life of
/// the database; it names the writer of a document for diagnostics and never
/// decides a merge.
// Only the tests read it today; a transport will name the writer with it.
#[allow(dead_code)]
pub fn device_id(conn: &Connection) -> Result<String, rusqlite::Error> {
    meta_get(conn, "device_id")?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

/// Hand out the next ordering value. Every shareable local edit takes one, so
/// values are unique per field and comparable across installations without
/// either side consulting a clock.
pub fn next_counter(conn: &Connection) -> Result<i64, rusqlite::Error> {
    conn.query_row(
        "INSERT INTO app_meta (key, value) VALUES ('counter', '1')
         ON CONFLICT(key) DO UPDATE
           SET value = CAST(CAST(app_meta.value AS INTEGER) + 1 AS TEXT)
         RETURNING CAST(value AS INTEGER)",
        [],
        |r| r.get(0),
    )
}

/// Raise the counter above every value an imported document carried, so an edit
/// made after seeing those values outranks them everywhere.
pub fn raise_counter(conn: &Connection, floor: i64) -> Result<(), rusqlite::Error> {
    if floor <= 0 {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO app_meta (key, value) VALUES ('counter', CAST(?1 AS TEXT))
         ON CONFLICT(key) DO UPDATE
           SET value = CAST(MAX(CAST(app_meta.value AS INTEGER), ?1) AS TEXT)",
        params![floor],
    )?;
    Ok(())
}

/// The day before which read marks are no longer shared. It is stored rather
/// than recomputed so that two installations reading the same document agree on
/// the window: `today - 7d` evaluated separately on each device would make every
/// merge look like a change.
///
/// The advance is capped at one day per call, so a device whose clock is far
/// ahead can never jump the horizon forward and discard marks that another
/// device, and its reader, still consider current.
pub fn advance_read_horizon(
    conn: &Connection,
    observed: Option<&str>,
) -> Result<String, rusqlite::Error> {
    let current = meta_get(conn, "read_horizon_day")?;
    let mut target = days_ago(READ_WINDOW_DAYS);
    // Only a well-formed date is accepted from a document, which also keeps the
    // value safe to interpolate into statements further down.
    if let Some(day) = observed.filter(|d| parse_day(d).is_some()) {
        if day > target.as_str() {
            target = day.to_string();
        }
    }
    if let Some(day) = current.as_deref() {
        if day > target.as_str() {
            target = day.to_string();
        }
    }
    if let Some(day) = current.as_deref().and_then(parse_day) {
        let cap = (day + chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        if target > cap {
            target = cap;
        }
    }
    if current.as_deref() != Some(target.as_str()) {
        meta_set(conn, "read_horizon_day", &target)?;
    }
    Ok(target)
}

/// One row of the intent ledger.
#[derive(Debug, Clone, Serialize)]
pub struct MarkRow {
    pub feed_url: String,
    pub guid: String,
    /// The entry's ordering day, used for the read window.
    pub day: String,
    pub read: Option<bool>,
    pub read_ctr: i64,
    pub star: Option<bool>,
    pub star_ctr: i64,
}

/// Entry ordering day: the same expression the list pages by, truncated to a
/// calendar day so the read window can be applied to it.
const DAY_EXPR: &str = "substr(COALESCE(e.published_at, e.created_at), 1, 10)";

/// Record one entry's read state, or change nothing when the incoming value is
/// not newer than the stored one. Returns whether anything changed, which is
/// what makes re-importing the same document report no changes.
pub fn write_mark_read(
    conn: &Connection,
    feed_url: &str,
    guid: &str,
    day: &str,
    read: Option<bool>,
    ctr: i64,
) -> Result<bool, rusqlite::Error> {
    let changed = conn.execute(
        "INSERT INTO sync_marks (feed_url, guid, day, read, read_ctr)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(feed_url, guid) DO UPDATE SET
             day = excluded.day,
             read = excluded.read,
             read_ctr = excluded.read_ctr
         WHERE excluded.read_ctr > sync_marks.read_ctr
            OR (excluded.read_ctr = sync_marks.read_ctr
                AND sync_marks.read IS NOT excluded.read)",
        params![feed_url, guid, day, read.map(|b| b as i64), ctr],
    )?;
    Ok(changed > 0)
}

/// Star counterpart of `write_mark_read`.
pub fn write_mark_star(
    conn: &Connection,
    feed_url: &str,
    guid: &str,
    day: &str,
    star: Option<bool>,
    ctr: i64,
) -> Result<bool, rusqlite::Error> {
    let changed = conn.execute(
        "INSERT INTO sync_marks (feed_url, guid, day, star, star_ctr)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(feed_url, guid) DO UPDATE SET
             day = excluded.day,
             star = excluded.star,
             star_ctr = excluded.star_ctr
         WHERE excluded.star_ctr > sync_marks.star_ctr
            OR (excluded.star_ctr = sync_marks.star_ctr
                AND sync_marks.star IS NOT excluded.star)",
        params![feed_url, guid, day, star.map(|b| b as i64), ctr],
    )?;
    Ok(changed > 0)
}

pub fn marks_all(conn: &Connection) -> Result<Vec<MarkRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT feed_url, guid, day, read, read_ctr, star, star_ctr
         FROM sync_marks ORDER BY day, feed_url, guid",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(MarkRow {
            feed_url: r.get(0)?,
            guid: r.get(1)?,
            day: r.get(2)?,
            read: r.get::<_, Option<i64>>(3)?.map(|v| v != 0),
            read_ctr: r.get(4)?,
            star: r.get::<_, Option<i64>>(5)?.map(|v| v != 0),
            star_ctr: r.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn prune_marks(conn: &Connection, horizon: &str) -> Result<usize, rusqlite::Error> {
    // A row that ever carried a star is kept forever: stars never expire, and
    // dropping the row would let another installation's older star record
    // reinstate a star the reader removed.
    conn.execute(
        "DELETE FROM sync_marks WHERE day < ?1 AND star IS NULL",
        params![horizon],
    )
}

/// Apply stored intent to the entries that exist locally. Rows whose state
/// would not change are left alone, so this is cheap enough to run after every
/// fetch as well as after an import.
pub fn apply_marks_to_entries(
    conn: &Connection,
    feed_id: Option<i64>,
) -> Result<usize, rusqlite::Error> {
    let scope = if feed_id.is_some() { " AND f.id = ?1" } else { "" };
    let sql = format!(
        "UPDATE entries AS e
         SET is_read = COALESCE(m.read, e.is_read),
             is_starred = COALESCE(m.star, e.is_starred)
         FROM sync_marks m JOIN feeds f ON f.url = m.feed_url
         WHERE e.feed_id = f.id AND e.guid = m.guid
           AND (COALESCE(m.read, e.is_read) <> e.is_read
                OR COALESCE(m.star, e.is_starred) <> e.is_starred){scope}"
    );
    match feed_id {
        Some(id) => conn.execute(&sql, params![id]),
        None => conn.execute(&sql, []),
    }
}

/// The copyable identity and ordering day of one entry.
fn entry_mark_key(
    conn: &Connection,
    entry_id: i64,
) -> Result<Option<(String, String, String)>, rusqlite::Error> {
    conn.query_row(
        "SELECT f.url, e.guid, substr(COALESCE(e.published_at, e.created_at), 1, 10)
         FROM entries e JOIN feeds f ON f.id = e.feed_id WHERE e.id = ?1",
        params![entry_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
}

/// Record a single entry's read state. Entries already outside the window are
/// left out at write time rather than pruned later, so the ledger never fills
/// with rows that are discarded within hours.
fn record_read_mark(conn: &Connection, entry_id: i64, is_read: bool) -> Result<(), String> {
    let horizon = advance_read_horizon(conn, None).map_err(|e| e.to_string())?;
    let Some((feed_url, guid, day)) = entry_mark_key(conn, entry_id).map_err(|e| e.to_string())?
    else {
        return Ok(());
    };
    if day < horizon {
        return Ok(());
    }
    let ctr = next_counter(conn).map_err(|e| e.to_string())?;
    write_mark_read(conn, &feed_url, &guid, &day, Some(is_read), ctr)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn record_star_mark(conn: &Connection, entry_id: i64, is_starred: bool) -> Result<(), String> {
    let Some((feed_url, guid, day)) = entry_mark_key(conn, entry_id).map_err(|e| e.to_string())?
    else {
        return Ok(());
    };
    let ctr = next_counter(conn).map_err(|e| e.to_string())?;
    write_mark_star(conn, &feed_url, &guid, &day, Some(is_starred), ctr)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Bulk counterpart of `record_read_mark`, for the batch actions. One statement
/// per batch instead of one per row: marking a scope read can touch thousands of
/// entries. `ctr` and `horizon` are formatted rather than bound because a bare
/// `?` that appears before this statement's WHERE clause would renumber the
/// scope conditions' placeholders; both values are integers and a parsed date.
fn record_read_marks_for_where(
    conn: &Connection,
    conditions: &str,
    params: &[Param],
    ctr: i64,
    horizon: &str,
) -> Result<usize, String> {
    let sql = format!(
        "INSERT INTO sync_marks (feed_url, guid, day, read, read_ctr)
         SELECT f.url, e.guid, {day}, 1, {ctr}
         FROM entries e JOIN feeds f ON f.id = e.feed_id
         WHERE e.is_read = 1 AND {day} >= '{horizon}'{conditions}
         ON CONFLICT(feed_url, guid) DO UPDATE SET
             read = 1, read_ctr = excluded.read_ctr
         WHERE excluded.read_ctr > sync_marks.read_ctr",
        day = DAY_EXPR,
        ctr = ctr,
        horizon = horizon,
        conditions = if conditions.is_empty() {
            String::new()
        } else {
            format!(" AND {conditions}")
        },
    );
    let refs = bind_refs(params);
    conn.execute(&sql, refs.as_slice())
        .map_err(|e| e.to_string())
}

fn record_read_marks_for_ids(conn: &Connection, ids: &[i64]) -> Result<usize, String> {
    if ids.is_empty() {
        return Ok(0);
    }
    let horizon = advance_read_horizon(conn, None).map_err(|e| e.to_string())?;
    let ctr = next_counter(conn).map_err(|e| e.to_string())?;
    let placeholders = vec!["?"; ids.len()].join(",");
    let conditions = format!("e.id IN ({placeholders})");
    let params: Vec<Param> = ids.iter().map(|id| Box::new(*id) as Param).collect();
    record_read_marks_for_where(conn, &conditions, &params, ctr, &horizon)
}

/// One-time seed of the ledger from the entries that predate it. Without it the
/// ledger and the entry columns would disagree from the first import onward, and
/// the reader's existing stars would look like they were dropped. Seeded rows
/// carry counter 0 — the weakest possible — so any real edit or import outranks
/// them.
fn seed_marks_once(conn: &Connection) -> Result<(), rusqlite::Error> {
    if meta_get(conn, "marks_seeded")?.is_some() {
        return Ok(());
    }
    let horizon = days_ago(READ_WINDOW_DAYS);
    conn.execute(
        &format!(
            "INSERT OR IGNORE INTO sync_marks
                 (feed_url, guid, day, read, read_ctr, star, star_ctr)
             SELECT f.url, e.guid, {day}, e.is_read, 0, e.is_starred, 0
             FROM entries e JOIN feeds f ON f.id = e.feed_id
             WHERE (e.is_read = 1 AND {day} >= '{horizon}') OR e.is_starred = 1",
            day = DAY_EXPR,
            horizon = horizon,
        ),
        [],
    )?;
    // Read marks older than the window are skipped, but every starred entry is
    // seeded regardless of age, because stars never expire.
    meta_set(conn, "marks_seeded", &horizon)?;
    Ok(())
}

/// One remembered removal.
#[derive(Debug, Clone, Serialize)]
pub struct TombstoneRow {
    pub kind: String,
    pub key: String,
    pub ctr: i64,
    pub day: String,
}

/// Record (or strengthen) a removal. Imported documents carry their own day, so
/// the fields are parameters rather than the local clock.
pub fn raise_tombstone(
    conn: &Connection,
    kind: &str,
    key: &str,
    ctr: i64,
    day: &str,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO sync_tombstones (kind, key, ctr, day) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(kind, key) DO UPDATE SET
             ctr = MAX(sync_tombstones.ctr, excluded.ctr),
             day = MAX(sync_tombstones.day, excluded.day)",
        params![kind, key, ctr, day],
    )?;
    Ok(())
}

fn record_tombstone(conn: &Connection, kind: &str, key: &str) -> Result<i64, rusqlite::Error> {
    let ctr = next_counter(conn)?;
    raise_tombstone(conn, kind, key, ctr, &today())?;
    Ok(ctr)
}

pub fn tombstones_all(conn: &Connection) -> Result<Vec<TombstoneRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT kind, key, ctr, day FROM sync_tombstones ORDER BY kind, key",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(TombstoneRow {
            kind: r.get(0)?,
            key: r.get(1)?,
            ctr: r.get(2)?,
            day: r.get(3)?,
        })
    })?;
    rows.collect()
}

/// Drop removals old enough that every installation has had a chance to see
/// them, so the document does not grow without bound.
pub fn prune_tombstones(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.execute(
        "DELETE FROM sync_tombstones WHERE day < ?1",
        params![days_ago(TOMBSTONE_RETENTION_DAYS)],
    )
}

/// The counter of the removal recorded for a feed URL or folder identity, or 0
/// when there is none.
pub fn tombstone_ctr(
    conn: &Connection,
    kind: &str,
    key: &str,
) -> Result<i64, rusqlite::Error> {
    Ok(conn
        .query_row(
            "SELECT ctr FROM sync_tombstones WHERE kind = ?1 AND key = ?2",
            params![kind, key],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
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
    // Only a genuinely new row takes a counter: the conflict path must not
    // inflate the sequence on every refresh.
    let sync_ctr = match feed_id_by_url(conn, url)? {
        Some(_) => 0,
        None => next_counter(conn)?,
    };
    conn.execute(
        "INSERT INTO feeds (url, title, site_url, sync_ctr) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(url) DO UPDATE SET title = ?2, site_url = COALESCE(?3, site_url)",
        params![url, title, site_url, sync_ctr],
    )?;
    conn.query_row("SELECT id FROM feeds WHERE url = ?1", params![url], |r| {
        r.get(0)
    })
}

/// Insert an entry, or update the existing one for the same (feed, guid).
/// `guid` must be non-empty; callers pass the link when the feed omits a GUID.
/// The arrival time (`created_at`) is set once on insert and never updated:
/// keyset paging treats it as immutable, so re-fetching a feed must not move
/// an entry's position.
pub fn upsert_entry(
    conn: &Connection,
    feed_id: i64,
    guid: &str,
    title: &str,
    link: Option<&str>,
    content_html: &str,
    excerpt: &str,
    image_url: Option<&str>,
    published_at: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO entries
           (feed_id, guid, title, link, content_html, excerpt, image_url, published_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
         ON CONFLICT(feed_id, guid) DO UPDATE SET
           title = excluded.title,
           link = excluded.link,
           content_html = excluded.content_html,
           excerpt = excluded.excerpt,
           image_url = excluded.image_url,
           published_at = COALESCE(excluded.published_at, entries.published_at)",
        params![
            feed_id,
            guid,
            title,
            link,
            content_html,
            excerpt,
            image_url,
            published_at
        ],
    )?;
    Ok(())
}

/// Derive `excerpt` and `image_url` for entries that predate those columns,
/// from the content already stored, so the list gains row detail without
/// re-fetching every feed.
///
/// `excerpt IS NULL` is the work queue: every insert writes an excerpt (possibly
/// empty), so a NULL unambiguously means "not backfilled yet". Work happens in
/// bounded batches inside one transaction, so an interrupted run resumes where
/// it stopped instead of restarting. Only these two columns are written.
pub fn backfill_row_details(conn: &Connection) -> Result<usize, rusqlite::Error> {
    const BATCH: i64 = 200;
    let mut processed = 0usize;
    let mut cursor = i64::MIN;
    loop {
        let rows: Vec<(i64, String)> = {
            let mut stmt = conn.prepare(
                "SELECT id, content_html FROM entries
                 WHERE excerpt IS NULL AND id > ?1
                 ORDER BY id LIMIT ?2",
            )?;
            let mapped = stmt.query_map(params![cursor, BATCH], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        if rows.is_empty() {
            break;
        }
        let tx = conn.unchecked_transaction()?;
        for (id, content_html) in &rows {
            let excerpt = crate::rowdetail::excerpt_from_html(content_html);
            // No media metadata survives for old rows, so the body's first image
            // is the only cover source; a later refresh upgrades it.
            let image_url = crate::rowdetail::cover_from_html(content_html, &[]);
            tx.execute(
                "UPDATE entries SET excerpt = ?1, image_url = ?2 WHERE id = ?3",
                params![excerpt, image_url, id],
            )?;
        }
        tx.commit()?;
        let batch_len = rows.len();
        processed += batch_len;
        cursor = rows[batch_len - 1].0;
        if batch_len < BATCH as usize {
            break;
        }
    }
    Ok(processed)
}

/// All feeds with unread entry counts, newest-fetch order not assumed (UI sorts if needed).
/// `title` is the display title: a user-set custom title wins over the feed-provided one.
pub fn list_feeds(conn: &Connection) -> Result<Vec<FeedDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.url, COALESCE(f.custom_title, f.title), f.site_url, f.last_fetched_at, f.last_error, f.folder_id,
                (SELECT COUNT(*) FROM entries e WHERE e.feed_id = f.id AND e.is_read = 0)
         FROM feeds f ORDER BY COALESCE(f.custom_title, f.title) COLLATE NOCASE",
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

/// Ordering position of an entry: publication time when one is known, arrival
/// time otherwise. The expression must match `idx_entries_sort` verbatim or
/// SQLite cannot use that index for the global ordering.
const SORT_KEY: &str = "COALESCE(published_at, created_at)";

/// Page cursor: the ordering position and id of the last row of the previous
/// page. Keyset rather than an offset, so refreshing feeds cannot shift the
/// page the reader is currently on.
#[derive(Debug, Clone, Deserialize)]
pub struct EntryCursor {
    /// `SORT_KEY` for the boundary row — never NULL, which is the whole point
    /// of coalescing the publication time with the arrival time.
    pub sort: String,
    pub id: i64,
}

/// One row of a paginated entry list. Carries no `content_html`: the list
/// renders a short excerpt and a cover, and bodies are fetched on demand for
/// the entry being read. `sort_key` is echoed back by the frontend as the
/// cursor for the following page.
#[derive(Debug, Clone, Serialize)]
pub struct EntryListItemDto {
    pub id: i64,
    pub feed_id: i64,
    pub title: String,
    pub link: Option<String>,
    /// Plain-text snippet; empty when the entry has no text to preview.
    pub excerpt: String,
    /// Absolute cover image URL, when one was derived at ingest.
    pub image_url: Option<String>,
    pub published_at: Option<String>,
    pub sort_key: String,
    pub is_read: bool,
    pub is_starred: bool,
}

type Param = Box<dyn rusqlite::ToSql>;

/// WHERE conditions (without the keyword) and their bound parameters for a
/// view/scope pair. Shared by listing and counting so the two can never
/// disagree about which entries belong to a view.
fn filter_conditions(view: &str, scope: EntryScope) -> Result<(Vec<String>, Vec<Param>), String> {
    if view != "all" && view != "unread" {
        return Err(format!("unknown view: {view}"));
    }
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<Param> = Vec::new();
    match scope {
        EntryScope::All => {}
        EntryScope::Feed { id } => {
            conditions.push("feed_id = ?".to_string());
            params.push(Box::new(id));
        }
        EntryScope::Folder { id } => {
            conditions.push("feed_id IN (SELECT id FROM feeds WHERE folder_id = ?)".to_string());
            params.push(Box::new(id));
        }
        EntryScope::Starred => conditions.push("is_starred = 1".to_string()),
    }
    if view == "unread" {
        conditions.push("is_read = 0".to_string());
    }
    Ok((conditions, params))
}

fn where_clause(conditions: &[String]) -> String {
    if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    }
}

fn bind_refs(params: &[Param]) -> Vec<&dyn rusqlite::ToSql> {
    params.iter().map(|p| p.as_ref()).collect()
}

/// One page of entries for a scope combined with a view (`"all"` or
/// `"unread"`), newest first: at most `limit` rows strictly older than
/// `after`, where `after: None` starts at the newest page.
///
/// The query actually asks for `limit + 1` rows — receiving more than `limit`
/// is how callers learn that another page exists, without a second query.
/// Callers MUST slice to `limit`.
pub fn list_entries(
    conn: &Connection,
    view: &str,
    scope: EntryScope,
    after: Option<&EntryCursor>,
    limit: i64,
) -> Result<Vec<EntryListItemDto>, String> {
    let (mut conditions, mut params) = filter_conditions(view, scope)?;
    if let Some(cursor) = after {
        conditions.push(format!("({key} < ? OR ({key} = ? AND id < ?))", key = SORT_KEY));
        params.push(Box::new(cursor.sort.clone()));
        params.push(Box::new(cursor.sort.clone()));
        params.push(Box::new(cursor.id));
    }
    params.push(Box::new(limit.max(0) + 1));
    let sql = format!(
        "SELECT id, feed_id, title, link, published_at, {key} AS sort_key, is_read, is_starred,
                COALESCE(excerpt, ''), image_url
         FROM entries {where}
         ORDER BY {key} DESC, id DESC
         LIMIT ?",
        key = SORT_KEY,
        where = where_clause(&conditions),
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let refs = bind_refs(&params);
    stmt.query_map(refs.as_slice(), map_list_item)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn map_list_item(r: &rusqlite::Row<'_>) -> rusqlite::Result<EntryListItemDto> {
    let is_read: i64 = r.get(6)?;
    let is_starred: i64 = r.get(7)?;
    Ok(EntryListItemDto {
        id: r.get(0)?,
        feed_id: r.get(1)?,
        title: r.get(2)?,
        link: r.get(3)?,
        published_at: r.get(4)?,
        sort_key: r.get(5)?,
        is_read: is_read != 0,
        is_starred: is_starred != 0,
        excerpt: r.get(8)?,
        image_url: r.get(9)?,
    })
}

/// How many entries match a view and scope. Separate from `list_entries`
/// because the sidebar's global badge must not be reused for a filtered view.
pub fn count_entries(conn: &Connection, view: &str, scope: EntryScope) -> Result<i64, String> {
    let (conditions, params) = filter_conditions(view, scope)?;
    let sql = format!("SELECT COUNT(*) FROM entries {}", where_clause(&conditions));
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let refs = bind_refs(&params);
    stmt.query_row(refs.as_slice(), |r| r.get(0))
        .map_err(|e| e.to_string())
}

/// One entry with its content, for the reading pane. `None` when the entry
/// no longer exists (its feed was removed).
pub fn get_entry(
    conn: &Connection,
    entry_id: i64,
) -> Result<Option<EntryDto>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, feed_id, title, link, content_html, published_at, is_read, is_starred
         FROM entries WHERE id = ?1",
        params![entry_id],
        map_entry,
    )
    .optional()
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

/// Test helper: every row matching a view and scope, ignoring paging. Keeps
/// existing assertions readable while the real API is page-bounded.
#[cfg(test)]
pub fn list_all_entries(
    conn: &Connection,
    view: &str,
    scope: EntryScope,
) -> Result<Vec<EntryListItemDto>, String> {
    let total = count_entries(conn, view, scope)?;
    let rows = list_entries(conn, view, scope, None, total)?;
    Ok(rows.into_iter().take(total as usize).collect())
}

pub fn set_read(
    conn: &Connection,
    entry_id: i64,
    is_read: bool,
) -> Result<(), String> {
    // The entry column and the ledger row move together: the column is what the
    // list reads, the ledger is what another installation receives.
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let n = tx
        .execute(
            "UPDATE entries SET is_read = ?1 WHERE id = ?2",
            params![is_read as i64, entry_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("entry {entry_id} not found"));
    }
    record_read_mark(&tx, entry_id, is_read)?;
    tx.commit().map_err(|e| e.to_string())
}

/// Ids per statement when marking a batch read. Deep scrolling can leave
/// thousands of rows loaded, so the id list is chunked rather than trusting
/// SQLite's bound-parameter limit to stay out of reach.
const READ_BATCH_CHUNK: usize = 500;

/// Mark the given entries read. Ids that are already read, or that no longer
/// exist, are not counted: the return value is the number of rows that actually
/// changed. The whole batch is one transaction, so a failure cannot leave it
/// half applied.
pub fn mark_read_many(conn: &Connection, ids: &[i64]) -> Result<i64, String> {
    if ids.is_empty() {
        return Ok(0);
    }
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut changed = 0i64;
    for chunk in ids.chunks(READ_BATCH_CHUNK) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!(
            "UPDATE entries SET is_read = 1 WHERE is_read = 0 AND id IN ({placeholders})"
        );
        let params: Vec<&dyn rusqlite::ToSql> =
            chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        changed += tx.execute(&sql, params.as_slice()).map_err(|e| e.to_string())? as i64;
    }
    record_read_marks_for_ids(&tx, ids)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(changed)
}

/// Mark every unread entry matching a scope read, whether or not it was ever
/// listed. Shares `filter_conditions` with listing and counting, so the rows
/// touched are exactly the ones the unread view would have shown.
pub fn mark_scope_read(conn: &Connection, scope: EntryScope) -> Result<i64, String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let (conditions, params) = filter_conditions("unread", scope)?;
    let sql = format!(
        "UPDATE entries SET is_read = 1 {}",
        where_clause(&conditions)
    );
    let refs = bind_refs(&params);
    let changed = tx
        .execute(&sql, refs.as_slice())
        .map_err(|e| e.to_string())?;
    // The whole scope is shared, not just the rows that just flipped: entries
    // read earlier are already in the ledger and the guarded insert leaves them
    // alone, while the ones that just changed are now due.
    let (rec_conditions, rec_params) = filter_conditions("all", scope)?;
    let horizon = advance_read_horizon(&tx, None).map_err(|e| e.to_string())?;
    let ctr = next_counter(&tx).map_err(|e| e.to_string())?;
    record_read_marks_for_where(
        &tx,
        &rec_conditions.join(" AND "),
        &rec_params,
        ctr,
        &horizon,
    )?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(changed as i64)
}

/// Star or unstar an entry. Independent of read state.
pub fn set_starred(
    conn: &Connection,
    entry_id: i64,
    is_starred: bool,
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let n = tx
        .execute(
            "UPDATE entries SET is_starred = ?1 WHERE id = ?2",
            params![is_starred as i64, entry_id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("entry {entry_id} not found"));
    }
    record_star_mark(&tx, entry_id, is_starred)?;
    tx.commit().map_err(|e| e.to_string())
}

/// Aggregate counts for the global sidebar rows.
#[derive(Debug, Clone, Serialize)]
pub struct EntryCountsDto {
    /// Total stored entries, read and unread alike.
    pub all_entries: i64,
    /// Total starred entries, read and unread alike.
    pub starred_entries: i64,
    /// Unread entries across the whole library, for the global batch action's
    /// label. Returned by the same statement as the other two so the figure
    /// can never disagree with the badges it sits beside.
    pub unread_entries: i64,
}

/// Total, starred, and unread entry counts in one statement, for the global
/// sidebar badges and the library-wide batch action.
pub fn entry_counts(conn: &Connection) -> Result<EntryCountsDto, rusqlite::Error> {
    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(is_starred), 0), COALESCE(SUM(is_read = 0), 0)
         FROM entries",
        [],
        |r| {
            Ok(EntryCountsDto {
                all_entries: r.get(0)?,
                starred_entries: r.get(1)?,
                unread_entries: r.get(2)?,
            })
        },
    )
}

/// Delete a feed; entries cascade (verified by schema test). The removal is
/// recorded as a tombstone first, so it propagates instead of being undone by an
/// installation that has not seen it yet. Keeping the delete itself hard leaves
/// every existing query untouched.
pub fn delete_feed(conn: &Connection, feed_id: i64) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let url: Option<String> = tx
        .query_row("SELECT url FROM feeds WHERE id = ?1", params![feed_id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM feeds WHERE id = ?1", params![feed_id])
        .map_err(|e| e.to_string())?;
    if let Some(url) = url {
        record_tombstone(&tx, TOMBSTONE_FEED, &url).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
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
        "SELECT f.id, f.url, COALESCE(f.custom_title, f.title), f.site_url, f.last_fetched_at, f.last_error, f.folder_id,
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

/// `(id, url)` for every feed assigned to a folder. Errors when the folder
/// does not exist.
pub fn feed_ids_and_urls_in_folder(
    conn: &Connection,
    folder_id: i64,
) -> Result<Vec<(i64, String)>, String> {
    let exists: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders WHERE id = ?1", params![folder_id], |r| {
            r.get(0)
        })
        .map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err(format!("folder {folder_id} not found"));
    }
    let mut stmt = conn
        .prepare("SELECT id, url FROM feeds WHERE folder_id = ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![folder_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

/// Set (Some) or clear (None) a feed's custom display title. Empty or
/// whitespace-only text clears it. The feed-provided `title` is never touched;
/// subsequent fetches keep updating it while the display prefers the custom one.
pub fn set_feed_title(conn: &Connection, feed_id: i64, title: Option<&str>) -> Result<(), String> {
    let custom = title.map(str::trim).filter(|t| !t.is_empty());
    let ctr = next_counter(conn).map_err(|e| e.to_string())?;
    let n = conn
        .execute(
            "UPDATE feeds SET custom_title = ?1, custom_title_ctr = ?3 WHERE id = ?2",
            params![custom, feed_id, ctr],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("feed {feed_id} not found"));
    }
    Ok(())
}

/// Create a folder. The name MUST be non-empty.
pub fn create_folder(conn: &Connection, name: &str) -> Result<FolderDto, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("folder name is empty".into());
    }
    let ctr = next_counter(conn).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO folders (name, uuid, sync_ctr)
         VALUES (?1, 'f-' || lower(hex(randomblob(8))), ?2)",
        params![name, ctr],
    )
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
    let ctr = next_counter(conn).map_err(|e| e.to_string())?;
    let n = conn
        .execute(
            "UPDATE folders SET name = ?1, name_ctr = ?3 WHERE id = ?2",
            params![name, folder_id, ctr],
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
    let uuid: Option<String> = tx
        .query_row("SELECT uuid FROM folders WHERE id = ?1", params![folder_id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
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
    if let Some(uuid) = uuid {
        record_tombstone(&tx, TOMBSTONE_FOLDER, &uuid).map_err(|e| e.to_string())?;
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
    let ctr = next_counter(conn).map_err(|e| e.to_string())?;
    let n = conn
        .execute(
            "UPDATE feeds SET folder_id = ?1, folder_ctr = ?3 WHERE id = ?2",
            params![folder_id, feed_id, ctr],
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

    /// Insert without going through the row-detail arguments the legacy tests
    /// do not care about. `excerpt` is written as empty rather than NULL, so a
    /// test-inserted row is never mistaken for one awaiting backfill.
    #[allow(clippy::too_many_arguments)]
    fn insert_entry(
        conn: &Connection,
        feed_id: i64,
        guid: &str,
        title: &str,
        link: Option<&str>,
        content_html: &str,
        published_at: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        upsert_entry(
            conn,
            feed_id,
            guid,
            title,
            link,
            content_html,
            "",
            None,
            published_at,
        )
    }

    /// A feed row with a URL/title derived from `slug`, for tests that only
    /// need a valid foreign key.
    fn insert_feed(conn: &Connection, slug: &str) -> i64 {
        insert_or_update_feed(conn, &format!("https://{slug}.example/feed"), slug, None).unwrap()
    }

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
        assert!(entry_cols.contains(&"created_at".to_string()));
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

        let created_at: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('entries') WHERE name='created_at'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(created_at, 1);

        for column in ["excerpt", "image_url"] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('entries') WHERE name = ?1",
                    params![column],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "entries must gain the {column} column");
        }
    }

    #[test]
    fn upsert_updates_row_detail_in_place() {
        let conn = open_memory().unwrap();
        let f = insert_feed(&conn, "row-detail");
        upsert_entry(
            &conn,
            f,
            "g1",
            "v1",
            None,
            "<p>one</p>",
            "one",
            None,
            Some("2026-01-01T00:00:00Z"),
        )
        .unwrap();
        // A later fetch supplies an image where there was none.
        upsert_entry(
            &conn,
            f,
            "g1",
            "v2",
            None,
            "<p>two</p>",
            "two",
            Some("https://img.example/cover.jpg"),
            Some("2026-01-02T00:00:00Z"),
        )
        .unwrap();

        let rows = list_all_entries(&conn, "all", EntryScope::All).unwrap();
        assert_eq!(rows.len(), 1, "re-upserting the same GUID must not duplicate");
        assert_eq!(rows[0].excerpt, "two");
        assert_eq!(
            rows[0].image_url.as_deref(),
            Some("https://img.example/cover.jpg")
        );
    }

    #[test]
    fn list_returns_row_detail_and_no_entry_body() {
        let conn = open_memory().unwrap();
        let f = insert_feed(&conn, "row-detail-list");
        upsert_entry(
            &conn,
            f,
            "g1",
            "Title",
            None,
            "<p>secret body</p>",
            "secret body",
            Some("https://img.example/c.jpg"),
            Some("2026-01-01T00:00:00Z"),
        )
        .unwrap();

        let rows = list_all_entries(&conn, "all", EntryScope::All).unwrap();
        assert_eq!(rows[0].excerpt, "secret body");
        assert_eq!(rows[0].image_url.as_deref(), Some("https://img.example/c.jpg"));

        // The response shape must still keep bodies out of the list payload.
        let json = serde_json::to_string(&rows).unwrap();
        assert!(!json.contains("content_html"), "list rows carry no body field");
        assert!(!json.contains("<p>"), "list rows carry no stored HTML");
    }

    #[test]
    fn backfill_fills_row_detail_without_touching_reading_state() {
        let conn = open_memory().unwrap();
        let f = insert_feed(&conn, "backfill");
        insert_entry(
            &conn,
            f,
            "g1",
            "Old",
            None,
            "<p>Hello &amp; welcome</p><img src=\"https://img.example/a.jpg\">",
            Some("2026-01-01T00:00:00Z"),
        )
        .unwrap();
        insert_entry(
            &conn,
            f,
            "g2",
            "Bare",
            None,
            "<p>No picture here</p>",
            None,
        )
        .unwrap();
        // Pretend both rows were stored before the columns existed.
        conn.execute("UPDATE entries SET excerpt = NULL, image_url = NULL", [])
            .unwrap();
        set_read(&conn, 1, true).unwrap();
        set_starred(&conn, 1, true).unwrap();

        let processed = backfill_row_details(&conn).unwrap();
        assert_eq!(processed, 2);

        let rows = list_all_entries(&conn, "all", EntryScope::All).unwrap();
        let old = rows.iter().find(|r| r.id == 1).unwrap();
        assert_eq!(old.excerpt, "Hello & welcome");
        assert_eq!(old.image_url.as_deref(), Some("https://img.example/a.jpg"));
        assert!(old.is_read, "read state must survive the backfill");
        assert!(old.is_starred, "starred state must survive the backfill");
        assert_eq!(old.published_at.as_deref(), Some("2026-01-01T00:00:00Z"));
        let bare = rows.iter().find(|r| r.id == 2).unwrap();
        assert_eq!(bare.excerpt, "No picture here");
        assert_eq!(bare.image_url, None);

        // Second run finds nothing left to do.
        assert_eq!(backfill_row_details(&conn).unwrap(), 0);
    }

    #[test]
    fn init_schema_backfills_created_at_for_entries_stored_before_it() {
        // Simulate a pre-upgrade database holding entries, with no created_at.
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch(FOLDERS_TABLE).unwrap();
        conn.execute(
            "INSERT INTO feeds (url, title) VALUES ('https://a.example/feed', 'A')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO entries (feed_id, guid, title, content_html) VALUES (1, 'g1', 'T', '<p>x</p>')",
            [],
        ).unwrap();

        // Two upgrade passes: the column is added, backfilled, and both pass.
        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap();

        let created: String = conn
            .query_row("SELECT created_at FROM entries", [], |r| r.get(0))
            .unwrap();
        assert!(!created.is_empty(), "legacy entry must gain an arrival time");
    }

    #[test]
    fn init_schema_creates_the_sort_index_exactly_once() {
        let conn = open_memory().unwrap();
        init_schema(&conn).unwrap();
        init_schema(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_entries_sort'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "repeated initialization must not duplicate the index");

        // The index expression must match the query expression verbatim,
        // otherwise SQLite cannot use it for the global ordering.
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='index' AND name='idx_entries_sort'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            sql.contains("COALESCE(published_at, created_at)"),
            "index must key on the ordering expression, got: {sql}"
        );
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

        insert_entry(&conn, a, "g1", "T", None, "<p>x</p>", None).unwrap();
        insert_entry(&conn, a, "g2", "T", None, "<p>x</p>", None).unwrap();
        insert_entry(&conn, b, "g3", "T", None, "<p>x</p>", None).unwrap();
        insert_entry(&conn, c, "g4", "T", None, "<p>x</p>", None).unwrap();

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

        insert_entry(&conn, feed_id, "g1", "Title v1", Some("https://a/1"), "<p>one</p>", Some("2026-01-01T00:00:00Z")).unwrap();
        insert_entry(&conn, feed_id, "g1", "Title v2", Some("https://a/1"), "<p>two</p>", Some("2026-01-02T00:00:00Z")).unwrap();

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
    fn refetching_an_entry_preserves_its_arrival_time() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();

        insert_entry(&conn, f, "g1", "v1", None, "<p>one</p>", Some("2026-01-01T00:00:00Z")).unwrap();
        let initial: String = conn
            .query_row("SELECT created_at FROM entries", [], |r| r.get(0))
            .unwrap();
        assert!(!initial.is_empty(), "a new entry records its arrival time");

        // Pin it so the assertion does not depend on clock resolution.
        conn.execute("UPDATE entries SET created_at = '2020-01-01 00:00:00'", [])
            .unwrap();
        insert_entry(&conn, f, "g1", "v2", None, "<p>two</p>", Some("2026-01-02T00:00:00Z")).unwrap();

        let (created, title): (String, String) = conn
            .query_row("SELECT created_at, title FROM entries", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(created, "2020-01-01 00:00:00", "re-fetch must not move an entry's position");
        assert_eq!(title, "v2", "the entry still updates in place");
    }

    #[test]
    fn same_guid_in_different_feeds_is_allowed() {
        let conn = open_memory().unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        insert_entry(&conn, a, "g1", "T", None, "<p>x</p>", None).unwrap();
        insert_entry(&conn, b, "g1", "T", None, "<p>x</p>", None).unwrap();
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
        insert_entry(&conn, a, "g1", "T", None, "<p>x</p>", None).unwrap();
        insert_entry(&conn, b, "g2", "T", None, "<p>x</p>", None).unwrap();

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
        insert_entry(&conn, f, "g1", "Old", None, "<p>x</p>", Some("2026-01-01T00:00:00Z")).unwrap();
        insert_entry(&conn, f, "g2", "New", None, "<p>x</p>", Some("2026-02-01T00:00:00Z")).unwrap();
        insert_entry(&conn, f, "g3", "Mid", None, "<p>x</p>", Some("2026-01-15T00:00:00Z")).unwrap();

        let all = list_all_entries(&conn, "all", EntryScope::All).unwrap();
        let titles: Vec<&str> = all.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["New", "Mid", "Old"]);

        set_read(&conn, all[0].id, true).unwrap();
        let unread = list_all_entries(&conn, "unread", EntryScope::All).unwrap();
        assert_eq!(unread.len(), 2);
        assert!(unread.iter().all(|e| !e.is_read));

        let per_feed = list_all_entries(&conn, "all", EntryScope::Feed { id: f }).unwrap();
        assert_eq!(per_feed.len(), 3, "feed view includes read entries");

        assert!(list_all_entries(&conn, "bogus", EntryScope::All).is_err());
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
        insert_entry(&conn, a, "g1", "E1", None, "<p>x</p>", Some("2026-01-01T00:00:00Z")).unwrap();
        insert_entry(&conn, a, "g2", "E2", None, "<p>x</p>", Some("2026-01-02T00:00:00Z")).unwrap();
        insert_entry(&conn, b, "g3", "E4", None, "<p>x</p>", Some("2026-01-04T00:00:00Z")).unwrap();
        insert_entry(&conn, c, "g4", "E3", None, "<p>x</p>", Some("2026-01-03T00:00:00Z")).unwrap();

        // e1 (feed a): starred + unread; e2: read; e4 (feed b): unread; e3 (feed c): starred + read.
        set_starred(&conn, 1, true).unwrap();
        set_starred(&conn, 4, true).unwrap();
        set_read(&conn, 2, true).unwrap();
        set_read(&conn, 4, true).unwrap();

        let titles = |entries: Vec<EntryListItemDto>| -> Vec<String> {
            entries.into_iter().map(|e| e.title).collect()
        };

        assert_eq!(
            titles(list_all_entries(&conn, "all", EntryScope::All).unwrap()),
            vec!["E4", "E3", "E2", "E1"],
            "newest first"
        );
        assert_eq!(
            titles(list_all_entries(&conn, "all", EntryScope::Folder { id: tech.id }).unwrap()),
            vec!["E4", "E2", "E1"],
            "folder scope covers its feeds only"
        );
        assert_eq!(
            titles(list_all_entries(&conn, "all", EntryScope::Feed { id: c }).unwrap()),
            vec!["E3"],
            "feed scope unchanged"
        );
        assert_eq!(
            titles(list_all_entries(&conn, "all", EntryScope::Starred).unwrap()),
            vec!["E3", "E1"],
            "starred scope is global across feeds and folders"
        );
        assert_eq!(
            titles(list_all_entries(&conn, "unread", EntryScope::Starred).unwrap()),
            vec!["E1"],
            "view combines with starred scope"
        );
        assert_eq!(
            titles(list_all_entries(&conn, "unread", EntryScope::Folder { id: tech.id }).unwrap()),
            vec!["E4", "E1"]
        );
    }

    /// Mirrors the frontend: ask the database for `limit` rows, slice off the
    /// probe row, and report whether another page exists.
    fn page(
        conn: &Connection,
        view: &str,
        scope: EntryScope,
        after: Option<&EntryCursor>,
        limit: i64,
    ) -> (Vec<EntryListItemDto>, bool) {
        let rows = list_entries(conn, view, scope, after, limit).unwrap();
        let has_more = rows.len() as i64 > limit;
        (
            rows.into_iter().take(limit.max(0) as usize).collect(),
            has_more,
        )
    }

    fn cursor_of(row: &EntryListItemDto) -> EntryCursor {
        EntryCursor {
            sort: row.sort_key.clone(),
            id: row.id,
        }
    }

    #[test]
    fn undated_entries_order_by_arrival_and_stay_reachable() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        insert_entry(&conn, f, "g1", "Dated 2019", None, "<p>x</p>", Some("2019-05-01T00:00:00+00:00")).unwrap();
        insert_entry(&conn, f, "g2", "Undated", None, "<p>x</p>", None).unwrap();
        // Deterministic arrival time, newer than the dated entry's publication.
        conn.execute(
            "UPDATE entries SET created_at = '2026-06-01 12:00:00' WHERE guid = 'g2'",
            [],
        )
        .unwrap();

        let (page1, more) = page(&conn, "all", EntryScope::All, None, 1);
        assert_eq!(page1[0].title, "Undated", "arrival time sorts it above a 2019 publication");
        assert_eq!(
            page1[0].sort_key, "2026-06-01 12:00:00",
            "the cursor must carry a real value, never NULL"
        );
        assert!(more);

        // Page past it: the cursor comparison must return the older entry
        // instead of dropping the NULL-published row as `NULL < ?` would.
        let (page2, more2) = page(&conn, "all", EntryScope::All, Some(&cursor_of(&page1[0])), 1);
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].title, "Dated 2019");
        assert!(!more2, "nothing follows the last entry");
    }

    #[test]
    fn consecutive_pages_cover_every_entry_exactly_once() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        for i in 0..120 {
            let published = format!("2026-01-01T{:02}:{:02}:00+00:00", i / 60, i % 60);
            insert_entry(&conn, f, &format!("g{i}"), &format!("E{i}"), None, "<p>x</p>", Some(&published)).unwrap();
        }

        let full: Vec<String> = list_all_entries(&conn, "all", EntryScope::All)
            .unwrap()
            .into_iter()
            .map(|e| e.title)
            .collect();
        assert_eq!(full.len(), 120);

        let (page1, more1) = page(&conn, "all", EntryScope::All, None, 50);
        let (page2, more2) = page(&conn, "all", EntryScope::All, Some(&cursor_of(&page1[49])), 50);
        assert!(more1, "entries remain after the first page");
        assert!(more2, "20 entries remain after the second");

        let walked: Vec<String> = page1
            .iter()
            .chain(page2.iter())
            .map(|e| e.title.clone())
            .collect();
        let unique: std::collections::BTreeSet<String> = walked.iter().cloned().collect();
        assert_eq!(unique.len(), 100, "no entry appears on both pages");
        assert_eq!(walked, full[..100], "pages follow the global order with no gaps");
    }

    #[test]
    fn limit_plus_one_signals_whether_another_page_exists() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        for i in 0..50 {
            let published = format!("2026-01-01T{:02}:{:02}:00+00:00", i / 60, i % 60);
            insert_entry(&conn, f, &format!("g{i}"), &format!("E{i}"), None, "<p>x</p>", Some(&published)).unwrap();
        }

        // Exactly one full page: the probe row never arrives.
        let rows = list_entries(&conn, "all", EntryScope::All, None, 50).unwrap();
        assert_eq!(rows.len(), 50, "50 stored rows yield 50 returned rows");
        let (page_rows, has_more) = page(&conn, "all", EntryScope::All, None, 50);
        assert_eq!(page_rows.len(), 50);
        assert!(!has_more, "a full but final page reports no more");

        // One row too many: the probe row is what signals another page.
        insert_entry(&conn, f, "g50", "E50", None, "<p>x</p>", Some("2026-01-01T99:99:00+00:00")).unwrap();
        let rows = list_entries(&conn, "all", EntryScope::All, None, 50).unwrap();
        assert_eq!(rows.len(), 51, "the extra row exists only to signal more");
        let (page_rows, has_more) = page(&conn, "all", EntryScope::All, None, 50);
        assert_eq!(page_rows.len(), 50);
        assert!(has_more);
    }

    #[test]
    fn count_entries_matches_each_filtered_view_and_scope() {
        let conn = open_memory().unwrap();
        let tech = create_folder(&conn, "Tech").unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        let c = insert_or_update_feed(&conn, "https://c.example/feed", "C", None).unwrap();
        move_feed_to_folder(&conn, a, Some(tech.id)).unwrap();
        move_feed_to_folder(&conn, b, Some(tech.id)).unwrap();
        insert_entry(&conn, a, "g1", "E1", None, "<p>x</p>", Some("2026-01-01T00:00:00+00:00")).unwrap();
        insert_entry(&conn, a, "g2", "E2", None, "<p>x</p>", Some("2026-01-02T00:00:00+00:00")).unwrap();
        insert_entry(&conn, b, "g3", "E4", None, "<p>x</p>", Some("2026-01-04T00:00:00+00:00")).unwrap();
        insert_entry(&conn, c, "g4", "E3", None, "<p>x</p>", Some("2026-01-03T00:00:00+00:00")).unwrap();
        set_starred(&conn, 1, true).unwrap();
        set_starred(&conn, 4, true).unwrap();
        set_read(&conn, 2, true).unwrap();
        set_read(&conn, 4, true).unwrap();

        assert_eq!(count_entries(&conn, "all", EntryScope::All).unwrap(), 4);
        assert_eq!(count_entries(&conn, "unread", EntryScope::All).unwrap(), 2);
        assert_eq!(count_entries(&conn, "all", EntryScope::Feed { id: c }).unwrap(), 1);
        assert_eq!(count_entries(&conn, "all", EntryScope::Folder { id: tech.id }).unwrap(), 3);
        assert_eq!(count_entries(&conn, "all", EntryScope::Starred).unwrap(), 2);
        assert_eq!(count_entries(&conn, "unread", EntryScope::Starred).unwrap(), 1);
        assert!(count_entries(&conn, "bogus", EntryScope::All).is_err());

        // The total must agree with what paging can actually walk.
        for view in ["all", "unread"] {
            for scope in [
                EntryScope::All,
                EntryScope::Feed { id: c },
                EntryScope::Folder { id: tech.id },
                EntryScope::Starred,
            ] {
                assert_eq!(
                    count_entries(&conn, view, scope).unwrap() as usize,
                    list_all_entries(&conn, view, scope).unwrap().len(),
                    "count and listing must describe the same set"
                );
            }
        }
    }

    #[test]
    fn get_entry_returns_content_for_stored_rows_and_none_for_unknown_ids() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        insert_entry(&conn, f, "g1", "T", Some("https://a/1"), "<p>body</p>", Some("2026-01-01T00:00:00+00:00")).unwrap();

        let entry = get_entry(&conn, 1).unwrap().expect("stored entry is returned");
        assert_eq!(entry.title, "T");
        assert_eq!(entry.content_html, "<p>body</p>", "the reading pane needs the body");
        assert!(get_entry(&conn, 999).unwrap().is_none(), "unknown id yields None");

        // The list view of the same entry carries no body at all.
        let row = list_all_entries(&conn, "all", EntryScope::All).unwrap().remove(0);
        assert_eq!(row.published_at.as_deref(), Some("2026-01-01T00:00:00+00:00"));
        assert!(!row.sort_key.is_empty());
    }

    #[test]
    fn set_starred_is_independent_of_read_state_and_persists() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        insert_entry(&conn, f, "g1", "T", None, "<p>x</p>", None).unwrap();

        set_read(&conn, 1, true).unwrap();
        set_starred(&conn, 1, true).unwrap();
        let entry = &list_all_entries(&conn, "all", EntryScope::All).unwrap()[0];
        assert!(entry.is_starred, "star persisted");
        assert!(entry.is_read, "read state untouched by starring");

        set_starred(&conn, 1, false).unwrap();
        let entry = &list_all_entries(&conn, "all", EntryScope::All).unwrap()[0];
        assert!(!entry.is_starred);
        assert!(entry.is_read, "read state untouched by unstarring");

        assert!(set_starred(&conn, 999, true).is_err(), "unknown entry rejected");
    }

    #[test]
    fn entry_counts_cover_totals_and_stars() {
        let conn = open_memory().unwrap();
        let empty = entry_counts(&conn).unwrap();
        assert_eq!(
            (empty.all_entries, empty.starred_entries, empty.unread_entries),
            (0, 0, 0),
            "zero-entry case"
        );

        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        insert_entry(&conn, f, "g1", "T", None, "<p>x</p>", None).unwrap();
        insert_entry(&conn, f, "g2", "T", None, "<p>x</p>", None).unwrap();
        insert_entry(&conn, f, "g3", "T", None, "<p>x</p>", None).unwrap();

        let counts = entry_counts(&conn).unwrap();
        assert_eq!(
            (counts.all_entries, counts.starred_entries, counts.unread_entries),
            (3, 0, 3)
        );

        // Starred counts include read and unread entries alike.
        set_starred(&conn, 1, true).unwrap();
        set_starred(&conn, 3, true).unwrap();
        let counts = entry_counts(&conn).unwrap();
        assert_eq!(
            (counts.all_entries, counts.starred_entries, counts.unread_entries),
            (3, 2, 3)
        );

        // Read state changes the unread total, never the other two.
        set_read(&conn, 1, true).unwrap();
        let counts = entry_counts(&conn).unwrap();
        assert_eq!(
            (counts.all_entries, counts.starred_entries, counts.unread_entries),
            (3, 2, 2)
        );

        set_starred(&conn, 1, false).unwrap();
        let counts = entry_counts(&conn).unwrap();
        assert_eq!(
            (counts.all_entries, counts.starred_entries, counts.unread_entries),
            (3, 1, 2)
        );

        // The library-wide total is what the unread view of every scope adds up
        // to, so the global batch label can never overstate its range.
        set_read(&conn, 2, true).unwrap();
        set_read(&conn, 3, true).unwrap();
        assert_eq!(
            entry_counts(&conn).unwrap().unread_entries,
            count_entries(&conn, "unread", EntryScope::All).unwrap()
        );
    }

    #[test]
    fn unread_counts_are_per_feed() {
        let conn = open_memory().unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        insert_entry(&conn, a, "g1", "T", None, "<p>x</p>", None).unwrap();
        insert_entry(&conn, a, "g2", "T", None, "<p>x</p>", None).unwrap();
        insert_entry(&conn, b, "g3", "T", None, "<p>x</p>", None).unwrap();

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

    #[test]
    fn custom_title_overrides_display_without_touching_site_title() {
        let conn = open_memory().unwrap();
        let id = insert_or_update_feed(&conn, "https://a.example/feed", "Site Title", None).unwrap();

        set_feed_title(&conn, id, Some("  My Title  ")).unwrap();
        let feed = get_feed(&conn, id).unwrap().unwrap();
        assert_eq!(feed.title, "My Title", "custom title displayed (trimmed)");
        let site: String = conn
            .query_row("SELECT title FROM feeds WHERE id = ?1", params![id], |r| r.get(0))
            .unwrap();
        assert_eq!(site, "Site Title", "feed-provided title untouched");

        // A later fetch updates the site title but the display keeps the custom one.
        insert_or_update_feed(&conn, "https://a.example/feed", "Site Title v2", None).unwrap();
        let feed = get_feed(&conn, id).unwrap().unwrap();
        assert_eq!(feed.title, "My Title", "custom title survives a refresh");
        let listed = list_feeds(&conn).unwrap();
        assert_eq!(listed[0].title, "My Title", "list_feeds resolves display title");

        // Clearing reverts the display to the feed-provided title.
        set_feed_title(&conn, id, None).unwrap();
        let feed = get_feed(&conn, id).unwrap().unwrap();
        assert_eq!(feed.title, "Site Title v2", "clearing reverts to site title");

        // Empty/whitespace text also clears.
        set_feed_title(&conn, id, Some("   ")).unwrap();
        set_feed_title(&conn, id, Some("Again")).unwrap();
        set_feed_title(&conn, id, Some("")).unwrap();
        let feed = get_feed(&conn, id).unwrap().unwrap();
        assert_eq!(feed.title, "Site Title v2", "empty text clears the custom title");

        assert!(set_feed_title(&conn, 999, Some("X")).is_err(), "unknown feed rejected");
    }

    #[test]
    fn feed_ids_and_urls_in_folder_lists_only_its_feeds() {
        let conn = open_memory().unwrap();
        let tech = create_folder(&conn, "Tech").unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        move_feed_to_folder(&conn, a, Some(tech.id)).unwrap();
        // b stays uncategorized.

        let targets = feed_ids_and_urls_in_folder(&conn, tech.id).unwrap();
        assert_eq!(targets, vec![(a, "https://a.example/feed".to_string())]);
        assert!(
            !targets.iter().any(|(id, _)| *id == b),
            "uncategorized feed excluded"
        );
        assert!(feed_ids_and_urls_in_folder(&conn, 999).is_err(), "unknown folder rejected");
    }

    #[test]
    fn mark_read_many_marks_only_the_listed_ids_and_counts_real_changes() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        for i in 1..=4 {
            insert_entry(&conn, f, &format!("g{i}"), "T", None, "<p>x</p>", None).unwrap();
        }
        set_read(&conn, 3, true).unwrap();

        let changed = mark_read_many(&conn, &[1, 2, 3]).unwrap();
        assert_eq!(changed, 2, "an already-read id is not counted as changed");

        let rows = list_all_entries(&conn, "all", EntryScope::All).unwrap();
        assert_eq!(rows.len(), 4);
        let read: Vec<bool> = (1..=4).map(|id| rows_read(&conn, id)).collect();
        assert_eq!(read, vec![true, true, true, false], "id 4 untouched");

        assert_eq!(mark_read_many(&conn, &[]).unwrap(), 0, "empty batch is a no-op");
        assert_eq!(
            mark_read_many(&conn, &[999]).unwrap(),
            0,
            "unknown ids are ignored rather than failing the batch"
        );
    }

    #[test]
    fn mark_read_many_covers_batches_larger_than_one_chunk() {
        let conn = open_memory().unwrap();
        let f = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let total = READ_BATCH_CHUNK + 200;
        for i in 0..total {
            insert_entry(&conn, f, &format!("g{i}"), "T", None, "<p>x</p>", None).unwrap();
        }
        let ids: Vec<i64> = (1..=total as i64).collect();

        assert_eq!(mark_read_many(&conn, &ids).unwrap(), total as i64);
        assert_eq!(
            count_entries(&conn, "unread", EntryScope::All).unwrap(),
            0,
            "every chunk was applied"
        );
    }

    #[test]
    fn mark_scope_read_matches_the_unread_view_of_each_scope() {
        let conn = open_memory().unwrap();
        let tech = create_folder(&conn, "Tech").unwrap();
        let a = insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let b = insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();
        let c = insert_or_update_feed(&conn, "https://c.example/feed", "C", None).unwrap();
        move_feed_to_folder(&conn, a, Some(tech.id)).unwrap();
        move_feed_to_folder(&conn, b, Some(tech.id)).unwrap();
        insert_entry(&conn, a, "g1", "E1", None, "<p>x</p>", Some("2026-01-01T00:00:00+00:00")).unwrap();
        insert_entry(&conn, a, "g2", "E2", None, "<p>x</p>", Some("2026-01-02T00:00:00+00:00")).unwrap();
        insert_entry(&conn, b, "g3", "E3", None, "<p>x</p>", Some("2026-01-03T00:00:00+00:00")).unwrap();
        insert_entry(&conn, c, "g4", "E4", None, "<p>x</p>", Some("2026-01-04T00:00:00+00:00")).unwrap();
        set_read(&conn, 4, true).unwrap();

        let unread_before = count_entries(&conn, "unread", EntryScope::Folder { id: tech.id }).unwrap();
        let changed = mark_scope_read(&conn, EntryScope::Folder { id: tech.id }).unwrap();
        assert_eq!(changed, unread_before, "the scope's unread count is what changes");
        assert_eq!(count_entries(&conn, "unread", EntryScope::Folder { id: tech.id }).unwrap(), 0);
        assert_eq!(
            count_entries(&conn, "unread", EntryScope::Feed { id: c }).unwrap(),
            0,
            "the feed outside the folder was already read"
        );
        assert_eq!(
            mark_scope_read(&conn, EntryScope::Folder { id: tech.id }).unwrap(),
            0,
            "a second pass changes nothing"
        );

        // Feed scope leaves the sibling feed alone; All scope reaches everything.
        assert_eq!(mark_scope_read(&conn, EntryScope::Feed { id: c }).unwrap(), 0);
        insert_entry(&conn, c, "g5", "E5", None, "<p>x</p>", Some("2026-01-05T00:00:00+00:00")).unwrap();
        insert_entry(&conn, b, "g7", "E7", None, "<p>x</p>", Some("2026-01-07T00:00:00+00:00")).unwrap();
        assert_eq!(mark_scope_read(&conn, EntryScope::Feed { id: c }).unwrap(), 1);
        assert_eq!(
            count_entries(&conn, "unread", EntryScope::Feed { id: b }).unwrap(),
            1,
            "marking one feed leaves its sibling unread"
        );
        assert_eq!(mark_scope_read(&conn, EntryScope::All).unwrap(), 1);
        assert_eq!(count_entries(&conn, "unread", EntryScope::All).unwrap(), 0);
    }

    /// Read flags by id, in list order, for the batch tests' assertions.
    fn rows_read(conn: &Connection, id: i64) -> bool {
        conn.query_row("SELECT is_read FROM entries WHERE id = ?1", params![id], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
            != 0
    }

    // --- Shared-state bookkeeping ---------------------------------------

    /// An RFC3339 timestamp `days` before now. Tests derive their dates from
    /// the clock so the read window they exercise is always current.
    fn stamp_days_ago(days: i64) -> String {
        (Utc::now() - chrono::Duration::days(days)).to_rfc3339()
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
            params![name],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    }

    fn entry_id(conn: &Connection, guid: &str) -> i64 {
        conn.query_row(
            "SELECT id FROM entries WHERE guid = ?1",
            params![guid],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn feed_sync_ctr(conn: &Connection, feed_id: i64) -> i64 {
        conn.query_row(
            "SELECT sync_ctr FROM feeds WHERE id = ?1",
            params![feed_id],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn folder_fields(conn: &Connection, folder_id: i64) -> (String, i64, i64) {
        conn.query_row(
            "SELECT uuid, sync_ctr, name_ctr FROM folders WHERE id = ?1",
            params![folder_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    }

    /// The three shared-state fields of a feed, in the order they are compared.
    fn feed_counters(conn: &Connection, feed_id: i64) -> (i64, i64) {
        conn.query_row(
            "SELECT custom_title_ctr, folder_ctr FROM feeds WHERE id = ?1",
            params![feed_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn schema_creates_the_shared_state_tables() {
        let conn = open_memory().unwrap();
        for name in ["app_meta", "sync_marks", "sync_tombstones"] {
            assert!(table_exists(&conn, name), "{name} must exist");
        }
    }

    #[test]
    fn folders_get_a_distinct_uuid_when_an_older_database_is_migrated() {
        // The schema as it shipped before shared state existed.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL);")
            .unwrap();
        conn.execute("INSERT INTO folders (name) VALUES ('Tech'), ('Tech')", [])
            .unwrap();

        init_schema(&conn).unwrap();

        let uuids: Vec<String> = {
            let mut stmt = conn.prepare("SELECT uuid FROM folders ORDER BY id").unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            rows.collect::<Result<_, _>>().unwrap()
        };
        assert_eq!(uuids.len(), 2);
        assert!(uuids.iter().all(|u| !u.is_empty()), "every folder needs an identity");
        assert_ne!(uuids[0], uuids[1], "two folders sharing a name stay distinct");
    }

    #[test]
    fn shared_counters_default_to_zero_on_rows_stored_before_them() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE feeds (id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE,
                                 title TEXT NOT NULL, site_url TEXT,
                                 last_fetched_at TEXT, last_error TEXT);
             INSERT INTO feeds (url, title) VALUES ('https://a.example/feed', 'A');",
        )
        .unwrap();

        init_schema(&conn).unwrap();

        let ctr: i64 = conn
            .query_row("SELECT sync_ctr FROM feeds", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ctr, 0, "a pre-existing feed has the weakest ordering value");
    }

    #[test]
    fn device_id_is_stable_and_counters_increase() {
        let dir = std::env::temp_dir().join(format!("rss-device-test-{}", std::process::id()));
        let path = dir.join("reader.db");
        let _ = std::fs::remove_file(&path);

        let first = {
            let conn = open(&path).unwrap();
            device_id(&conn).unwrap()
        };
        let second = {
            let conn = open(&path).unwrap();
            device_id(&conn).unwrap()
        };
        assert_eq!(first, second, "the identity is written once and then stable");

        let conn = open(&path).unwrap();
        let a = next_counter(&conn).unwrap();
        let b = next_counter(&conn).unwrap();
        assert_eq!(b, a + 1, "every edit takes a strictly higher value");
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn folders_created_with_the_same_name_get_distinct_identities() {
        let conn = open_memory().unwrap();
        let a = create_folder(&conn, "Tech").unwrap();
        let b = create_folder(&conn, "Tech").unwrap();
        let (uuid_a, ctr_a, _) = folder_fields(&conn, a.id);
        let (uuid_b, ctr_b, _) = folder_fields(&conn, b.id);
        assert_ne!(uuid_a, uuid_b, "matching by name would merge two real folders");
        assert!(ctr_b > ctr_a);
    }

    #[test]
    fn each_shared_feed_field_carries_its_own_counter() {
        let conn = open_memory().unwrap();
        let folder = create_folder(&conn, "Tech").unwrap();
        let feed = insert_feed(&conn, "a");
        let before = feed_counters(&conn, feed);

        set_feed_title(&conn, feed, Some("Mine")).unwrap();
        let after_title = feed_counters(&conn, feed);
        assert!(after_title.0 > before.0, "the title edit is stamped");
        assert_eq!(after_title.1, before.1, "an unrelated field is left alone");

        move_feed_to_folder(&conn, feed, Some(folder.id)).unwrap();
        let after_folder = feed_counters(&conn, feed);
        assert!(after_folder.1 > after_title.1, "the folder edit is stamped");
        assert_eq!(after_folder.0, after_title.0, "the title stamp is unchanged");
    }

    #[test]
    fn marks_carry_the_entry_ordering_day_and_prune_by_it() {
        let conn = open_memory().unwrap();
        let feed = insert_feed(&conn, "a");
        let published = stamp_days_ago(0);
        insert_entry(&conn, feed, "g1", "T", None, "<p>x</p>", Some(&published)).unwrap();
        let id = entry_id(&conn, "g1");
        set_read(&conn, id, true).unwrap();

        let day: String = conn
            .query_row("SELECT day FROM sync_marks WHERE guid = 'g1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(day, &published[..10], "the day is the entry's ordering day");

        // A mark from long ago is what pruning is for.
        write_mark_read(&conn, "https://a.example/feed", "ancient", "2000-01-01", Some(true), 99)
            .unwrap();
        assert_eq!(prune_marks(&conn, &days_ago(READ_WINDOW_DAYS)).unwrap(), 1);
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_marks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 1, "only the mark inside the window survives");
    }

    #[test]
    fn a_mark_for_an_absent_entry_is_applied_when_it_arrives() {
        let conn = open_memory().unwrap();
        let feed = insert_feed(&conn, "a");
        let url = "https://a.example/feed";

        write_mark_star(&conn, url, "later", &days_ago(0), Some(true), 5).unwrap();
        assert_eq!(
            apply_marks_to_entries(&conn, None).unwrap(),
            0,
            "there is no entry to apply it to yet"
        );

        let published = stamp_days_ago(0);
        insert_entry(&conn, feed, "later", "T", None, "<p>x</p>", Some(&published)).unwrap();
        assert_eq!(apply_marks_to_entries(&conn, Some(feed)).unwrap(), 1);

        let starred: i64 = conn
            .query_row("SELECT is_starred FROM entries WHERE guid = 'later'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(starred, 1, "the mark recorded before arrival is applied");
        assert_eq!(apply_marks_to_entries(&conn, None).unwrap(), 0, "applying again changes nothing");
    }

    #[test]
    fn read_marks_are_recorded_only_inside_the_window() {
        let conn = open_memory().unwrap();
        let feed = insert_feed(&conn, "a");
        let recent = stamp_days_ago(0);
        let old = stamp_days_ago(30);
        insert_entry(&conn, feed, "recent", "R", None, "<p>x</p>", Some(&recent)).unwrap();
        insert_entry(&conn, feed, "old", "O", None, "<p>x</p>", Some(&old)).unwrap();

        let recent_id = entry_id(&conn, "recent");
        let old_id = entry_id(&conn, "old");
        set_read(&conn, recent_id, true).unwrap();
        set_read(&conn, old_id, true).unwrap();

        assert!(rows_read(&conn, recent_id) && rows_read(&conn, old_id));
        let shared: Vec<String> = {
            let mut stmt = conn.prepare("SELECT guid FROM sync_marks ORDER BY guid").unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            rows.collect::<Result<_, _>>().unwrap()
        };
        assert_eq!(shared, vec!["recent"], "an old read mark is local only");
    }

    #[test]
    fn stars_are_recorded_regardless_of_the_window() {
        let conn = open_memory().unwrap();
        let feed = insert_feed(&conn, "a");
        let old = stamp_days_ago(300);
        insert_entry(&conn, feed, "old", "O", None, "<p>x</p>", Some(&old)).unwrap();
        let id = entry_id(&conn, "old");

        set_starred(&conn, id, true).unwrap();

        let star: Option<i64> = conn
            .query_row("SELECT star FROM sync_marks WHERE guid = 'old'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(star, Some(1), "stars never expire");
    }

    #[test]
    fn the_ledger_is_seeded_once_from_an_existing_library() {
        let conn = open_memory().unwrap();
        let feed = insert_feed(&conn, "a");
        let recent = stamp_days_ago(1);
        let old = stamp_days_ago(300);
        insert_entry(&conn, feed, "recent", "R", None, "<p>x</p>", Some(&recent)).unwrap();
        insert_entry(&conn, feed, "old", "O", None, "<p>x</p>", Some(&old)).unwrap();
        // Written directly, the way a build without a ledger would have.
        conn.execute("UPDATE entries SET is_read = 1 WHERE guid = 'recent'", [])
            .unwrap();
        conn.execute("UPDATE entries SET is_starred = 1 WHERE guid = 'old'", [])
            .unwrap();
        conn.execute("DELETE FROM app_meta WHERE key = 'marks_seeded'", [])
            .unwrap();

        init_schema(&conn).unwrap();

        let seeded: Vec<(String, i64, i64)> = {
            let mut stmt = conn
                .prepare("SELECT guid, read_ctr, star_ctr FROM sync_marks ORDER BY guid")
                .unwrap();
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .unwrap();
            rows.collect::<Result<_, _>>().unwrap()
        };
        assert_eq!(seeded, vec![("old".into(), 0, 0), ("recent".into(), 0, 0)]);
        assert!(
            seeded.iter().all(|(_, read_ctr, star_ctr)| *read_ctr == 0 && *star_ctr == 0),
            "seeded rows are the weakest possible, so any real edit outranks them"
        );

        init_schema(&conn).unwrap();
        let after: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_marks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(after, 2, "seeding does not repeat");
    }

    #[test]
    fn deleting_records_a_tombstone_and_still_cascades() {
        let conn = open_memory().unwrap();
        let feed = insert_feed(&conn, "a");
        let published = stamp_days_ago(0);
        insert_entry(&conn, feed, "g", "T", None, "<p>x</p>", Some(&published)).unwrap();
        let folder = create_folder(&conn, "Tech").unwrap();
        let (folder_uuid, _, _) = folder_fields(&conn, folder.id);

        delete_feed(&conn, feed).unwrap();
        delete_folder(&conn, folder.id).unwrap();

        let entries: i64 = conn
            .query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(entries, 0, "entries still cascade");
        assert!(
            tombstone_ctr(&conn, TOMBSTONE_FEED, "https://a.example/feed").unwrap() > 0,
            "the removal is remembered"
        );
        assert!(tombstone_ctr(&conn, TOMBSTONE_FOLDER, &folder_uuid).unwrap() > 0);
    }

    #[test]
    fn old_tombstones_are_pruned() {
        let conn = open_memory().unwrap();
        raise_tombstone(&conn, TOMBSTONE_FEED, "https://old.example/feed", 3, "2000-01-01")
            .unwrap();
        raise_tombstone(&conn, TOMBSTONE_FEED, "https://new.example/feed", 4, &days_ago(1))
            .unwrap();

        assert_eq!(prune_tombstones(&conn).unwrap(), 1);
        assert_eq!(tombstones_all(&conn).unwrap().len(), 1);
    }

    #[test]
    fn adding_a_feed_again_outranks_its_own_tombstone() {
        let conn = open_memory().unwrap();
        let first = insert_feed(&conn, "a");
        delete_feed(&conn, first).unwrap();
        let url = "https://a.example/feed";
        let removed_at = tombstone_ctr(&conn, TOMBSTONE_FEED, url).unwrap();

        let again = insert_feed(&conn, "a");

        assert!(
            feed_sync_ctr(&conn, again) > removed_at,
            "re-adding is a newer intent than the removal"
        );
        let listed = list_feeds(&conn).unwrap();
        assert_eq!(listed.len(), 1);
    }
}
