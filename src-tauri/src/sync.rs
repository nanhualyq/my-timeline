//! The sync document: what a reader's shared library state looks like outside
//! this installation, and how two copies of it merge back into one.
//!
//! Everything here except `project` and `apply` is a pure function over the
//! document. That is deliberate: order-independence and idempotence are the
//! properties the format exists for, and they are only testable while the merge
//! is separate from the database. A later change puts the same document on a
//! network transport without touching any of this.

use crate::db;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;


/// The only format version this build writes or accepts. It is separate from
/// the application version so that changing the format is a deliberate act.
pub const DOC_VERSION: i64 = 1;

/// A folder as it is shared: identified by a generated uuid, never by a local
/// id, because the ids differ on every installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderRecord {
    pub uuid: String,
    pub name: String,
    pub name_ctr: i64,
    /// The counter this folder was created at. A removal recorded at a higher
    /// counter supersedes it.
    pub sync_ctr: i64,
}

/// A subscription. Entry bodies, fetch times, and errors are deliberately
/// absent: every installation re-acquires those from the feed itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedRecord {
    pub url: String,
    /// The reader's own title for the feed. A feed-provided title is never
    /// shared, because each installation derives it from its own fetch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default)]
    pub title_ctr: i64,
    /// The uuid of the folder the feed is filed under, when it is filed at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default)]
    pub folder_ctr: i64,
    pub sync_ctr: i64,
}

/// One entry's shared intent. An absent `read` or `star` means the value was
/// never set, which is what makes an explicit "false" — marking unread or
/// removing a star — survivable, instead of being reinstated by the other side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkRecord {
    pub feed: String,
    pub guid: String,
    /// The entry's ordering day, used to bound read marks.
    pub day: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read: Option<bool>,
    #[serde(default)]
    pub read_ctr: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub star: Option<bool>,
    #[serde(default)]
    pub star_ctr: i64,
}

/// A remembered removal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TombstoneRecord {
    /// `db::TOMBSTONE_FEED` or `db::TOMBSTONE_FOLDER`.
    pub kind: String,
    /// The feed URL, or the folder uuid.
    pub key: String,
    pub ctr: i64,
    pub day: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub doc_version: i64,
    /// The calendar day before which read marks are not shared. Stored in the
    /// document rather than derived from a clock, so two installations always
    /// agree on the window they are applying.
    pub read_horizon_day: String,
    #[serde(default)]
    pub folders: Vec<FolderRecord>,
    #[serde(default)]
    pub feeds: Vec<FeedRecord>,
    #[serde(default)]
    pub marks: Vec<MarkRecord>,
    #[serde(default)]
    pub tombstones: Vec<TombstoneRecord>,
    /// Reserved for reader settings, which this build neither writes nor reads.
    /// Absent means "none", so an older document needs no version bump.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
}

/// What an export produced.
#[derive(Debug, Clone, Serialize)]
pub struct ExportOutcome {
    pub text: String,
    pub feeds_exported: i64,
    pub folders_exported: i64,
    pub marks_exported: i64,
}

/// What an import applied. Every figure counts actual changes, so importing the
/// same document twice reports zero the second time.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportSummary {
    pub folders_added: i64,
    pub folders_changed: i64,
    pub folders_removed: i64,
    pub feeds_added: i64,
    pub feeds_changed: i64,
    pub feeds_removed: i64,
    pub stars_applied: i64,
    pub read_marks_applied: i64,
    /// Marks kept for entries this installation has not fetched yet.
    pub marks_recorded_for_later: i64,
}

// --- Merging -------------------------------------------------------------

/// Resolve one independently settable field from two sides.
///
/// The higher counter wins. An exact tie is broken by the value itself rather
/// than by argument order, which is what keeps the merge symmetric: two
/// installations that never saw each other can produce the same counter.
#[allow(dead_code)]
fn pick_field<T: Ord + Clone>((a, a_ctr): (Option<T>, i64), (b, b_ctr): (Option<T>, i64)) -> (Option<T>, i64) {
    if a_ctr != b_ctr {
        return if a_ctr > b_ctr { (a, a_ctr) } else { (b, b_ctr) };
    }
    match (a, b) {
        (Some(a), Some(b)) => {
            if a >= b {
                (Some(a), a_ctr)
            } else {
                (Some(b), b_ctr)
            }
        }
        (Some(a), None) => (Some(a), a_ctr),
        (None, Some(b)) => (Some(b), b_ctr),
        (None, None) => (None, a_ctr),
    }
}

/// Combine two documents into one that represents the union of both.
///
/// Order-independent and idempotent by construction: every field is resolved
/// by the same symmetric rule, and re-merging a document with itself reproduces
/// it. The result is sorted, so two installations that merged the same inputs
/// produce byte-identical documents.
#[allow(dead_code)]
pub fn merge(a: &Document, b: &Document) -> Document {
    let mut folders: BTreeMap<String, FolderRecord> = BTreeMap::new();
    for record in a.folders.iter().chain(b.folders.iter()) {
        folders
            .entry(record.uuid.clone())
            .and_modify(|current| {
                let (name, name_ctr) = pick_field(
                    (Some(current.name.clone()), current.name_ctr),
                    (Some(record.name.clone()), record.name_ctr),
                );
                current.name = name.unwrap_or_default();
                current.name_ctr = name_ctr;
                current.sync_ctr = current.sync_ctr.max(record.sync_ctr);
            })
            .or_insert_with(|| record.clone());
    }

    let mut feeds: BTreeMap<String, FeedRecord> = BTreeMap::new();
    for record in a.feeds.iter().chain(b.feeds.iter()) {
        feeds
            .entry(record.url.clone())
            .and_modify(|current| {
                let (title, title_ctr) = pick_field(
                    (current.custom_title.clone(), current.title_ctr),
                    (record.custom_title.clone(), record.title_ctr),
                );
                let (folder, folder_ctr) = pick_field(
                    (current.folder.clone(), current.folder_ctr),
                    (record.folder.clone(), record.folder_ctr),
                );
                current.custom_title = title;
                current.title_ctr = title_ctr;
                current.folder = folder;
                current.folder_ctr = folder_ctr;
                current.sync_ctr = current.sync_ctr.max(record.sync_ctr);
            })
            .or_insert_with(|| record.clone());
    }

    let mut marks: BTreeMap<(String, String), MarkRecord> = BTreeMap::new();
    for record in a.marks.iter().chain(b.marks.iter()) {
        marks
            .entry((record.feed.clone(), record.guid.clone()))
            .and_modify(|current| {
                let (read, read_ctr) = pick_field(
                    (current.read, current.read_ctr),
                    (record.read, record.read_ctr),
                );
                let (star, star_ctr) = pick_field(
                    (current.star, current.star_ctr),
                    (record.star, record.star_ctr),
                );
                current.read = read;
                current.read_ctr = read_ctr;
                current.star = star;
                current.star_ctr = star_ctr;
                // The later of the two days keeps a mark inside the window for
                // as long as either side believes it is.
                current.day = current.day.clone().max(record.day.clone());
            })
            .or_insert_with(|| record.clone());
    }

    let mut tombstones: BTreeMap<(String, String), TombstoneRecord> = BTreeMap::new();
    for record in a.tombstones.iter().chain(b.tombstones.iter()) {
        tombstones
            .entry((record.kind.clone(), record.key.clone()))
            .and_modify(|current| {
                current.ctr = current.ctr.max(record.ctr);
                current.day = current.day.clone().max(record.day.clone());
            })
            .or_insert_with(|| record.clone());
    }

    let feed_removals: BTreeMap<&String, i64> = tombstones
        .values()
        .filter(|t| t.kind == db::TOMBSTONE_FEED)
        .map(|t| (&t.key, t.ctr))
        .collect();
    let folder_removals: BTreeMap<&String, i64> = tombstones
        .values()
        .filter(|t| t.kind == db::TOMBSTONE_FOLDER)
        .map(|t| (&t.key, t.ctr))
        .collect();

    // A removal outranks anything created before it. The record stays only when
    // it was created afterwards — that is, when the reader added it again.
    feeds.retain(|url, record| match feed_removals.get(url) {
        Some(removed_at) => record.sync_ctr > *removed_at,
        None => true,
    });
    folders.retain(|uuid, record| match folder_removals.get(uuid) {
        Some(removed_at) => record.sync_ctr > *removed_at,
        None => true,
    });

    Document {
        doc_version: a.doc_version.max(b.doc_version),
        read_horizon_day: a.read_horizon_day.clone().max(b.read_horizon_day.clone()),
        folders: folders.into_values().collect(),
        feeds: feeds.into_values().collect(),
        marks: marks.into_values().collect(),
        tombstones: tombstones.into_values().collect(),
        settings: a.settings.clone().or_else(|| b.settings.clone()),
    }
}

/// Whether a record created at `sync_ctr` survives a removal recorded at
/// `removed_at` (0 meaning "no removal").
pub fn survives_removal(sync_ctr: i64, removed_at: i64) -> bool {
    removed_at == 0 || sync_ctr > removed_at
}

// --- Database bridge -----------------------------------------------------

/// Read the local library as a document. The read window is advanced and the
/// ledger and tombstones are pruned first, so the result is what this
/// installation currently believes, not everything it ever recorded.
pub fn project(conn: &Connection) -> Result<Document, String> {
    let horizon = db::advance_read_horizon(conn, None).map_err(|e| e.to_string())?;
    db::prune_marks(conn, &horizon).map_err(|e| e.to_string())?;
    db::prune_tombstones(conn).map_err(|e| e.to_string())?;

    let folders = {
        let mut stmt = conn
            .prepare("SELECT uuid, name, name_ctr, sync_ctr FROM folders ORDER BY uuid")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(FolderRecord {
                    uuid: r.get(0)?,
                    name: r.get(1)?,
                    name_ctr: r.get(2)?,
                    sync_ctr: r.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    let feeds = {
        // The folder is referenced by uuid so the document survives a copy.
        let mut stmt = conn
            .prepare(
                "SELECT f.url, f.custom_title, f.custom_title_ctr, fo.uuid, f.folder_ctr, f.sync_ctr
                 FROM feeds f LEFT JOIN folders fo ON fo.id = f.folder_id
                 ORDER BY f.url",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(FeedRecord {
                    url: r.get(0)?,
                    custom_title: r.get(1)?,
                    title_ctr: r.get(2)?,
                    folder: r.get(3)?,
                    folder_ctr: r.get(4)?,
                    sync_ctr: r.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };

    // Marks older than the window are not shared, whether or not they survived
    // pruning above (a starred row keeps its read state, and stays).
    let marks = db::marks_all(conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|m| m.day >= horizon || m.star.is_some())
        .map(|m| MarkRecord {
            feed: m.feed_url,
            guid: m.guid,
            day: m.day,
            read: m.read,
            read_ctr: m.read_ctr,
            star: m.star,
            star_ctr: m.star_ctr,
        })
        .collect();

    let tombstones = db::tombstones_all(conn)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|t| TombstoneRecord {
            kind: t.kind,
            key: t.key,
            ctr: t.ctr,
            day: t.day,
        })
        .collect();

    Ok(Document {
        doc_version: DOC_VERSION,
        read_horizon_day: horizon,
        folders,
        feeds,
        marks,
        tombstones,
        settings: None,
    })
}

/// Render the current shared state as document text.
pub fn export(conn: &Connection) -> Result<ExportOutcome, String> {
    let document = project(conn)?;
    let text = serde_json::to_string_pretty(&document).map_err(|e| e.to_string())?;
    Ok(ExportOutcome {
        text,
        feeds_exported: document.feeds.len() as i64,
        folders_exported: document.folders.len() as i64,
        marks_exported: document.marks.len() as i64,
    })
}

/// Parse and merge a document into the local library.
///
/// Everything is validated before the first write, so an unreadable document or
/// one from another format version leaves the library untouched.
pub fn import(conn: &Connection, text: &str) -> Result<ImportSummary, String> {
    let document: Document = serde_json::from_str(text)
        .map_err(|e| format!("not a sync document: {e}"))?;
    if document.doc_version != DOC_VERSION {
        return Err(format!(
            "unsupported sync document version {}; this build supports {}",
            document.doc_version, DOC_VERSION
        ));
    }
    apply(conn, &document)
}

/// The highest ordering value the document mentions.
fn highest_counter(document: &Document) -> i64 {
    document
        .folders
        .iter()
        .flat_map(|f| [f.name_ctr, f.sync_ctr])
        .chain(
            document
                .feeds
                .iter()
                .flat_map(|f| [f.title_ctr, f.folder_ctr, f.sync_ctr]),
        )
        .chain(
            document
                .marks
                .iter()
                .flat_map(|m| [m.read_ctr, m.star_ctr]),
        )
        .chain(document.tombstones.iter().map(|t| t.ctr))
        .max()
        .unwrap_or(0)
}

fn apply(conn: &Connection, document: &Document) -> Result<ImportSummary, String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut summary = ImportSummary::default();

    // From here on, any local edit outranks everything the document contains.
    db::raise_counter(&tx, highest_counter(document)).map_err(|e| e.to_string())?;

    // The window the document was written with, advanced by at most a day so a
    // skewed clock on the writing side cannot throw away current marks here.
    let horizon = db::advance_read_horizon(&tx, Some(&document.read_horizon_day))
        .map_err(|e| e.to_string())?;
    db::prune_marks(&tx, &horizon).map_err(|e| e.to_string())?;

    // Removals first: nothing superseded by a removal should be created.
    for removal in &document.tombstones {
        db::raise_tombstone(&tx, &removal.kind, &removal.key, removal.ctr, &removal.day)
            .map_err(|e| e.to_string())?;
        match removal.kind.as_str() {
            db::TOMBSTONE_FEED => {
                summary.feeds_removed += tx
                    .execute(
                        "DELETE FROM feeds WHERE url = ?1 AND sync_ctr <= ?2",
                        params![removal.key, removal.ctr],
                    )
                    .map_err(|e| e.to_string())? as i64;
            }
            db::TOMBSTONE_FOLDER => {
                // Its feeds move to uncategorized, exactly as a local delete
                // would leave them.
                tx.execute(
                    "UPDATE feeds SET folder_id = NULL WHERE folder_id IN
                       (SELECT id FROM folders WHERE uuid = ?1 AND sync_ctr <= ?2)",
                    params![removal.key, removal.ctr],
                )
                .map_err(|e| e.to_string())?;
                summary.folders_removed += tx
                    .execute(
                        "DELETE FROM folders WHERE uuid = ?1 AND sync_ctr <= ?2",
                        params![removal.key, removal.ctr],
                    )
                    .map_err(|e| e.to_string())? as i64;
            }
            _ => {}
        }
    }

    for folder in &document.folders {
        let removed_at = db::tombstone_ctr(&tx, db::TOMBSTONE_FOLDER, &folder.uuid)
            .map_err(|e| e.to_string())?;
        if !survives_removal(folder.sync_ctr, removed_at) {
            continue;
        }
        let existing: Option<(i64, String, i64)> = tx
            .query_row(
                "SELECT id, name, name_ctr FROM folders WHERE uuid = ?1",
                params![folder.uuid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match existing {
            None => {
                tx.execute(
                    "INSERT INTO folders (uuid, name, name_ctr, sync_ctr)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![folder.uuid, folder.name, folder.name_ctr, folder.sync_ctr],
                )
                .map_err(|e| e.to_string())?;
                summary.folders_added += 1;
            }
            Some((id, name, name_ctr)) => {
                if folder.name_ctr > name_ctr && folder.name != name {
                    tx.execute(
                        "UPDATE folders SET name = ?1, name_ctr = ?2 WHERE id = ?3",
                        params![folder.name, folder.name_ctr, id],
                    )
                    .map_err(|e| e.to_string())?;
                    summary.folders_changed += 1;
                }
            }
        }
    }

    for feed in &document.feeds {
        let removed_at =
            db::tombstone_ctr(&tx, db::TOMBSTONE_FEED, &feed.url).map_err(|e| e.to_string())?;
        if !survives_removal(feed.sync_ctr, removed_at) {
            continue;
        }
        let folder_id: Option<i64> = match &feed.folder {
            Some(uuid) => tx
                .query_row(
                    "SELECT id FROM folders WHERE uuid = ?1",
                    params![uuid],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?,
            None => None,
        };
        let existing: Option<(i64, Option<String>, i64, i64)> = tx
            .query_row(
                "SELECT id, custom_title, custom_title_ctr, folder_ctr FROM feeds WHERE url = ?1",
                params![feed.url],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match existing {
            None => {
                // The feed-provided title is unknown until this installation
                // fetches it; the URL is a truthful stand-in until then.
                let title = feed.custom_title.clone().unwrap_or_else(|| feed.url.clone());
                tx.execute(
                    "INSERT INTO feeds
                       (url, title, custom_title, custom_title_ctr, folder_id, folder_ctr, sync_ctr)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        feed.url,
                        title,
                        feed.custom_title,
                        feed.title_ctr,
                        folder_id,
                        feed.folder_ctr,
                        feed.sync_ctr
                    ],
                )
                .map_err(|e| e.to_string())?;
                summary.feeds_added += 1;
            }
            Some((id, _custom_title, title_ctr, folder_ctr)) => {
                let mut changed = false;
                if feed.title_ctr > title_ctr {
                    tx.execute(
                        "UPDATE feeds SET custom_title = ?1, custom_title_ctr = ?2 WHERE id = ?3",
                        params![feed.custom_title, feed.title_ctr, id],
                    )
                    .map_err(|e| e.to_string())?;
                    changed = true;
                }
                if feed.folder_ctr > folder_ctr && feed.folder.is_some() {
                    tx.execute(
                        "UPDATE feeds SET folder_id = ?1, folder_ctr = ?2 WHERE id = ?3",
                        params![folder_id, feed.folder_ctr, id],
                    )
                    .map_err(|e| e.to_string())?;
                    changed = true;
                }
                if changed {
                    summary.feeds_changed += 1;
                }
            }
        }
    }

    for mark in &document.marks {
        let mut changed = false;
        if mark.read_ctr > 0 {
            changed |= db::write_mark_read(
                &tx, &mark.feed, &mark.guid, &mark.day, mark.read, mark.read_ctr,
            )
            .map_err(|e| e.to_string())?;
        }
        if mark.star_ctr > 0 {
            changed |= db::write_mark_star(
                &tx, &mark.feed, &mark.guid, &mark.day, mark.star, mark.star_ctr,
            )
            .map_err(|e| e.to_string())?;
        }
        if !changed {
            continue;
        }
        // Whether the mark landed on an entry or is waiting for one decides how
        // the reader is told about it.
        let present: bool = tx
            .query_row(
                "SELECT 1 FROM entries e JOIN feeds f ON f.id = e.feed_id
                 WHERE f.url = ?1 AND e.guid = ?2",
                params![mark.feed, mark.guid],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .is_some();
        if present {
            if mark.read_ctr > 0 {
                summary.read_marks_applied += 1;
            }
            if mark.star_ctr > 0 {
                summary.stars_applied += 1;
            }
        } else {
            summary.marks_recorded_for_later += 1;
        }
    }

    db::apply_marks_to_entries(&tx, None).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(summary)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, EntryScope};
    use chrono::Utc;

    fn stamp_days_ago(days: i64) -> String {
        (Utc::now() - chrono::Duration::days(days)).to_rfc3339()
    }

    fn days_before(days: i64) -> String {
        (Utc::now() - chrono::Duration::days(days))
            .format("%Y-%m-%d")
            .to_string()
    }

    fn entry_id(conn: &Connection, guid: &str) -> i64 {
        conn.query_row(
            "SELECT id FROM entries WHERE guid = ?1",
            params![guid],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn folder_id_for_uuid(conn: &Connection, uuid: &str) -> i64 {
        conn.query_row(
            "SELECT id FROM folders WHERE uuid = ?1",
            params![uuid],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// A library with one feed, one recent entry, one star, one read mark, and
    /// one folder: enough state to exercise every record kind.
    fn seeded_library() -> Connection {
        let conn = db::open_memory().unwrap();
        let folder = db::create_folder(&conn, "Tech").unwrap();
        let feed = db::insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        db::move_feed_to_folder(&conn, feed, Some(folder.id)).unwrap();
        db::upsert_entry(
            &conn,
            feed,
            "g1",
            "Headline",
            Some("https://a.example/1"),
            "<p>body text</p>",
            "body text",
            None,
            Some(&stamp_days_ago(0)),
        )
        .unwrap();
        let id = entry_id(&conn, "g1");
        db::set_starred(&conn, id, true).unwrap();
        db::set_read(&conn, id, true).unwrap();
        conn
    }

    fn parse(text: &str) -> Document {
        serde_json::from_str(text).unwrap()
    }

    // --- Removal rules ---------------------------------------------------

    #[test]
    fn a_record_survives_unless_a_removal_outranks_it() {
        assert!(survives_removal(1, 0), "no removal means it stays");
        assert!(survives_removal(5, 4), "created after the removal, it survives");
        assert!(!survives_removal(4, 4), "the same counter is not newer");
        assert!(!survives_removal(3, 9), "recorded later, the removal wins");
    }

    #[test]
    fn a_removal_is_not_undone_by_a_stale_document() {
        let conn = db::open_memory().unwrap();
        let feed = db::insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let stale = export(&conn).unwrap().text;
        db::delete_feed(&conn, feed).unwrap();
        let current = export(&conn).unwrap().text;

        let merged = merge(&parse(&current), &parse(&stale));
        assert!(merged.feeds.is_empty(), "the removal outranks the stale listing");

        import(&conn, &stale).unwrap();
        assert!(
            db::list_feeds(&conn).unwrap().is_empty(),
            "importing the stale copy does not resurrect the feed"
        );
    }

    #[test]
    fn a_feed_added_again_after_its_removal_survives_the_merge() {
        let conn = db::open_memory().unwrap();
        let first = db::insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        db::delete_feed(&conn, first).unwrap();
        db::insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        let current = export(&conn).unwrap().text;

        // A document written between the removal and the re-add.
        let stale = Document {
            doc_version: DOC_VERSION,
            read_horizon_day: days_before(7),
            folders: vec![],
            feeds: vec![],
            marks: vec![],
            tombstones: vec![TombstoneRecord {
                kind: db::TOMBSTONE_FEED.into(),
                key: "https://a.example/feed".into(),
                ctr: 2,
                day: days_before(1),
            }],
            settings: None,
        };
        let merged = merge(&parse(&current), &stale);
        assert_eq!(merged.feeds.len(), 1, "the newer re-add wins");
    }

    // --- Document shape --------------------------------------------------

    #[test]
    fn the_document_keys_by_copyable_identity_and_carries_no_content() {
        let conn = seeded_library();
        let outcome = export(&conn).unwrap();

        assert!(!outcome.text.contains("body text"), "entry content is not shared");
        assert!(!outcome.text.contains("Headline"), "entry headlines are not shared");

        let document = parse(&outcome.text);
        assert_eq!(document.doc_version, DOC_VERSION);
        assert_eq!(document.feeds.len(), 1);
        assert_eq!(document.feeds[0].url, "https://a.example/feed");
        let uuid = document.folders[0].uuid.clone();
        assert_eq!(
            document.feeds[0].folder.as_deref(),
            Some(uuid.as_str()),
            "a feed names its folder by uuid, not by a local id"
        );
        assert_eq!(document.marks.len(), 1);
        assert_eq!(document.marks[0].feed, "https://a.example/feed");
        assert_eq!(document.marks[0].guid, "g1");
        assert_eq!(document.marks[0].star, Some(true));
        assert_eq!(document.marks[0].read, Some(true));
        assert_eq!(document.settings, None, "settings are reserved, not written");
    }

    #[test]
    fn a_document_round_trips_through_its_own_text() {
        let conn = seeded_library();
        let text = export(&conn).unwrap().text;
        let reparsed = parse(&text);
        assert_eq!(reparsed, project(&conn).unwrap());
        let again = serde_json::to_string(&reparsed).unwrap();
        assert_eq!(parse(&again), reparsed);
    }

    #[test]
    fn an_older_document_without_optional_sections_still_parses() {
        let document = parse(r#"{"doc_version": 1, "read_horizon_day": "2026-01-01"}"#);
        assert!(document.feeds.is_empty());
        assert!(document.marks.is_empty());
        assert!(document.tombstones.is_empty());
        assert_eq!(document.settings, None);
    }

    // --- Merge properties ------------------------------------------------

    /// Small deterministic generator: the properties below need many document
    /// pairs, not realistic ones, and a dependency for that is not worth it.
    struct Rng(u64);

    impl Rng {
        fn below(&mut self, n: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 33) % n.max(1)
        }
    }

    fn random_document(rng: &mut Rng) -> Document {
        let day = format!("2026-01-0{}", rng.below(9) + 1);
        let folders = (0..rng.below(3))
            .map(|i| FolderRecord {
                uuid: format!("f{i}"),
                name: format!("Folder {}", rng.below(2)),
                name_ctr: rng.below(4) as i64,
                sync_ctr: rng.below(3) as i64,
            })
            .collect();
        let feeds = (0..rng.below(4))
            .map(|i| FeedRecord {
                url: format!("https://f{i}.example/feed"),
                custom_title: if rng.below(2) == 0 {
                    Some(format!("Title {}", rng.below(2)))
                } else {
                    None
                },
                title_ctr: rng.below(4) as i64,
                folder: if rng.below(2) == 0 {
                    Some(format!("f{}", rng.below(3)))
                } else {
                    None
                },
                folder_ctr: rng.below(4) as i64,
                sync_ctr: rng.below(3) as i64,
            })
            .collect();
        let marks = (0..rng.below(4))
            .map(|i| MarkRecord {
                feed: format!("https://f{}.example/feed", rng.below(4)),
                guid: format!("g{i}"),
                day: day.clone(),
                read: if rng.below(3) == 0 {
                    None
                } else {
                    Some(rng.below(2) == 0)
                },
                read_ctr: rng.below(4) as i64,
                star: if rng.below(3) == 0 {
                    None
                } else {
                    Some(rng.below(2) == 0)
                },
                star_ctr: rng.below(4) as i64,
            })
            .collect();
        let tombstones = (0..rng.below(2))
            .map(|i| TombstoneRecord {
                kind: if rng.below(2) == 0 {
                    db::TOMBSTONE_FEED.into()
                } else {
                    db::TOMBSTONE_FOLDER.into()
                },
                key: format!("k{i}"),
                ctr: rng.below(3) as i64,
                day: day.clone(),
            })
            .collect();
        Document {
            doc_version: DOC_VERSION,
            read_horizon_day: day,
            folders,
            feeds,
            marks,
            tombstones,
            settings: None,
        }
    }

    #[test]
    fn merging_is_order_independent_and_idempotent() {
        let mut rng = Rng(42);
        for _ in 0..300 {
            let a = random_document(&mut rng);
            let b = random_document(&mut rng);
            assert_eq!(
                merge(&a, &b),
                merge(&b, &a),
                "argument order must not change the result"
            );

            let ab = merge(&a, &b);
            assert_eq!(merge(&ab, &ab), ab, "a merged document is its own fixed point");
            assert_eq!(merge(&ab, &a), ab, "re-merging one input changes nothing");
            assert_eq!(merge(&ab, &b), ab, "re-merging the other input changes nothing");
        }
    }

    #[test]
    fn a_conflict_is_decided_by_the_recorded_order() {
        let conn = db::open_memory().unwrap();
        let feed = db::insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        db::set_feed_title(&conn, feed, Some("Old name")).unwrap();
        let earlier = parse(&export(&conn).unwrap().text);

        db::set_feed_title(&conn, feed, Some("New name")).unwrap();
        let later = parse(&export(&conn).unwrap().text);

        let merged = merge(&earlier, &later);
        assert_eq!(merged.feeds[0].custom_title.as_deref(), Some("New name"));
        assert_eq!(merged.feeds[0].title_ctr, later.feeds[0].title_ctr);
    }

    #[test]
    fn clearing_a_field_can_win_over_a_set_value() {
        let conn = db::open_memory().unwrap();
        let feed = db::insert_or_update_feed(&conn, "https://a.example/feed", "A", None).unwrap();
        db::set_feed_title(&conn, feed, Some("Mine")).unwrap();
        let with_title = parse(&export(&conn).unwrap().text);

        db::set_feed_title(&conn, feed, None).unwrap();
        let cleared = parse(&export(&conn).unwrap().text);

        let merged = merge(&with_title, &cleared);
        assert_eq!(
            merged.feeds[0].custom_title, None,
            "clearing is an edit, not an absence"
        );
    }

    #[test]
    fn removing_a_star_can_win_over_a_set_star() {
        let conn = seeded_library();
        let starred = parse(&export(&conn).unwrap().text);
        let id = entry_id(&conn, "g1");
        db::set_starred(&conn, id, false).unwrap();
        let unstarred = parse(&export(&conn).unwrap().text);

        let merged = merge(&starred, &unstarred);
        assert_eq!(merged.marks[0].star, Some(false));
    }

    // --- Import ----------------------------------------------------------

    #[test]
    fn an_exported_document_reproduces_itself_in_an_empty_library() {
        let source = seeded_library();
        let first = export(&source).unwrap().text;

        let target = db::open_memory().unwrap();
        let summary = import(&target, &first).unwrap();
        assert_eq!(summary.folders_added, 1);
        assert_eq!(summary.feeds_added, 1);
        assert_eq!(
            summary.marks_recorded_for_later, 1,
            "no entries exist here yet"
        );

        assert_eq!(
            export(&target).unwrap().text,
            first,
            "the same shared state comes back out"
        );
    }

    #[test]
    fn importing_the_same_document_twice_reports_no_further_changes() {
        let source = seeded_library();
        let text = export(&source).unwrap().text;
        let target = db::open_memory().unwrap();

        let first = import(&target, &text).unwrap();
        assert!(first.feeds_added > 0 && first.folders_added > 0);

        let second = import(&target, &text).unwrap();
        assert_eq!(second.folders_added, 0);
        assert_eq!(second.folders_changed, 0);
        assert_eq!(second.folders_removed, 0);
        assert_eq!(second.feeds_added, 0);
        assert_eq!(second.feeds_changed, 0);
        assert_eq!(second.feeds_removed, 0);
        assert_eq!(second.stars_applied, 0);
        assert_eq!(second.read_marks_applied, 0);
        assert_eq!(second.marks_recorded_for_later, 0);
    }

    #[test]
    fn an_unsupported_version_or_unreadable_text_changes_nothing() {
        let conn = seeded_library();
        let before = export(&conn).unwrap().text;

        let wrong_version =
            import(&conn, r#"{"doc_version": 99, "read_horizon_day": "2026-01-01"}"#).unwrap_err();
        assert!(wrong_version.contains("version 99"), "{wrong_version}");

        let garbage = import(&conn, "not a document").unwrap_err();
        assert!(garbage.contains("not a sync document"), "{garbage}");

        assert_eq!(export(&conn).unwrap().text, before, "the library is untouched");
    }

    #[test]
    fn imported_marks_apply_to_entries_that_already_exist() {
        let source = seeded_library();
        let text = export(&source).unwrap().text;

        // The same feed and entry, fetched independently and not yet starred.
        let target = db::open_memory().unwrap();
        let feed = db::insert_or_update_feed(&target, "https://a.example/feed", "A", None).unwrap();
        db::upsert_entry(
            &target,
            feed,
            "g1",
            "Headline",
            None,
            "<p>body text</p>",
            "body text",
            None,
            Some(&stamp_days_ago(0)),
        )
        .unwrap();

        let summary = import(&target, &text).unwrap();
        assert_eq!(summary.stars_applied, 1);
        assert_eq!(summary.read_marks_applied, 1);
        assert_eq!(summary.marks_recorded_for_later, 0);

        let id = entry_id(&target, "g1");
        let (read, starred): (i64, i64) = target
            .query_row(
                "SELECT is_read, is_starred FROM entries WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((read, starred), (1, 1));
    }

    #[test]
    fn an_imported_mark_lands_when_the_feed_is_fetched() {
        let conn = db::open_memory().unwrap();
        let url = "https://a.example/feed";
        let document = Document {
            doc_version: DOC_VERSION,
            read_horizon_day: days_before(7),
            folders: vec![],
            feeds: vec![FeedRecord {
                url: url.into(),
                custom_title: None,
                title_ctr: 0,
                folder: None,
                folder_ctr: 0,
                sync_ctr: 1,
            }],
            marks: vec![MarkRecord {
                feed: url.into(),
                guid: "g1".into(),
                day: stamp_days_ago(0)[..10].to_string(),
                read: Some(true),
                read_ctr: 3,
                star: Some(true),
                star_ctr: 4,
            }],
            tombstones: vec![],
            settings: None,
        };
        let summary = import(&conn, &serde_json::to_string(&document).unwrap()).unwrap();
        assert_eq!(summary.marks_recorded_for_later, 1);

        // The feed is fetched for the first time and delivers the entry.
        let parsed = crate::fetch::ParsedFeed {
            title: "A".into(),
            site_url: None,
            entries: vec![crate::fetch::ParsedEntry {
                guid: "g1".into(),
                title: "Headline".into(),
                link: None,
                content_html: "<p>x</p>".into(),
                excerpt: "x".into(),
                image_url: None,
                published_at: Some(stamp_days_ago(0)),
            }],
        };
        crate::fetch::store_feed(&conn, url, &parsed).unwrap();

        let id = entry_id(&conn, "g1");
        let (read, starred): (i64, i64) = conn
            .query_row(
                "SELECT is_read, is_starred FROM entries WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((read, starred), (1, 1), "the waiting mark is applied on arrival");
    }

    // --- Two installations ----------------------------------------------

    #[test]
    fn two_installations_converge_and_each_keeps_its_own_field() {
        let a = db::open_memory().unwrap();
        let b = db::open_memory().unwrap();
        let url = "https://a.example/feed";

        let feed_a = db::insert_or_update_feed(&a, url, "A", None).unwrap();
        db::create_folder(&a, "Tech").unwrap();
        db::upsert_entry(
            &a,
            feed_a,
            "g1",
            "Headline",
            None,
            "<p>x</p>",
            "x",
            None,
            Some(&stamp_days_ago(0)),
        )
        .unwrap();
        let entry_a = entry_id(&a, "g1");
        db::set_starred(&a, entry_a, true).unwrap();
        db::set_read(&a, entry_a, true).unwrap();

        // B has its own copy of the same feed and entry.
        let feed_b = db::insert_or_update_feed(&b, url, "A", None).unwrap();
        db::upsert_entry(
            &b,
            feed_b,
            "g1",
            "Headline",
            None,
            "<p>x</p>",
            "x",
            None,
            Some(&stamp_days_ago(0)),
        )
        .unwrap();

        // First exchange: B learns the folder, the star, and the read state.
        import(&b, &export(&a).unwrap().text).unwrap();

        // Both sides now edit a different field of the same feed.
        let document_a = export(&a).unwrap().text;
        let folder_uuid = parse(&document_a).folders[0].uuid.clone();
        db::set_feed_title(&a, feed_a, Some("Mine")).unwrap();
        let folder_b = folder_id_for_uuid(&b, &folder_uuid);
        db::move_feed_to_folder(&b, feed_b, Some(folder_b)).unwrap();

        // Second exchange: merge what each side has.
        let merged = merge(
            &parse(&export(&a).unwrap().text),
            &parse(&export(&b).unwrap().text),
        );
        let text = serde_json::to_string(&merged).unwrap();
        import(&a, &text).unwrap();
        import(&b, &text).unwrap();

        for (conn, feed) in [(&a, feed_a), (&b, feed_b)] {
            let listed = db::list_feeds(conn).unwrap();
            assert_eq!(listed.len(), 1);
            let renamed = listed.iter().find(|f| f.id == feed).unwrap();
            assert_eq!(renamed.title, "Mine", "the rename reached both sides");
            assert_eq!(
                renamed.folder_id,
                Some(folder_id_for_uuid(conn, &folder_uuid)),
                "the folder assignment reached both sides"
            );
            assert!(
                db::get_entry(conn, entry_id(conn, "g1"))
                    .unwrap()
                    .unwrap()
                    .is_starred,
                "the star reached both sides"
            );
            assert_eq!(
                db::count_entries(conn, "unread", EntryScope::All).unwrap(),
                0,
                "the read mark reached both sides"
            );
        }
    }

    #[test]
    fn two_installations_creating_a_same_named_folder_keep_both() {
        let a = db::open_memory().unwrap();
        let b = db::open_memory().unwrap();
        db::create_folder(&a, "Tech").unwrap();
        db::create_folder(&b, "Tech").unwrap();

        let merged = merge(
            &parse(&export(&a).unwrap().text),
            &parse(&export(&b).unwrap().text),
        );
        assert_eq!(
            merged.folders.len(),
            2,
            "matching by name would silently merge two real folders"
        );

        import(&a, &serde_json::to_string(&merged).unwrap()).unwrap();
        assert_eq!(db::list_folders(&a).unwrap().len(), 2);
    }

    #[test]
    fn a_removal_on_one_installation_reaches_the_other() {
        let a = db::open_memory().unwrap();
        let b = db::open_memory().unwrap();
        let feed = db::insert_or_update_feed(&a, "https://a.example/feed", "A", None).unwrap();
        import(&b, &export(&a).unwrap().text).unwrap();
        assert_eq!(db::list_feeds(&b).unwrap().len(), 1);

        db::delete_feed(&a, feed).unwrap();
        let summary = import(&b, &export(&a).unwrap().text).unwrap();

        assert_eq!(summary.feeds_removed, 1);
        assert!(db::list_feeds(&b).unwrap().is_empty());
    }
}
