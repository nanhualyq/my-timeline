//! The sync document: what a reader's shared library state looks like outside
//! this installation, and how two copies of it merge back into one.
//!
//! Everything here except `project` and `apply` is a pure function over the
//! document. That is deliberate: order-independence and idempotence are the
//! properties the format exists for, and they are only testable while the merge
//! is separate from the database. A later change puts the same document on a
//! network transport without touching any of this.

use crate::db;
use crate::webdav;
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
    let text = render(&document)?;
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

// --- Keeping the document on a WebDAV collection -------------------------
//
// The cycle is: read (conditionally), merge into the library, render the merged
// state canonically, and write it back (conditionally). A rejected write means
// another installation wrote in between, so the cycle re-reads and re-merges
// instead of overwriting. Everything here is one function deep because a
// trigger must never be able to observe a half-finished sync.

/// Attempts at reading and writing before giving up. A rejection means someone
/// else wrote first; retrying converges, and three consecutive rejections mean
/// the two installations are fighting rather than racing.
pub const SYNC_ATTEMPTS: usize = 3;

/// How often a focus-driven sync may run. Kept in the backend: a frontend timer
/// would reset on every reload, which is exactly when it matters.
pub const FOCUS_SYNC_THROTTLE_SECS: i64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOutcome {
    /// Remote changes were applied and the merged document was written back.
    Synced,
    /// The document did not exist and this installation created it.
    Created,
    /// There was nothing to download, nothing to send, or both.
    NothingChanged,
    /// The cycle did not complete; `detail` says why.
    Failed,
}

/// What the reader is told about the last sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// RFC3339, so the dialog can say when rather than only what.
    pub ran_at: String,
    /// What happened to the document on the server. Deliberately separate from
    /// `applied`: a download-only sync leaves the document alone while changing
    /// the local library, and a local-only upload is the reverse.
    pub outcome: SyncOutcome,
    pub detail: String,
    /// Whether this sync changed the local library, so the view must refresh.
    /// Absent from statuses recorded by earlier builds, which then read as
    /// "nothing applied" and fall back to their `outcome`.
    #[serde(default)]
    pub applied: bool,
}

impl SyncStatus {
    /// A status for a cycle that applied nothing locally: every path before the
    /// merge, because nothing reached the library yet.
    fn now(outcome: SyncOutcome, detail: impl Into<String>) -> SyncStatus {
        SyncStatus::with_applied(outcome, detail, false)
    }

    /// A status from a point where the merge has already run, so the caller
    /// knows whether the library changed.
    fn with_applied(
        outcome: SyncOutcome,
        detail: impl Into<String>,
        applied: bool,
    ) -> SyncStatus {
        SyncStatus {
            ran_at: chrono::Utc::now().to_rfc3339(),
            outcome,
            detail: detail.into(),
            applied,
        }
    }
}

/// One canonical rendering. Export and the "is anything different" comparison
/// both go through here, so they cannot drift into two formats.
pub fn render(document: &Document) -> Result<String, String> {
    serde_json::to_string_pretty(document).map_err(|e| e.to_string())
}

/// A one-line account of what an import applied.
pub fn describe_summary(summary: &ImportSummary) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut push = |count: i64, noun: &str| {
        if count > 0 {
            parts.push(format!("{count} {noun}"));
        }
    };
    push(summary.feeds_added, "feed(s) added");
    push(summary.feeds_changed, "feed(s) updated");
    push(summary.feeds_removed, "feed(s) removed");
    push(summary.folders_added, "folder(s) added");
    push(summary.folders_changed, "folder(s) renamed");
    push(summary.folders_removed, "folder(s) removed");
    push(summary.stars_applied, "star(s)");
    push(summary.read_marks_applied, "read mark(s)");
    push(summary.marks_recorded_for_later, "mark(s) waiting for their feed");
    if parts.is_empty() {
        "no changes to apply".to_string()
    } else {
        parts.join(", ")
    }
}

/// Owns the "only one sync at a time" rule. Held by the application, so a
/// startup sync and a focus event cannot interleave.
pub struct SyncEngine {
    running: std::sync::atomic::AtomicBool,
}

impl Default for SyncEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncEngine {
    pub fn new() -> SyncEngine {
        SyncEngine {
            running: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Run a sync, or explain why none ran.
    ///
    /// A connection that is not configured is an error rather than a status:
    /// nothing was attempted, and the reader needs to fill the form in. Every
    /// other outcome — including failure — is a status, because a status is
    /// what the reader is shown afterwards.
    pub async fn sync(
        self: &std::sync::Arc<Self>,
        conn: &crate::reader::SharedConn,
        forced: bool,
    ) -> Result<SyncStatus, String> {
        let config = {
            let guard = conn.lock().map_err(|e| e.to_string())?;
            db::load_sync_config(&guard).map_err(|e| e.to_string())?
        };
        let Some(config) = config.filter(db::SyncConfig::is_configured) else {
            return Err("the WebDAV connection is not configured".into());
        };
        let target = webdav::Target::from_config(&config)?;

        if !self.enter() {
            // Dropped rather than queued: the running cycle will report the
            // outcome, and a queue of focus events would only repeat it.
            return Ok(self.previous(conn, &target)?.unwrap_or_else(|| {
                SyncStatus::now(SyncOutcome::NothingChanged, "a sync is already running")
            }));
        }
        let result = self.run_once(conn, &target, forced).await;
        self.leave();
        result
    }

    async fn run_once(
        &self,
        conn: &crate::reader::SharedConn,
        target: &webdav::Target,
        forced: bool,
    ) -> Result<SyncStatus, String> {
        if !forced {
            if let Some(previous) = self.previous(conn, target)? {
                if ran_within(&previous.ran_at, FOCUS_SYNC_THROTTLE_SECS) {
                    return Ok(previous);
                }
            }
        }
        let status = cycle(conn, target).await;
        self.record(conn, target, &status)?;
        Ok(status)
    }

    fn enter(&self) -> bool {
        self.running
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            )
            .is_ok()
    }

    fn leave(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Release);
    }

    fn previous(
        &self,
        conn: &crate::reader::SharedConn,
        target: &webdav::Target,
    ) -> Result<Option<SyncStatus>, String> {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        let stored = db::remote_status_json(&guard, &target.document_url)
            .map_err(|e| e.to_string())?;
        Ok(stored.and_then(|json| serde_json::from_str(&json).ok()))
    }

    fn record(
        &self,
        conn: &crate::reader::SharedConn,
        target: &webdav::Target,
        status: &SyncStatus,
    ) -> Result<(), String> {
        let json = serde_json::to_string(status).map_err(|e| e.to_string())?;
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::set_remote_status_json(&guard, &target.document_url, &json)
            .map_err(|e| e.to_string())
    }
}

fn ran_within(timestamp: &str, seconds: i64) -> bool {
    match chrono::DateTime::parse_from_rfc3339(timestamp) {
        Ok(at) => {
            let elapsed = chrono::Utc::now().signed_duration_since(at.with_timezone(&chrono::Utc));
            elapsed.num_seconds() < seconds
        }
        // An unreadable timestamp must not pin the throttle open.
        Err(_) => false,
    }
}

/// One read / merge / write cycle. Never panics on a transport problem: every
/// failure comes back as a `Failed` status carrying the reason.
///
/// A `304` means the *server* has nothing new — it does not mean the cycle is
/// over, because the local library may have changes the server has never seen.
/// The two sides are judged separately.
async fn cycle(conn: &crate::reader::SharedConn, target: &webdav::Target) -> SyncStatus {
    let key = target.document_url.as_str();
    let (mut known_etag, uploaded) = {
        let guard = match conn.lock() {
            Ok(guard) => guard,
            Err(e) => return SyncStatus::now(SyncOutcome::Failed, e.to_string()),
        };
        let etag = match db::remote_etag(&guard, key) {
            Ok(etag) => etag,
            Err(e) => return SyncStatus::now(SyncOutcome::Failed, e.to_string()),
        };
        let uploaded = match db::remote_uploaded_text(&guard, key) {
            Ok(uploaded) => uploaded,
            Err(e) => return SyncStatus::now(SyncOutcome::Failed, e.to_string()),
        };
        (etag, uploaded)
    };

    let mut attempt = 0;
    // Carried across attempts: a rejected write re-reads and re-merges, and by
    // then the first attempt's import reports no changes of its own, so the
    // library change would otherwise be forgotten.
    let mut applied = false;
    loop {
        attempt += 1;
        let fetched = match webdav::get_document(target, known_etag.as_deref()).await {
            Ok(fetched) => fetched,
            Err(e) => return SyncStatus::now(SyncOutcome::Failed, e),
        };

        let (remote, etag, creating) = match fetched {
            // Nothing to download; the library still gets compared below.
            webdav::GetOutcome::NotModified => (None, known_etag.clone(), false),
            webdav::GetOutcome::Missing => (None, None, true),
            webdav::GetOutcome::Fetched { text, etag } => (Some(text), etag, false),
        };

        // Merge into the library, then render what the library now holds: the
        // canonical form of the union is what gets uploaded.
        let (merged, summary) = {
            let guard = match conn.lock() {
                Ok(guard) => guard,
                Err(e) => return SyncStatus::with_applied(SyncOutcome::Failed, e.to_string(), applied),
            };
            let summary = match &remote {
                Some(text) => match import(&guard, text) {
                    Ok(summary) => summary,
                    Err(e) => return SyncStatus::with_applied(SyncOutcome::Failed, e, applied),
                },
                None => ImportSummary::default(),
            };
            let document = match project(&guard) {
                Ok(document) => document,
                Err(e) => return SyncStatus::with_applied(SyncOutcome::Failed, e, applied),
            };
            match render(&document) {
                Ok(rendered) => (rendered, summary),
                Err(e) => return SyncStatus::with_applied(SyncOutcome::Failed, e, applied),
            }
        };
        applied |= !nothing_applied(&summary);

        // The server already holding this exact text is the same "nothing to
        // send" as having uploaded it a minute ago, so compare against both the
        // version just read and the one last written.
        let unchanged = Some(merged.as_str()) == remote.as_deref()
            || Some(merged.as_str()) == uploaded.as_deref();

        if unchanged {
            if let Err(e) = store_remote_state(conn, key, etag.as_deref(), &merged) {
                return SyncStatus::with_applied(SyncOutcome::Failed, e, applied);
            }
            // The document is untouched, but the library may not be: this is
            // the download-only case, and the caller must still refresh.
            return SyncStatus::with_applied(
                SyncOutcome::NothingChanged,
                detail_for(&summary, remote.is_some(), false),
                applied,
            );
        }

        let write = if creating {
            webdav::put_document(target, &merged, None).await
        } else {
            webdav::put_document(target, &merged, etag.as_deref()).await
        };

        match write {
            Ok(webdav::PutOutcome::Written { etag: new_etag }) => {
                if let Err(e) = store_remote_state(conn, key, new_etag.as_deref(), &merged) {
                    return SyncStatus::with_applied(SyncOutcome::Failed, e, applied);
                }
                let detail = detail_for(&summary, remote.is_some(), true);
                return if creating {
                    SyncStatus::with_applied(
                        SyncOutcome::Created,
                        format!("created the document; {detail}"),
                        applied,
                    )
                } else {
                    SyncStatus::with_applied(SyncOutcome::Synced, detail, applied)
                };
            }
            Ok(webdav::PutOutcome::PreconditionFailed) => {
                if attempt >= SYNC_ATTEMPTS {
                    return SyncStatus::with_applied(
                        SyncOutcome::Failed,
                        format!(
                            "the document kept changing on the server; gave up after {SYNC_ATTEMPTS} attempts"
                        ),
                        applied,
                    );
                }
                // Re-read from the version just read, so the next read returns
                // the body rather than another 304.
                known_etag = etag;
            }
            Err(e) => return SyncStatus::with_applied(SyncOutcome::Failed, e, applied),
        }
    }
}

/// Whether an import changed anything.
fn nothing_applied(summary: &ImportSummary) -> bool {
    summary.feeds_added == 0
        && summary.feeds_changed == 0
        && summary.feeds_removed == 0
        && summary.folders_added == 0
        && summary.folders_changed == 0
        && summary.folders_removed == 0
        && summary.stars_applied == 0
        && summary.read_marks_applied == 0
        && summary.marks_recorded_for_later == 0
}

/// What to say about a cycle: what came down, and whether anything went up.
fn detail_for(summary: &ImportSummary, downloaded: bool, uploaded: bool) -> String {
    match (nothing_applied(summary), downloaded, uploaded) {
        (true, _, false) => "no changes on either side".to_string(),
        (true, false, true) => "uploaded local changes".to_string(),
        (true, true, true) => "uploaded the merged document".to_string(),
        (false, _, true) => describe_summary(summary),
        (false, true, false) => describe_summary(summary),
        (false, false, false) => describe_summary(summary),
    }
}

fn store_remote_state(
    conn: &crate::reader::SharedConn,
    key: &str,
    etag: Option<&str>,
    text: &str,
) -> Result<(), String> {
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::set_remote_etag(&guard, key, etag).map_err(|e| e.to_string())?;
    db::set_remote_uploaded_text(&guard, key, Some(text)).map_err(|e| e.to_string())
}

/// The last sync's status for the configured connection, for the dialog to show
/// after a sync that ran before it was opened.
pub fn last_status(
    conn: &Connection,
    config: &db::SyncConfig,
) -> Result<Option<SyncStatus>, String> {
    if !config.is_configured() {
        return Ok(None);
    }
    let url = webdav::document_url(&config.url, &config.path)?;
    let stored = db::remote_status_json(conn, &url).map_err(|e| e.to_string())?;
    Ok(stored.and_then(|json| serde_json::from_str(&json).ok()))
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
    // --- Canonical rendering ---------------------------------------------

    #[test]
    fn a_library_renders_the_same_text_every_time() {
        let conn = seeded_library();
        let first = export(&conn).unwrap().text;
        let second = export(&conn).unwrap().text;
        assert_eq!(first, second);
    }

    #[test]
    fn exporting_and_merging_the_same_state_render_identical_text() {
        // Marks created in an order that differs from the order they render in:
        // the point is that the two renderers agree on one canonical order.
        let conn = db::open_memory().unwrap();
        for (i, url) in ["https://c.example/feed", "https://a.example/feed", "https://b.example/feed"]
            .iter()
            .enumerate()
        {
            let feed = db::insert_or_update_feed(&conn, url, "F", None).unwrap();
            db::upsert_entry(
                &conn,
                feed,
                &format!("g{i}"),
                "T",
                None,
                "<p>x</p>",
                "x",
                None,
                Some(&stamp_days_ago(0)),
            )
            .unwrap();
            let id = entry_id(&conn, &format!("g{i}"));
            db::set_starred(&conn, id, true).unwrap();
            db::set_read(&conn, id, true).unwrap();
        }

        let document = project(&conn).unwrap();
        let merged = merge(&document, &document);

        assert_eq!(render(&merged).unwrap(), render(&document).unwrap());
    }

    // --- The sync cycle ---------------------------------------------------

    use crate::reader::SharedConn;
    use crate::webdav::stub::Stub;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex};

    fn shared(conn: Connection) -> SharedConn {
        Arc::new(Mutex::new(conn))
    }

    fn configure(conn: &SharedConn, stub: &Stub) {
        let guard = conn.lock().unwrap();
        db::save_sync_config(
            &guard,
            &db::SyncConfig {
                url: stub.base_url.clone(),
                path: "state.json".into(),
                username: "me".into(),
                password: "secret".into(),
                accept_invalid_certs: false,
            },
        )
        .unwrap();
    }

    fn stored_config(conn: &SharedConn) -> db::SyncConfig {
        let guard = conn.lock().unwrap();
        db::load_sync_config(&guard).unwrap().unwrap()
    }

    /// A document as another installation would have written it.
    fn document_from(urls: &[&str]) -> String {
        let other = db::open_memory().unwrap();
        for url in urls {
            db::insert_or_update_feed(&other, url, "F", None).unwrap();
        }
        export(&other).unwrap().text
    }

    fn local_urls(conn: &SharedConn) -> Vec<String> {
        let guard = conn.lock().unwrap();
        db::list_feeds(&guard)
            .unwrap()
            .into_iter()
            .map(|f| f.url)
            .collect()
    }

    #[tokio::test]
    async fn an_unconfigured_connection_attempts_nothing() {
        let conn = shared(seeded_library());
        let engine = Arc::new(SyncEngine::new());

        let error = engine.sync(&conn, true).await.unwrap_err();

        assert!(error.contains("not configured"), "{error}");
    }

    #[tokio::test]
    async fn a_missing_document_is_created() {
        let stub = Stub::start();
        let conn = shared(seeded_library());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());

        let status = engine.sync(&conn, true).await.unwrap();

        assert_eq!(status.outcome, SyncOutcome::Created);
        let body = stub.body().expect("the document now exists");
        assert!(body.contains("a.example"), "{body}");
        // Created without any version to replace.
        assert_eq!(stub.requests().last().unwrap().method, "PUT");
        assert_eq!(stub.requests().last().unwrap().if_match, None);
    }

    #[tokio::test]
    async fn a_second_sync_downloads_nothing_and_uploads_nothing() {
        let stub = Stub::start();
        let conn = shared(seeded_library());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());
        engine.sync(&conn, true).await.unwrap();
        let puts = stub.put_count();
        let requests = stub.requests().len();

        let status = engine.sync(&conn, true).await.unwrap();

        assert_eq!(status.outcome, SyncOutcome::NothingChanged);
        assert_eq!(stub.put_count(), puts, "an unchanged library is not uploaded");
        let after = stub.requests();
        assert_eq!(after.len(), requests + 1, "one conditional read, no write");
        assert!(
            after.last().unwrap().if_none_match.is_some(),
            "the read asked the server whether anything changed"
        );
    }

    #[tokio::test]
    async fn a_document_the_server_already_holds_is_not_uploaded_again() {
        let stub = Stub::start();
        let conn = shared(seeded_library());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());
        engine.sync(&conn, true).await.unwrap();
        let puts = stub.put_count();

        // The server's version tag moved on without the content changing, as a
        // re-upload or a different server would leave it.
        {
            let guard = conn.lock().unwrap();
            db::set_remote_etag(&guard, &stub.document_url(), Some("\"older\"")).unwrap();
        }
        let status = engine.sync(&conn, true).await.unwrap();

        assert_eq!(status.outcome, SyncOutcome::NothingChanged);
        assert_eq!(stub.put_count(), puts, "nothing needed sending");
    }

    /// The shape that used to go unreported: the document on the server already
    /// holds the merged state, so nothing is uploaded, while the library did
    /// change. The outcome alone would say "nothing happened".
    #[tokio::test]
    async fn a_download_only_sync_reports_the_library_change() {
        let stub = Stub::start();
        let conn = shared(db::open_memory().unwrap());
        let url = "https://a.example/feed";

        // The other installation knows this entry and has starred it.
        let other = db::open_memory().unwrap();
        let other_feed = db::insert_or_update_feed(&other, url, "A", None).unwrap();
        db::upsert_entry(
            &other,
            other_feed,
            "g1",
            "Headline",
            None,
            "<p>x</p>",
            "x",
            None,
            Some(&stamp_days_ago(0)),
        )
        .unwrap();
        db::set_starred(&other, entry_id(&other, "g1"), true).unwrap();
        stub.set_body(&export(&other).unwrap().text);
        configure(&conn, &stub);

        // This installation has the entry, but not the star.
        {
            let guard = conn.lock().unwrap();
            let feed = db::insert_or_update_feed(&guard, url, "A", None).unwrap();
            db::upsert_entry(
                &guard,
                feed,
                "g1",
                "Headline",
                None,
                "<p>x</p>",
                "x",
                None,
                Some(&stamp_days_ago(0)),
            )
            .unwrap();
        }

        let engine = Arc::new(SyncEngine::new());
        let status = engine.sync(&conn, true).await.unwrap();

        assert_eq!(
            status.outcome,
            SyncOutcome::NothingChanged,
            "the document on the server was already the merged state"
        );
        assert!(status.applied, "the library changed, so the view must refresh");
        assert!(status.detail.contains("1 star(s)"), "{}", status.detail);
        assert_eq!(stub.put_count(), 0, "nothing needed sending");
        assert_eq!(stub.requests().len(), 1, "one read, no write");

        let guard = conn.lock().unwrap();
        let starred: i64 = guard
            .query_row(
                "SELECT is_starred FROM entries WHERE id = ?1",
                params![entry_id(&guard, "g1")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(starred, 1, "the star landed in the library");
    }

    /// A rejected write re-reads and re-merges, and the second import reports no
    /// changes of its own. The library change from the first attempt must not be
    /// forgotten.
    #[tokio::test]
    async fn a_conflict_retry_still_reports_the_library_change() {
        let stub = Stub::start();
        stub.set_body(&document_from(&["https://b.example/feed"]));
        stub.always_conflict();
        let conn = shared(db::open_memory().unwrap());
        // A local subscription the server has never seen forces a write attempt
        // on every attempt, so the rejections keep coming.
        {
            let guard = conn.lock().unwrap();
            db::insert_or_update_feed(&guard, "https://c.example/feed", "C", None).unwrap();
        }
        configure(&conn, &stub);

        let engine = Arc::new(SyncEngine::new());
        let status = engine.sync(&conn, true).await.unwrap();

        assert_eq!(status.outcome, SyncOutcome::Failed);
        assert!(status.detail.contains("gave up"), "{}", status.detail);
        assert!(
            status.applied,
            "the first attempt had already merged the remote subscription in"
        );
        assert!(local_urls(&conn).contains(&"https://b.example/feed".to_string()));
    }

    /// Uploading is not applying: the library already held what was sent, so
    /// there is nothing to refresh.
    #[tokio::test]
    async fn uploading_local_changes_reports_no_library_change() {
        let stub = Stub::start();
        let conn = shared(seeded_library());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());

        let created = engine.sync(&conn, true).await.unwrap();
        assert_eq!(created.outcome, SyncOutcome::Created);
        assert!(!created.applied, "creating the document applied nothing locally");

        {
            let guard = conn.lock().unwrap();
            db::insert_or_update_feed(&guard, "https://d.example/feed", "D", None).unwrap();
        }
        let synced = engine.sync(&conn, true).await.unwrap();

        assert_eq!(synced.outcome, SyncOutcome::Synced);
        assert!(!synced.applied, "the library already held the uploaded change");
    }

    #[tokio::test]
    async fn a_replayed_status_keeps_the_recorded_library_change() {
        let stub = Stub::start();
        stub.set_body(&document_from(&["https://b.example/feed"]));
        let conn = shared(db::open_memory().unwrap());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());

        let first = engine.sync(&conn, true).await.unwrap();
        assert!(first.applied);

        let throttled = engine.sync(&conn, false).await.unwrap();
        assert_eq!(throttled.ran_at, first.ran_at, "the throttle replayed the status");
        assert!(throttled.applied, "a replay keeps the recorded fact");

        // A trigger that arrives during a cycle reports that cycle's status.
        engine.running.store(true, Ordering::Release);
        let dropped = engine.sync(&conn, true).await.unwrap();
        engine.running.store(false, Ordering::Release);
        assert_eq!(dropped.ran_at, first.ran_at);
        assert!(dropped.applied);
    }

    #[test]
    fn a_status_recorded_before_the_field_existed_reads_as_unapplied() {
        let old = r#"{"ran_at":"2026-11-28T10:00:00+00:00","outcome":"synced","detail":"uploaded local changes"}"#;
        let parsed: SyncStatus = serde_json::from_str(old).unwrap();
        assert!(!parsed.applied, "an older status makes no claim about the library");

        let fresh = SyncStatus::with_applied(
            SyncOutcome::NothingChanged,
            "1 feed(s) added",
            true,
        );
        let text = serde_json::to_string(&fresh).unwrap();
        assert!(text.contains("\"applied\":true"), "{text}");
        assert!(serde_json::from_str::<SyncStatus>(&text).unwrap().applied);
    }

    #[tokio::test]
    async fn a_write_that_loses_the_race_is_re_read_and_merged() {
        let stub = Stub::start();
        let conn = shared(seeded_library());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());
        engine.sync(&conn, true).await.unwrap();

        // Another installation replaces the document between our read and write,
        // and the local library gains a subscription of its own. The stored
        // version is deliberately stale: that is what a write by someone else
        // looks like from here.
        {
            let guard = conn.lock().unwrap();
            db::set_remote_etag(&guard, &stub.document_url(), Some("\"older\"")).unwrap();
        }
        stub.interfere_on_next_put(&document_from(&["https://b.example/feed"]));
        {
            let guard = conn.lock().unwrap();
            db::insert_or_update_feed(&guard, "https://c.example/feed", "C", None).unwrap();
        }

        let status = engine.sync(&conn, true).await.unwrap();

        assert_eq!(status.outcome, SyncOutcome::Synced);
        let body = stub.body().unwrap();
        assert!(body.contains("b.example"), "the other installation's feed survived: {body}");
        assert!(body.contains("c.example"), "the local feed reached the server: {body}");
        assert!(body.contains("a.example"), "the original subscription survived: {body}");
        let local = local_urls(&conn);
        assert!(local.contains(&"https://b.example/feed".to_string()));
        assert!(local.contains(&"https://c.example/feed".to_string()));
    }

    #[tokio::test]
    async fn an_exhausted_retry_budget_reports_failure_and_keeps_the_library() {
        let stub = Stub::start();
        let conn = shared(seeded_library());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());
        engine.sync(&conn, true).await.unwrap();
        let before = stub.body();
        let families = local_urls(&conn).len();

        // Every write is rejected: the two installations are fighting.
        stub.always_conflict();
        {
            let guard = conn.lock().unwrap();
            db::insert_or_update_feed(&guard, "https://c.example/feed", "C", None).unwrap();
        }

        let status = engine.sync(&conn, true).await.unwrap();

        assert_eq!(status.outcome, SyncOutcome::Failed);
        assert!(status.detail.contains("gave up"), "{}", status.detail);
        assert_eq!(stub.put_count() > 3, true, "it kept trying before giving up");
        assert_eq!(stub.body(), before, "the server document was not overwritten");
        assert!(
            local_urls(&conn).len() > families,
            "nothing local was lost"
        );
    }

    #[tokio::test]
    async fn a_failed_sync_is_reported_afterwards() {
        let stub = Stub::start();
        stub.force_status(500);
        let conn = shared(seeded_library());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());

        let status = engine.sync(&conn, true).await.unwrap();
        assert_eq!(status.outcome, SyncOutcome::Failed);

        let config = stored_config(&conn);
        let guard = conn.lock().unwrap();
        let stored = last_status(&guard, &config).unwrap().expect("the failure is kept");
        assert_eq!(stored.outcome, SyncOutcome::Failed);
        assert!(stored.detail.contains("500"), "{}", stored.detail);
    }

    #[tokio::test]
    async fn a_trigger_during_a_running_sync_is_dropped() {
        let stub = Stub::start();
        let conn = shared(seeded_library());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());

        // Pretend a cycle is in flight.
        engine.running.store(true, Ordering::Release);
        let dropped = engine.sync(&conn, true).await.unwrap();
        assert_eq!(stub.requests().len(), 0, "no request was made");
        assert!(dropped.detail.contains("already running"), "{}", dropped.detail);

        engine.running.store(false, Ordering::Release);
        let real = engine.sync(&conn, true).await.unwrap();
        assert_eq!(real.outcome, SyncOutcome::Created, "the engine recovered");
    }

    #[tokio::test]
    async fn a_focus_trigger_is_throttled_and_a_forced_one_is_not() {
        let stub = Stub::start();
        let conn = shared(seeded_library());
        configure(&conn, &stub);
        let engine = Arc::new(SyncEngine::new());
        engine.sync(&conn, true).await.unwrap();
        let requests = stub.requests().len();

        let throttled = engine.sync(&conn, false).await.unwrap();
        assert_eq!(stub.requests().len(), requests, "the throttle skipped the network");
        assert_eq!(throttled.outcome, SyncOutcome::Created, "the previous status is reported");

        let forced = engine.sync(&conn, true).await.unwrap();
        assert!(stub.requests().len() > requests, "a forced sync ignores the throttle");
        assert_eq!(forced.outcome, SyncOutcome::NothingChanged);
    }

    #[tokio::test]
    async fn syncing_reaches_both_installations() {
        // The end-to-end shape: each side stars or subscribes, both sync, and
        // both end up with the union while the server holds it.
        let stub = Stub::start();
        stub.set_body(&document_from(&["https://shared.example/feed"]));
        let a = shared(seeded_library());
        let b = shared(db::open_memory().unwrap());
        configure(&a, &stub);
        configure(&b, &stub);
        let engine = Arc::new(SyncEngine::new());

        // B subscribes to something the other side has never heard of.
        {
            let guard = b.lock().unwrap();
            db::insert_or_update_feed(&guard, "https://b-only.example/feed", "B", None).unwrap();
        }
        // A stars something.
        {
            let guard = a.lock().unwrap();
            db::insert_or_update_feed(&guard, "https://shared.example/feed", "S", None).unwrap();
            let feed = db::feed_id_by_url(&guard, "https://shared.example/feed")
                .unwrap()
                .unwrap();
            db::upsert_entry(
                &guard,
                feed,
                "g-shared",
                "T",
                None,
                "<p>x</p>",
                "x",
                None,
                Some(&stamp_days_ago(0)),
            )
            .unwrap();
            let id = entry_id(&guard, "g-shared");
            db::set_starred(&guard, id, true).unwrap();
        }

        engine.sync(&a, true).await.unwrap();
        // B fetches the shared feed itself, as it would on its own refresh.
        {
            let guard = b.lock().unwrap();
            let feed = db::insert_or_update_feed(&guard, "https://shared.example/feed", "S", None)
                .unwrap();
            db::upsert_entry(
                &guard,
                feed,
                "g-shared",
                "T",
                None,
                "<p>x</p>",
                "x",
                None,
                Some(&stamp_days_ago(0)),
            )
            .unwrap();
        }
        engine.sync(&b, true).await.unwrap();
        engine.sync(&a, true).await.unwrap();

        assert!(local_urls(&a).contains(&"https://b-only.example/feed".to_string()));
        assert!(local_urls(&b).contains(&"https://b-only.example/feed".to_string()));
        let guard = b.lock().unwrap();
        let starred: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM entries WHERE guid = 'g-shared' AND is_starred = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(starred, 1, "the star reached the other installation");
        let body = stub.body().unwrap();
        assert!(body.contains("b-only.example") && body.contains("shared.example"));
    }

    #[tokio::test]
    async fn an_unreachable_server_leaves_the_library_unchanged() {
        let conn = shared(seeded_library());
        {
            let guard = conn.lock().unwrap();
            db::save_sync_config(
                &guard,
                &db::SyncConfig {
                    url: "http://127.0.0.1:1/dav".into(),
                    path: "state.json".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let engine = Arc::new(SyncEngine::new());
        let before = {
            let guard = conn.lock().unwrap();
            export(&guard).unwrap().text
        };

        let status = engine.sync(&conn, true).await.unwrap();

        assert_eq!(status.outcome, SyncOutcome::Failed);
        assert!(!status.applied, "nothing reached the library");
        assert!(
            status.detail.contains("cannot reach"),
            "the reason names the cause: {}",
            status.detail
        );
        let after = {
            let guard = conn.lock().unwrap();
            export(&guard).unwrap().text
        };
        assert_eq!(after, before, "an unreachable server changes nothing locally");

        let config = stored_config(&conn);
        let guard = conn.lock().unwrap();
        let recorded = last_status(&guard, &config).unwrap().unwrap();
        assert_eq!(recorded.outcome, SyncOutcome::Failed);
    }

}
