use crate::db;
use chrono::Utc;
use feed_rs::parser as feed_parser;
use rusqlite::Connection;
use std::time::Duration;

pub const FETCH_TIMEOUT: Duration = Duration::from_secs(20);

/// A parsed feed ready to be stored.
pub struct ParsedFeed {
    pub title: String,
    pub site_url: Option<String>,
    pub entries: Vec<ParsedEntry>,
}

pub struct ParsedEntry {
    pub guid: String,
    pub title: String,
    pub link: Option<String>,
    pub content_html: String,
    pub published_at: Option<String>,
}

/// Fetch a URL and return its bytes plus the final URL after redirects.
pub async fn fetch_bytes(url: &str) -> Result<(Vec<u8>, String), String> {
    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .user_agent("my-timeline-rss/0.1")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let final_url = resp.url().to_string();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("read body failed: {e}"))?
        .to_vec();
    Ok((bytes, final_url))
}

/// Parse feed bytes with feed-rs and sanitize entry HTML with ammonia (ingest-time).
pub fn parse_feed(bytes: &[u8]) -> Result<ParsedFeed, String> {
    // Custom id generator: when a feed entry has no GUID, fall back to its link
    // (empty link -> empty id -> entry filtered out below, never stored without identity).
    let parser = feed_parser::Builder::default()
        .id_generator(|links, _title, _uri| {
            links.first().map(|l| l.href.clone()).unwrap_or_default()
        })
        .build();
    let feed = parser.parse(bytes).map_err(|e| format!("parse failed: {e}"))?;
    let title = feed
        .title
        .map(|t| t.content)
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Untitled feed".to_string());
    let site_url = feed.links.first().map(|l| l.href.clone());

    let entries = feed
        .entries
        .into_iter()
        .map(|e| {
            let link = e.links.first().map(|l| l.href.clone());
            // GUID falls back to link; both missing is filtered out below.
            let guid = if !e.id.trim().is_empty() {
                e.id.trim().to_string()
            } else {
                link.clone().unwrap_or_default()
            }; // feed-rs already applies the link fallback via the id generator above
            let title = e
                .title
                .map(|t| t.content)
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| "(no title)".to_string());
            // Prefer full content, fall back to summary.
            let raw = e
                .content
                .and_then(|c| c.body)
                .filter(|s| !s.trim().is_empty())
                .or_else(|| e.summary.map(|s| s.content))
                .unwrap_or_default();
            let content_html = ammonia::clean(&raw);
            let published = e
                .published
                .or(e.updated)
                .map(|dt| dt.to_rfc3339());
            ParsedEntry {
                guid,
                title,
                link,
                content_html,
                published_at: published,
            }
        })
        .filter(|e| !e.guid.is_empty())
        .collect();

    Ok(ParsedFeed {
        title,
        site_url,
        entries,
    })
}

/// Store a parsed feed and its entries; returns the feed id.
pub fn store_feed(conn: &Connection, url: &str, parsed: &ParsedFeed) -> Result<i64, String> {
    let feed_id = db::insert_or_update_feed(conn, url, &parsed.title, parsed.site_url.as_deref())
        .map_err(|e| e.to_string())?;
    for e in &parsed.entries {
        db::upsert_entry(
            conn,
            feed_id,
            &e.guid,
            &e.title,
            e.link.as_deref(),
            &e.content_html,
            e.published_at.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(feed_id)
}

/// Fetch + parse + store for one URL, recording success/failure on the feed row.
/// Shared by refresh_feed, refresh_all, and add_feed.
pub async fn fetch_and_store(
    conn: &std::sync::Mutex<Connection>,
    feed_id: i64,
    url: &str,
) -> Result<(), String> {
    let result = async {
        let (bytes, _final_url) = fetch_bytes(url).await?;
        let parsed = parse_feed(&bytes)?;
        let conn = conn.lock().map_err(|e| e.to_string())?;
        store_feed(&conn, url, &parsed).map(|_| ())
    }
    .await;

    let now = Utc::now().to_rfc3339();
    let error = result.as_ref().err().map(|s| s.as_str());
    if let Ok(conn) = conn.lock() {
        let _ = db::set_feed_fetch_result(&conn, feed_id, &now, error);
    }
    result
}

/// Whether the bytes look like a feed we can parse.
#[cfg(test)]
pub fn looks_like_feed(bytes: &[u8]) -> bool {
    parse_feed(bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{open_memory, list_all_entries, EntryScope};

    const FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Fixture Feed</title>
    <link>https://fixture.example/</link>
    <item>
      <title>First post</title>
      <link>https://fixture.example/1</link>
      <guid isPermaLink="false">post-1</guid>
      <description>&lt;p&gt;Hello &lt;script&gt;alert(1)&lt;/script&gt;world&lt;/p&gt;</description>
      <pubDate>Mon, 01 Jan 2026 00:00:00 +0000</pubDate>
    </item>
    <item>
      <title>Second post</title>
      <link>https://fixture.example/2</link>
      <description>summary only</description>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn parses_fixture_and_sanitizes_script_tags() {
        let parsed = parse_feed(FIXTURE.as_bytes()).unwrap();
        assert_eq!(parsed.title, "Fixture Feed");
        assert_eq!(parsed.entries.len(), 2);

        let first = &parsed.entries[0];
        assert_eq!(first.guid, "post-1");
        assert!(!first.content_html.contains("<script"), "script must be stripped");
        assert!(first.content_html.contains("Hello"));
        assert_eq!(first.published_at.as_deref(), Some("2026-01-01T00:00:00+00:00"));

        let second = &parsed.entries[1];
        assert_eq!(second.guid, "https://fixture.example/2", "guid falls back to link");
        assert!(second.published_at.is_none());
    }

    #[test]
    fn rejects_non_feed_bytes() {
        assert!(parse_feed(b"<html><body>not a feed</body></html>").is_err());
        assert!(!looks_like_feed(b"<html><body>not a feed</body></html>"));
    }

    #[test]
    fn store_then_refetch_updates_without_duplicates() {
        let conn = open_memory().unwrap();
        let parsed = parse_feed(FIXTURE.as_bytes()).unwrap();

        let id = store_feed(&conn, "https://fixture.example/rss", &parsed).unwrap();
        store_feed(&conn, "https://fixture.example/rss", &parsed).unwrap();

        let entries = list_all_entries(&conn, "all", EntryScope::All).unwrap();
        assert_eq!(entries.len(), 2, "re-storing the same feed must not duplicate");
        let feeds = db::list_feeds(&conn).unwrap();
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].id, id);
        assert_eq!(feeds[0].title, "Fixture Feed");
    }
}
