use crate::db::{self, FeedDto};
use crate::fetch::{self, fetch_and_store};
use futures::stream::{FuturesUnordered, StreamExt};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub type SharedConn = Arc<Mutex<Connection>>;

/// Bounded concurrency for full refreshes.
pub const MAX_CONCURRENT_FETCHES: usize = 5;

/// Refresh a single feed. Returns Ok(()) when the fetch succeeded, Err(reason)
/// when it failed (the failure is already recorded on the feed row).
pub async fn refresh_feed(conn: SharedConn, feed_id: i64) -> Result<(), String> {
    let url = {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::feed_url_by_id(&guard, feed_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("feed {feed_id} not found"))?
    };
    fetch_and_store(&conn, feed_id, &url).await
}

/// Refresh every feed concurrently with a bounded worker set.
/// Each feed's outcome is recorded independently; this never fails as a whole.
/// Returns (feed_id, result) for every feed.
pub async fn refresh_all(conn: SharedConn) -> Vec<(i64, Result<(), String>)> {
    let targets = {
        match conn.lock() {
            Ok(guard) => db::all_feed_ids_and_urls(&guard).unwrap_or_default(),
            Err(_) => return vec![],
        }
    };

    let mut results = Vec::with_capacity(targets.len());
    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
    let mut pending = targets.into_iter();

    loop {
        while in_flight.len() < MAX_CONCURRENT_FETCHES {
            let Some((feed_id, url)) = pending.next() else { break };
            let conn = conn.clone();
            in_flight.push(async move {
                let result = fetch_and_store(&conn, feed_id, &url).await;
                (feed_id, result)
            });
        }
        match in_flight.next().await {
            Some(item) => results.push(item),
            None => break,
        }
    }
    results
}

/// The outcome of an add-feed attempt.
pub enum AddFeedResult {
    /// A feed already stored under this URL; refreshed just now.
    Existing(FeedDto),
    /// A new subscription.
    Added(FeedDto),
}

/// Serializable per-feed outcome of a full refresh.
#[derive(serde::Serialize)]
pub struct RefreshOutcome {
    pub feed_id: i64,
    pub error: Option<String>,
}

/// Add a subscription from a user-supplied URL.
/// Tries the URL as a feed first; if it is an HTML page, discovers a linked feed.
/// On failure nothing is created or modified.
pub async fn add_feed(conn: SharedConn, url: &str) -> Result<AddFeedResult, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("URL is empty".into());
    }
    // Light validation up front so a bare string fails fast.
    let parsed_url = url::Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;
    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err("only http(s) URLs are supported".into());
    }

    // Is it already subscribed?
    let existing_id = {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::feed_id_by_url(&guard, url).map_err(|e| e.to_string())?
    };

    // Attempt 1: the URL itself is a feed.
    match fetch::fetch_bytes(url).await {
        Ok((bytes, final_url)) => {
            if let Ok(parsed) = fetch::parse_feed(&bytes) {
                return finish_add(&conn, url, parsed, existing_id).await;
            }
            // Attempt 2: HTML page -> discover <link rel="alternate">.
            let discovered = discover_feed_link(&bytes, &final_url)?;
            match fetch::fetch_bytes(&discovered).await {
                Ok((feed_bytes, _)) => {
                    let parsed = fetch::parse_feed(&feed_bytes)
                        .map_err(|e| format!("discovered link is not a feed: {e}"))?;
                    finish_add(&conn, &discovered, parsed, existing_id).await
                }
                Err(e) => Err(format!("failed to fetch discovered feed: {e}")),
            }
        }
        Err(e) => Err(format!("failed to fetch URL: {e}")),
    }
}

async fn finish_add(
    conn: &SharedConn,
    url: &str,
    parsed: fetch::ParsedFeed,
    existing_id: Option<i64>,
) -> Result<AddFeedResult, String> {
    let dto = {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        let id = fetch::store_feed(&guard, url, &parsed)?;
        db::get_feed(&guard, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "feed vanished after insert".to_string())?
    };
    Ok(if existing_id.is_none() {
        AddFeedResult::Added(dto)
    } else {
        AddFeedResult::Existing(dto)
    })
}

/// Find `<link rel="alternate" type="application/rss+xml|atom+xml">` in HTML
/// and resolve it against the page URL.
fn discover_feed_link(html: &[u8], page_url: &str) -> Result<String, String> {
    // Lossy decode is fine: we only need tag attributes.
    let text = String::from_utf8_lossy(html);
    let document = scraper::Html::parse_document(&text);
    let selector = scraper::Selector::parse("link[rel='alternate']").unwrap();
    let base = url::Url::parse(page_url).map_err(|e| e.to_string())?;

    for link in document.select(&selector) {
        let rel_type = link
            .attr("type")
            .unwrap_or("")
            .to_ascii_lowercase();
        let is_feed = rel_type.contains("rss") || rel_type.contains("atom");
        let href = link.attr("href").unwrap_or("");
        if is_feed && !href.is_empty() {
            let abs = base
                .join(href)
                .map_err(|e| format!("bad feed href: {e}"))?;
            return Ok(abs.to_string());
        }
    }
    Err("page does not link to a feed".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;

    const FEED_A: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Feed A</title><link>https://a.example/</link>
<item><title>A1</title><link>https://a.example/1</link><guid>a-1</guid></item>
</channel></rss>"#;

    const FEED_B: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel><title>Feed B</title><link>https://b.example/</link>
<item><title>B1</title><link>https://b.example/1</link><guid>b-1</guid></item>
</channel></rss>"#;

    /// Minimal local HTTP server serving fixed responses by path.
    fn serve(routes: Vec<(&'static str, String)>) -> String {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/")
                    .to_string();
                let (status, body) = routes
                    .iter()
                    .find(|(p, _)| *p == path)
                    .map(|(_, b)| ("200 OK", b.clone()))
                    .unwrap_or(("404 Not Found", "not found".to_string()));
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    fn shared() -> SharedConn {
        Arc::new(Mutex::new(open_memory().unwrap()))
    }

    #[tokio::test]
    async fn add_direct_feed_url_creates_subscription() {
        let base = serve(vec![("/feed.xml", FEED_A.to_string())]);
        let conn = shared();
        let result = add_feed(conn.clone(), &format!("{base}/feed.xml")).await;
        match result {
            Ok(AddFeedResult::Added(f)) => {
                assert_eq!(f.title, "Feed A");
                assert_eq!(f.unread_count, 1);
            }
            other => panic!("expected Added, got {:?}", err_str(&other)),
        }
    }

    #[tokio::test]
    async fn add_page_url_discovers_linked_feed() {
        let page = r#"<html><head>
            <link rel="alternate" type="application/rss+xml" href="/rss.xml">
            </head><body>hi</body></html>"#;
        let base = serve(vec![
            ("/", page.to_string()),
            ("/rss.xml", FEED_B.to_string()),
        ]);
        let conn = shared();
        let result = add_feed(conn.clone(), &format!("{base}/")).await;
        match result {
            Ok(AddFeedResult::Added(f)) => {
                assert_eq!(f.title, "Feed B");
                assert_eq!(f.unread_count, 1);
            }
            other => panic!("expected Added, got {:?}", err_str(&other)),
        }
    }

    #[tokio::test]
    async fn add_invalid_url_creates_nothing() {
        let base = serve(vec![]); // every path 404s
        let conn = shared();

        let r1 = add_feed(conn.clone(), &format!("{base}/nothing")).await;
        assert!(r1.is_err(), "404 page must be rejected");

        let r2 = add_feed(conn.clone(), "not a url at all").await;
        assert!(r2.is_err());

        let guard = conn.lock().unwrap();
        assert_eq!(db::list_feeds(&guard).unwrap().len(), 0, "no feed rows created");
        assert_eq!(
            db::list_entries(&guard, "all", None).unwrap().len(),
            0,
            "no entries created"
        );
    }

    #[tokio::test]
    async fn add_same_url_twice_does_not_duplicate() {
        let base = serve(vec![("/feed.xml", FEED_A.to_string())]);
        let conn = shared();
        let url = format!("{base}/feed.xml");
        add_feed(conn.clone(), &url).await.unwrap();
        let second = add_feed(conn.clone(), &url).await.unwrap();
        assert!(matches!(second, AddFeedResult::Existing(_)));
        let guard = conn.lock().unwrap();
        assert_eq!(db::list_feeds(&guard).unwrap().len(), 1);
        assert_eq!(db::list_entries(&guard, "all", None).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn refresh_all_isolates_failures() {
        let base = serve(vec![("/good.xml", FEED_A.to_string())]);
        let conn = shared();
        // Feed A: local server (works). Feed B: unroutable port (fails fast).
        let good_id = {
            let guard = conn.lock().unwrap();
            db::insert_or_update_feed(&guard, &format!("{base}/good.xml"), "Good", None).unwrap()
        };
        let bad_id = {
            let guard = conn.lock().unwrap();
            db::insert_or_update_feed(
                &guard,
                "http://127.0.0.1:9/bad.xml",
                "Bad",
                None,
            )
            .unwrap()
        };

        let results = refresh_all(conn.clone()).await;
        assert_eq!(results.len(), 2, "every feed gets an outcome");

        let guard = conn.lock().unwrap();
        let good = db::get_feed(&guard, good_id).unwrap().unwrap();
        let bad = db::get_feed(&guard, bad_id).unwrap().unwrap();

        assert!(good.last_error.is_none(), "good feed stored without error");
        assert!(good.last_fetched_at.is_some());
        assert!(bad.last_error.is_some(), "failed feed records its error");
        let entries = db::list_entries(&guard, "all", None).unwrap();
        assert_eq!(entries.len(), 1, "good feed's entry stored despite sibling failure");
        assert_eq!(entries[0].feed_id, good_id);
    }

    #[tokio::test]
    async fn refresh_single_feed_touches_only_that_feed() {
        let base = serve(vec![("/good.xml", FEED_A.to_string())]);
        let conn = shared();
        let good_id = {
            let guard = conn.lock().unwrap();
            db::insert_or_update_feed(&guard, &format!("{base}/good.xml"), "Good", None).unwrap()
        };
        let other_id = {
            let guard = conn.lock().unwrap();
            let id =
                db::insert_or_update_feed(&guard, "http://127.0.0.1:9/other.xml", "Other", None)
                    .unwrap();
            db::set_feed_fetch_result(&guard, id, "2026-01-01T00:00:00Z", Some("old error"))
                .unwrap();
            id
        };

        refresh_feed(conn.clone(), good_id).await.unwrap();

        let guard = conn.lock().unwrap();
        let good = db::get_feed(&guard, good_id).unwrap().unwrap();
        assert!(good.last_error.is_none());
        assert_eq!(good.unread_count, 1);
        let other = db::get_feed(&guard, other_id).unwrap().unwrap();
        assert_eq!(
            other.last_error.as_deref(),
            Some("old error"),
            "untouched feed keeps its previous state"
        );
    }

    fn err_str<T>(r: &Result<T, String>) -> String {
        match r {
            Ok(_) => "ok".into(),
            Err(e) => e.clone(),
        }
    }
}
