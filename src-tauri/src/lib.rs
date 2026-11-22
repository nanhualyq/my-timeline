mod db;
mod fetch;
mod opml;
mod reader;

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
) -> Result<Vec<db::EntryDto>, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::list_entries(&guard, &view, scope)
}

#[tauri::command]
fn list_folders(app: tauri::AppHandle) -> Result<Vec<db::FolderDto>, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::list_folders(&guard)
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

/// Import an OPML file chosen by the frontend. Merge-only; performs no
/// fetching — entries only arrive through a later manual refresh.
#[tauri::command]
fn import_opml(app: tauri::AppHandle, path: String) -> Result<opml::ImportOutcome, String> {
    let xml = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    opml::import_opml(&guard, &xml)
}

/// Export all subscriptions (with folder grouping) as OPML to a path chosen
/// by the frontend.
#[tauri::command]
fn export_opml(app: tauri::AppHandle, path: String) -> Result<opml::ExportOutcome, String> {
    let conn = shared_conn(&app)?;
    let exported = {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        opml::export_opml(&guard)?
    };
    std::fs::write(&path, &exported.xml).map_err(|e| format!("cannot write {path}: {e}"))?;
    Ok(opml::ExportOutcome {
        feeds_exported: exported.feeds_exported,
    })
}

#[tauri::command]
fn set_read(app: tauri::AppHandle, entry_id: i64, is_read: bool) -> Result<(), String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::set_read(&guard, entry_id, is_read)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app data dir: {e}"))?;
            // Debug builds must not share the production database: the data dir
            // is derived from the bundle identifier, which `tauri dev` does not
            // override, so dev and release would otherwise open the same file.
            let data_dir = if cfg!(debug_assertions) {
                data_dir.with_file_name("com.mytimeline.app-dev")
            } else {
                data_dir
            };
            let conn = db::open(&data_dir.join("reader.db")).map_err(|e| e.to_string())?;
            app.manage(Arc::new(Mutex::new(conn)) as SharedConn);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_feeds,
            list_entries,
            list_folders,
            create_folder,
            rename_folder,
            delete_folder,
            move_feed_to_folder,
            set_read,
            set_starred,
            import_opml,
            export_opml,
            remove_feed,
            add_feed,
            refresh_feed,
            refresh_all
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
