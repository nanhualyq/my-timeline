mod db;
mod fetch;
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
    feed_id: Option<i64>,
) -> Result<Vec<db::EntryDto>, String> {
    let conn = shared_conn(&app)?;
    let guard = conn.lock().map_err(|e| e.to_string())?;
    db::list_entries(&guard, &view, feed_id)
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
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app data dir: {e}"))?;
            let conn = db::open(&data_dir.join("reader.db")).map_err(|e| e.to_string())?;
            app.manage(Arc::new(Mutex::new(conn)) as SharedConn);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_feeds,
            list_entries,
            set_read,
            remove_feed,
            add_feed,
            refresh_feed,
            refresh_all
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
