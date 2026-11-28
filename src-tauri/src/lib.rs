mod db;
mod fetch;
mod opml;
mod reader;
mod rowdetail;
mod sync;
mod webdav;

use reader::{AddFeedResult, SharedConn};
use std::sync::{Arc, Mutex};
use tauri::Manager;

fn shared_conn(app: &tauri::AppHandle) -> Result<SharedConn, String> {
    Ok(app.state::<SharedConn>().inner().clone())
}

#[tauri::command]
fn list_feeds(app: tauri::AppHandle) -> Result<Vec<db::FeedDto>, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::list_feeds(&guard).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_entries(
    app: tauri::AppHandle,
    view: String,
    scope: db::EntryScope,
    after: Option<db::EntryCursor>,
    limit: i64,
) -> Result<Vec<db::EntryListItemDto>, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::list_entries(&guard, &view, scope, after.as_ref(), limit)
}

/// Entries matching a view and scope, for the list's progress total.
#[tauri::command]
fn count_entries(
    app: tauri::AppHandle,
    view: String,
    scope: db::EntryScope,
) -> Result<i64, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::count_entries(&guard, &view, scope)
}

/// One entry with its content, for the reading pane.
#[tauri::command]
fn get_entry(app: tauri::AppHandle, entry_id: i64) -> Result<Option<db::EntryDto>, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::get_entry(&guard, entry_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_folders(app: tauri::AppHandle) -> Result<Vec<db::FolderDto>, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::list_folders(&guard)
}

/// Total and starred entry counts for the two global sidebar badges.
#[tauri::command]
fn entry_counts(app: tauri::AppHandle) -> Result<db::EntryCountsDto, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::entry_counts(&guard).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_folder(app: tauri::AppHandle, name: String) -> Result<db::FolderDto, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::create_folder(&guard, &name)
}

#[tauri::command]
fn rename_folder(app: tauri::AppHandle, folder_id: i64, name: String) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::rename_folder(&guard, folder_id, &name)
}

#[tauri::command]
fn delete_folder(app: tauri::AppHandle, folder_id: i64) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::delete_folder(&guard, folder_id)
}

#[tauri::command]
fn move_feed_to_folder(
    app: tauri::AppHandle,
    feed_id: i64,
    folder_id: Option<i64>,
) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::move_feed_to_folder(&guard, feed_id, folder_id)
}

#[tauri::command]
fn set_starred(app: tauri::AppHandle, entry_id: i64, is_starred: bool) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::set_starred(&guard, entry_id, is_starred)
}

/// Import OPML text chosen by the frontend. Content-only: the platform's file
/// picker hands back a filesystem path on desktop and a document reference on
/// Android, and only the frontend knows how to turn either into text. Merge-only;
/// performs no fetching — entries only arrive through a later manual refresh.
#[tauri::command]
fn import_opml(app: tauri::AppHandle, xml: String) -> Result<opml::ImportOutcome, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    opml::import_opml(&guard, &xml)
}

/// Render all subscriptions (with folder grouping) as OPML text. The frontend
/// writes it wherever the platform's file picker pointed.
#[tauri::command]
fn export_opml(app: tauri::AppHandle) -> Result<opml::ExportOutcome, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    let exported = opml::export_opml(&guard)?;
    Ok(opml::ExportOutcome {
        xml: exported.xml,
        feeds_exported: exported.feeds_exported,
    })
}

/// Render the shared library state as sync-document text. The frontend writes it
/// wherever the platform's file picker pointed, exactly as it does for OPML:
/// only the frontend can turn a desktop path or an Android document reference
/// into a file.
#[tauri::command]
fn export_sync_document(app: tauri::AppHandle) -> Result<sync::ExportOutcome, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    export_sync_outcome(&guard)
}

/// Merge sync-document text chosen by the frontend into the local library.
/// Merge-only: it never removes anything the document does not name, and it
/// performs no fetching — entries arrive through a later refresh.
#[tauri::command]
fn import_sync_document(
    app: tauri::AppHandle,
    text: String,
) -> Result<sync::ImportSummary, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    import_sync_outcome(&guard, &text)
}

/// What both sync commands return, separated from the state lookup only so that
/// the values crossing the IPC boundary — including the text a document is
/// rendered to — can be exercised without an application handle. The commands
/// themselves are the three lines above.
fn export_sync_outcome(conn: &rusqlite::Connection) -> Result<sync::ExportOutcome, String> {
    sync::export(conn)
}

fn import_sync_outcome(
    conn: &rusqlite::Connection,
    text: &str,
) -> Result<sync::ImportSummary, String> {
    sync::import(conn, text)
}

#[tauri::command]
fn set_read(app: tauri::AppHandle, entry_id: i64, is_read: bool) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::set_read(&guard, entry_id, is_read)
}

/// Mark a batch of entries read. The ids come from the rows the list showed,
/// so this can never reach an entry the reader has not seen.
#[tauri::command]
fn mark_read_many(app: tauri::AppHandle, ids: Vec<i64>) -> Result<i64, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::mark_read_many(&guard, &ids)
}

/// Mark every unread entry of a scope read, whether or not it was listed.
#[tauri::command]
fn mark_scope_read(app: tauri::AppHandle, scope: db::EntryScope) -> Result<i64, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::mark_scope_read(&guard, scope)
}

#[tauri::command]
fn remove_feed(app: tauri::AppHandle, feed_id: i64) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::delete_feed(&guard, feed_id)
}

#[tauri::command]
async fn add_feed(app: tauri::AppHandle, url: String) -> Result<db::FeedDto, String> {
    let conn = shared_conn(&app)?;
    match reader::add_feed(conn, &url).await? {
        AddFeedResult::Added(f) | AddFeedResult::Existing(f) => Ok(f),
    }
}

#[tauri::command]
async fn refresh_feed(app: tauri::AppHandle, feed_id: i64) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    reader::refresh_feed(conn, feed_id).await
}

/// Refresh every feed under a folder. Per-feed outcomes are included so the
/// frontend can summarize successes and failures in one alert.
#[tauri::command]
async fn refresh_folder(
    app: tauri::AppHandle,
    folder_id: i64,
) -> Result<Vec<reader::RefreshOutcome>, String> {
    let conn = shared_conn(&app)?;
    let results = reader::refresh_folder(conn, folder_id).await?;
    Ok(results
        .into_iter()
        .map(|(feed_id, result)| reader::RefreshOutcome {
            feed_id,
            error: result.err(),
        })
        .collect())
}

/// Set or clear a feed's custom display title (`title: null` reverts to the
/// feed-provided title).
#[tauri::command]
fn set_feed_title(
    app: tauri::AppHandle,
    feed_id: i64,
    title: Option<String>,
) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::set_feed_title(&guard, feed_id, title.as_deref())
}

/// Refresh all feeds. Never fails as a whole; per-feed outcomes are included
/// and each feed's error is already recorded in the database.
#[tauri::command]
async fn refresh_all(app: tauri::AppHandle) -> Result<Vec<reader::RefreshOutcome>, String> {
    let conn = shared_conn(&app)?;
    let results = reader::refresh_all(conn).await;
    Ok(results
        .into_iter()
        .map(|(feed_id, result)| reader::RefreshOutcome {
            feed_id,
            error: result.err(),
        })
        .collect())
}

/// The saved WebDAV connection, or `None` when the form has never been filled
/// in. Separate from the sync commands so the dialog can render the fields
/// without touching the network.
#[tauri::command]
fn load_sync_config(app: tauri::AppHandle) -> Result<Option<db::SyncConfig>, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::load_sync_config(&guard).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_sync_config(app: tauri::AppHandle, config: db::SyncConfig) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::save_sync_config(&guard, &config).map_err(|e| e.to_string())
}

/// Check the saved connection without changing anything, so a wrong URL, path,
/// or password is reported at configuration time rather than at the next sync.
#[tauri::command]
async fn test_sync_connection(app: tauri::AppHandle) -> Result<webdav::ConnectionTest, String> {
    let config = {
        let conn = shared_conn(&app)?;
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::load_sync_config(&guard).map_err(|e| e.to_string())?
    };
    let Some(config) = config.filter(db::SyncConfig::is_configured) else {
        return Err("the WebDAV connection is not configured".into());
    };
    let target = webdav::Target::from_config(&config)?;
    webdav::test_connection(&target).await
}

/// Run one sync cycle. `force` bypasses the focus throttle; the "only one at a
/// time" rule is never bypassed.
///
/// A sync that fails to reach the server returns a `Failed` status rather than
/// an error: the reader is shown a status either way, and the status is what
/// gets recorded for later. An error means nothing was attempted at all.
#[tauri::command]
async fn sync_now(app: tauri::AppHandle, force: bool) -> Result<sync::SyncStatus, String> {
    let engine = app
        .state::<std::sync::Arc<sync::SyncEngine>>()
        .inner()
        .clone();
    let conn = shared_conn(&app)?;
    engine.sync(&conn, force).await
}

/// The last sync's outcome, for a dialog opened after a sync that ran on focus
/// or at startup.
#[tauri::command]
fn sync_status(app: tauri::AppHandle) -> Result<Option<sync::SyncStatus>, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    let config = db::load_sync_config(&guard).map_err(|e| e.to_string())?;
    match config {
        Some(config) => sync::last_status(&guard, &config),
        None => Ok(None),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app data dir: {e}"))?;
            // Debug builds must not share the production database: the data dir
            // is derived from the bundle identifier, which `tauri dev` does not
            // override, so dev and release would otherwise open the same file.
            //
            // Desktop only. On Android the data dir is the app's own
            // per-package directory, so a debug and a release install of the
            // same package can never coexist; asking for a sibling directory
            // (…-dev) lands outside the app's sandbox, where creating it fails
            // and the database cannot be opened at all.
            let desktop_debug =
                cfg!(debug_assertions) && !cfg!(any(target_os = "android", target_os = "ios"));
            let data_dir = if desktop_debug {
                data_dir.with_file_name("com.mytimeline.app-dev")
            } else {
                data_dir
            };
            let conn = db::open(&data_dir.join("reader.db")).map_err(|e| e.to_string())?;
            app.manage(Arc::new(Mutex::new(conn)) as SharedConn);
            // One sync at a time, decided here rather than by the triggers.
            app.manage(Arc::new(sync::SyncEngine::new()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_feeds,
            list_entries,
            count_entries,
            get_entry,
            list_folders,
            entry_counts,
            create_folder,
            rename_folder,
            delete_folder,
            move_feed_to_folder,
            set_feed_title,
            set_read,
            mark_read_many,
            mark_scope_read,
            set_starred,
            import_opml,
            export_opml,
            export_sync_document,
            import_sync_document,
            load_sync_config,
            save_sync_config,
            test_sync_connection,
            sync_now,
            sync_status,
            remove_feed,
            add_feed,
            refresh_feed,
            refresh_all,
            refresh_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    /// The two commands return these values across the IPC boundary, so the
    /// tests here check what leaves the process: text that parses back into a
    /// document, counts the dialog can print, and serializable snapshots.
    #[test]
    fn export_returns_text_that_parses_as_a_document() {
        let conn = db::open_memory().unwrap();
        let outcome = export_sync_outcome(&conn).unwrap();

        let document: sync::Document = serde_json::from_str(&outcome.text).unwrap();
        assert_eq!(document.doc_version, sync::DOC_VERSION);
        assert_eq!(outcome.feeds_exported, 0);
        assert_eq!(outcome.folders_exported, 0);
        assert_eq!(outcome.marks_exported, 0);

        let as_ipc: serde_json::Value = serde_json::to_value(&outcome).unwrap();
        assert_eq!(as_ipc["text"].as_str().unwrap(), outcome.text);
    }

    #[test]
    fn importing_an_export_into_an_empty_library_changes_nothing() {
        let conn = db::open_memory().unwrap();
        let text = export_sync_outcome(&conn).unwrap().text;

        let summary = import_sync_outcome(&conn, &text).unwrap();
        assert_eq!(summary.feeds_added, 0);
        assert_eq!(summary.folders_added, 0);
        assert_eq!(summary.marks_recorded_for_later, 0);
        serde_json::to_value(&summary).unwrap();
    }

    #[test]
    fn a_document_this_build_cannot_read_reaches_the_ui_as_a_message() {
        let conn = db::open_memory().unwrap();
        let error = import_sync_outcome(
            &conn,
            r#"{"doc_version": 99, "read_horizon_day": "2026-01-01"}"#,
        )
        .unwrap_err();
        assert!(error.contains("version 99"), "{error}");
    }

    /// Registration is the one thing `cargo check` cannot catch: a command that
    /// exists but is missing from the handler simply never runs from the UI.
    #[test]
    fn every_sync_command_is_registered_with_the_handler() {
        let source = include_str!("lib.rs");
        let handler = source
            .split("generate_handler![")
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .expect("a handler list");
        for command in [
            "load_sync_config",
            "save_sync_config",
            "test_sync_connection",
            "sync_now",
            "sync_status",
            "export_sync_document",
            "import_sync_document",
        ] {
            assert!(handler.contains(command), "{command} is not registered");
        }
    }
}
