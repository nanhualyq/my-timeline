//! OPML import/export: subscriptions and folder grouping in the standard
//! interchange format. Parsing and generation live entirely in Rust.

use crate::db;
use rusqlite::Connection;
use rusqlite::params;
use serde::Serialize;
use std::collections::HashMap;

/// Result of an import: how many feeds were added, how many existing URLs
/// were skipped, and how many new folders were created.
#[derive(Debug, Clone, Serialize)]
pub struct ImportOutcome {
    pub feeds_added: i64,
    pub feeds_skipped: i64,
    pub folders_created: i64,
}

/// The rendered document plus its feed count (not sent to the frontend as-is).
#[derive(Debug, Clone)]
pub struct Exported {
    pub xml: String,
    pub feeds_exported: i64,
}

/// Serializable result of an export.
#[derive(Debug, Clone, Serialize)]
pub struct ExportOutcome {
    pub feeds_exported: i64,
}

/// A feed outline found in the document, with the folder it belongs to.
/// Nested folders are flattened: `folder` is the top-level ancestor name.
struct PendingFeed {
    url: String,
    title: Option<String>,
    site_url: Option<String>,
    folder: Option<String>,
}

/// Import an OPML document. Merge-only: an already-subscribed URL is skipped
/// (its folder assignment and entries are untouched); new feeds are inserted
/// with the title from the outline. Creates folders on demand. Performs no
/// fetching — entries only arrive through a later manual refresh.
pub fn import_opml(conn: &Connection, xml: &str) -> Result<ImportOutcome, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("invalid OPML: {e}"))?;
    let root = doc.root_element();
    if !root.is_element() || !root.tag_name().name().eq_ignore_ascii_case("opml") {
        return Err("not an OPML document: missing <opml> root element".into());
    }
    let body = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("body"))
        .ok_or("not an OPML document: missing <body> element")?;

    let mut pending = Vec::new();
    collect_feeds(body, None, &mut pending);

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let mut folder_ids = load_folder_names(&tx)?;
    let mut outcome = ImportOutcome {
        feeds_added: 0,
        feeds_skipped: 0,
        folders_created: 0,
    };

    for feed in pending {
        if db::feed_id_by_url(&tx, &feed.url)
            .map_err(|e| e.to_string())?
            .is_some()
        {
            outcome.feeds_skipped += 1;
            continue;
        }
        let folder_id = match &feed.folder {
            Some(name) => {
                let id = match folder_ids.get(name) {
                    Some(id) => *id,
                    None => {
                        let folder = db::create_folder(&tx, name)?;
                        folder_ids.insert(name.clone(), folder.id);
                        outcome.folders_created += 1;
                        folder.id
                    }
                };
                Some(id)
            }
            None => None,
        };
        let title = feed
            .title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .unwrap_or(feed.url.as_str())
            .to_string();
        tx.execute(
            "INSERT INTO feeds (url, title, site_url, folder_id) VALUES (?1, ?2, ?3, ?4)",
            params![feed.url, title, feed.site_url, folder_id],
        )
        .map_err(|e| e.to_string())?;
        outcome.feeds_added += 1;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(outcome)
}

/// Depth-first walk of outlines. `top_folder` is the name of the outermost
/// folder ancestor so far: feeds are always assigned to it (nested folders
/// flatten to their top-level ancestor). Non-feed outlines without feed
/// descendants never produce a folder because no feed references their name.
fn collect_feeds(
    node: roxmltree::Node<'_, '_>,
    top_folder: Option<&str>,
    out: &mut Vec<PendingFeed>,
) {
    for child in node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("outline"))
    {
        match attr_ci(child, "xmlUrl").filter(|u| !u.trim().is_empty()) {
            Some(url) => out.push(PendingFeed {
                url: url.trim().to_string(),
                title: attr_ci(child, "text").or_else(|| attr_ci(child, "title")),
                site_url: attr_ci(child, "htmlUrl").filter(|u| !u.trim().is_empty()),
                folder: top_folder.map(str::to_string),
            }),
            None => {
                let own_name = attr_ci(child, "text").or_else(|| attr_ci(child, "title"));
                let folder_name = top_folder
                    .map(str::to_string)
                    .or_else(|| own_name.filter(|n| !n.trim().is_empty()));
                collect_feeds(child, folder_name.as_deref(), out);
            }
        }
    }
}

fn attr_ci(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    node.attributes()
        .find(|a| a.name().eq_ignore_ascii_case(name))
        .map(|a| a.value().to_string())
}

fn load_folder_names(conn: &Connection) -> Result<HashMap<String, i64>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name FROM folders")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    rows.map(|row| {
        let (id, name) = row.map_err(|e| e.to_string())?;
        Ok((name, id))
    })
    .collect()
}

/// Render all subscriptions as OPML. Each folder becomes an outline wrapping
/// its feeds; uncategorized feeds are exported as top-level feed outlines.
/// Empty folders are omitted because import cannot restore them.
pub fn export_opml(conn: &Connection) -> Result<Exported, String> {
    let folders = db::list_folders(conn)?;
    let all_feeds = db::list_feeds(conn).map_err(|e| e.to_string())?;

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<opml version=\"2.0\">\n");
    out.push_str("  <head>\n    <title>Subscriptions</title>\n  </head>\n");
    out.push_str("  <body>\n");

    let mut feeds_exported = 0i64;
    for folder in &folders {
        if folder.feeds.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "    <outline text=\"{}\">\n",
            escape(&folder.name)
        ));
        for feed in &folder.feeds {
            out.push_str(&feed_outline(feed, "      "));
            feeds_exported += 1;
        }
        out.push_str("    </outline>\n");
    }
    for feed in all_feeds.iter().filter(|f| f.folder_id.is_none()) {
        out.push_str(&feed_outline(feed, "    "));
        feeds_exported += 1;
    }

    out.push_str("  </body>\n</opml>\n");
    Ok(Exported { xml: out, feeds_exported })
}

fn feed_outline(feed: &db::FeedDto, indent: &str) -> String {
    let mut line = format!(
        "{indent}<outline text=\"{}\" type=\"rss\" xmlUrl=\"{}\"",
        escape(&feed.title),
        escape(&feed.url)
    );
    if let Some(site) = &feed.site_url {
        line.push_str(&format!(" htmlUrl=\"{}\"", escape(site)));
    }
    line.push_str(" />\n");
    line
}

fn escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            c => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head><title>My subs</title></head>
  <body>
    <outline text="Tech &amp; Gadgets">
      <outline text="The Verge" type="rss" xmlUrl="https://theverge.com/rss.xml" htmlUrl="https://theverge.com/"/>
      <outline text="Sub group">
        <outline text="Ars" type="rss" xmlUrl="https://arstechnica.com/feed"/>
      </outline>
    </outline>
    <outline text="Solo" type="rss" xmlUrl="https://solo.example/feed"/>
  </body>
</opml>"#;

    #[test]
    fn import_creates_folders_and_feeds_without_fetching() {
        let conn = open_memory().unwrap();
        let outcome = import_opml(&conn, SAMPLE).unwrap();
        assert_eq!(
            (outcome.feeds_added, outcome.feeds_skipped, outcome.folders_created),
            (3, 0, 1)
        );

        let folders = db::list_folders(&conn).unwrap();
        assert_eq!(folders.len(), 1, "only the top-level folder is created");
        assert_eq!(folders[0].name, "Tech & Gadgets");
        let titles: Vec<&str> = folders[0].feeds.iter().map(|f| f.title.as_str()).collect();
        assert!(titles.contains(&"The Verge"), "direct child feed nested");
        assert!(titles.contains(&"Ars"), "nested-folder feed flattened to top-level ancestor");
        assert!(!folders.iter().any(|f| f.name == "Sub group"), "intermediate folder not created");

        let solo = db::list_feeds(&conn)
            .unwrap()
            .into_iter()
            .find(|f| f.url == "https://solo.example/feed")
            .unwrap();
        assert_eq!(solo.folder_id, None, "top-level feed outline is uncategorized");

        // Import only stores records — no entries exist until a manual refresh.
        assert_eq!(
            db::list_all_entries(&conn, "all", db::EntryScope::All).unwrap().len(),
            0
        );
    }

    #[test]
    fn import_skips_existing_url_preserving_its_folder() {
        let conn = open_memory().unwrap();
        let home = db::create_folder(&conn, "Home").unwrap();
        let existing = db::insert_or_update_feed(&conn, "https://theverge.com/rss.xml", "Old", None).unwrap();
        db::move_feed_to_folder(&conn, existing, Some(home.id)).unwrap();

        let outcome = import_opml(&conn, SAMPLE).unwrap();
        assert_eq!(outcome.feeds_skipped, 1, "existing URL skipped");
        assert_eq!(outcome.feeds_added, 2, "new URLs added");

        let feed = db::get_feed(&conn, existing).unwrap().unwrap();
        assert_eq!(feed.folder_id, Some(home.id), "existing folder assignment preserved");
        assert_eq!(feed.title, "Old", "existing title untouched");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM feeds WHERE url = 'https://theverge.com/rss.xml'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "no duplicate feed created");
    }

    #[test]
    fn import_creates_no_folder_when_all_its_feeds_are_skipped() {
        let conn = open_memory().unwrap();
        db::insert_or_update_feed(&conn, "https://theverge.com/rss.xml", "Old", None).unwrap();
        let opml = r#"<opml version="2.0"><body>
            <outline text="Elsewhere">
              <outline text="V" xmlUrl="https://theverge.com/rss.xml"/>
            </outline>
          </body></opml>"#;

        let outcome = import_opml(&conn, opml).unwrap();
        assert_eq!(outcome.feeds_skipped, 1);
        assert_eq!(outcome.feeds_added, 0);
        assert_eq!(outcome.folders_created, 0, "no empty folder from a skipped feed");
        assert!(db::list_folders(&conn).unwrap().is_empty());
    }

    #[test]
    fn import_rejects_malformed_and_non_opml_documents() {
        let conn = open_memory().unwrap();
        assert!(import_opml(&conn, "<opml><body>").is_err(), "malformed XML");
        assert!(import_opml(&conn, "<html><body/></html>").is_err(), "wrong root element");
        assert!(import_opml(&conn, "not xml at all").is_err(), "no XML root");
        assert!(import_opml(&conn, "<opml version=\"2.0\"></opml>").is_err(), "missing body");
        assert_eq!(db::list_feeds(&conn).unwrap().len(), 0, "nothing created on rejection");
        assert_eq!(db::list_folders(&conn).unwrap().len(), 0);
    }

    #[test]
    fn export_wraps_folder_feeds_and_keeps_uncategorized_top_level() {
        let conn = open_memory().unwrap();
        let tech = db::create_folder(&conn, "Tech \"R\" & Co").unwrap();
        let a = db::insert_or_update_feed(
            &conn,
            "https://a.example/feed",
            "A <site>",
            Some("https://a.example/"),
        )
        .unwrap();
        db::move_feed_to_folder(&conn, a, Some(tech.id)).unwrap();
        db::insert_or_update_feed(&conn, "https://b.example/feed", "B", None).unwrap();

        let exported = export_opml(&conn).unwrap();
        assert_eq!(exported.feeds_exported, 2);
        assert!(exported.xml.starts_with("<?xml"));
        assert!(exported.xml.contains("<opml version=\"2.0\">"));
        // Folder outline wraps its feed; special characters are escaped.
        let folder_pos = exported.xml.find("text=\"Tech &quot;R&quot; &amp; Co\"").unwrap();
        let a_pos = exported.xml.find("xmlUrl=\"https://a.example/feed\"").unwrap();
        let b_pos = exported.xml.find("xmlUrl=\"https://b.example/feed\"").unwrap();
        assert!(folder_pos < a_pos, "feed A sits inside the folder outline");
        assert!(a_pos < exported.xml.find("</outline>").unwrap());
        assert!(
            exported.xml[..b_pos].matches("<outline text=\"Tech").count() == 1
                && b_pos > exported.xml.find("Tech &quot;R&quot;").unwrap(),
            "uncategorized feed B is outside the folder"
        );
        assert!(exported.xml.contains("htmlUrl=\"https://a.example/\""));
        assert!(exported.xml.contains("text=\"A &lt;site&gt;\""), "title escaped");
    }

    #[test]
    fn export_uses_custom_display_title() {
        let conn = open_memory().unwrap();
        let id = db::insert_or_update_feed(&conn, "https://a.example/feed", "Site Title", None)
            .unwrap();
        db::set_feed_title(&conn, id, Some("My Custom Title")).unwrap();

        let exported = export_opml(&conn).unwrap();
        assert!(
            exported.xml.contains("text=\"My Custom Title\""),
            "export carries the custom display title"
        );
        assert!(
            !exported.xml.contains("Site Title"),
            "the overridden site title is not exported"
        );
    }

    #[test]
    fn export_then_import_round_trip_preserves_folders() {
        let conn = open_memory().unwrap();
        let tech = db::create_folder(&conn, "Tech & Gadgets").unwrap();
        let a = db::insert_or_update_feed(
            &conn,
            "https://a.example/feed",
            "Feed \"A\"",
            Some("https://a.example/"),
        )
        .unwrap();
        db::move_feed_to_folder(&conn, a, Some(tech.id)).unwrap();
        db::insert_or_update_feed(&conn, "https://b.example/feed", "Feed B", None).unwrap();

        let xml = export_opml(&conn).unwrap().xml;

        let fresh = open_memory().unwrap();
        let outcome = import_opml(&fresh, &xml).unwrap();
        assert_eq!((outcome.feeds_added, outcome.folders_created), (2, 1));

        let folders = db::list_folders(&fresh).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "Tech & Gadgets");
        assert_eq!(folders[0].feeds.len(), 1);
        assert_eq!(folders[0].feeds[0].url, "https://a.example/feed");
        assert_eq!(folders[0].feeds[0].title, "Feed \"A\"");
        assert_eq!(folders[0].feeds[0].site_url.as_deref(), Some("https://a.example/"));

        let b = db::list_feeds(&fresh)
            .unwrap()
            .into_iter()
            .find(|f| f.url == "https://b.example/feed")
            .unwrap();
        assert_eq!(b.folder_id, None, "uncategorized stays uncategorized");
    }
}
