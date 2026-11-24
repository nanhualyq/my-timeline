//! Derived row details: a short plain-text excerpt and a cover image URL.
//!
//! One module serves both callers: ingest (`fetch.rs`) has a parsed feed entry
//! with media metadata, while the one-time backfill (`db.rs`) only has stored
//! HTML. Keeping the text and image rules here stops the two from drifting.

use feed_rs::model::Entry;
use scraper::node::Node;
use scraper::{Html, Selector};
use url::Url;

/// Longest excerpt kept for a row, in characters.
pub const EXCERPT_MAX_CHARS: usize = 200;

/// Render HTML as one line of plain text: element text only, whitespace
/// collapsed to single spaces. Empty when the markup carries no visible text.
///
/// Ammonia drops `<script>`/`<style>` together with their contents; going
/// through it first means a raw feed summary (which is not sanitized at ingest)
/// cannot leak stylesheet or script source into the excerpt as text.
pub fn plain_text(html: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }
    let cleaned = ammonia::clean(html);
    let document = Html::parse_fragment(&cleaned);
    let mut out = String::new();
    for node in document.tree.nodes() {
        match node.value() {
            // Inline text is concatenated as-is: the markup already carries the
            // spacing, so `own <b>abstract</b>.` stays "own abstract."
            Node::Text(text) => out.push_str(text),
            // Block boundaries carry no characters of their own, so without this
            // `<p>first</p><p>second</p>` would run together.
            Node::Element(el) if is_block_element(el.name()) => out.push(' '),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_block_element(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "br"
            | "hr"
            | "li"
            | "ul"
            | "ol"
            | "dl"
            | "dd"
            | "dt"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "blockquote"
            | "pre"
            | "table"
            | "tr"
            | "td"
            | "th"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "figure"
            | "figcaption"
            | "aside"
            | "nav"
            | "main"
            | "form"
            | "address"
    )
}

/// `plain_text`, truncated to `EXCERPT_MAX_CHARS` at a word boundary.
pub fn excerpt_from_html(html: &str) -> String {
    truncate_at_word_boundary(&plain_text(html), EXCERPT_MAX_CHARS)
}

/// The entry's cover image: the URL of the most explicitly declared image.
///
/// Precedence follows how deliberately a feed points at "the entry's image":
/// a `media:thumbnail` is exactly that, `media:content` and image enclosures are
/// attachments, and the first `<img>` in the body is a heuristic tried last.
pub fn cover_from_entry(
    entry: &Entry,
    content_html: &str,
    feed_site_url: Option<&str>,
) -> Option<String> {
    let bases = bases_for(entry, feed_site_url);
    let thumbnails = entry
        .media
        .iter()
        .flat_map(|m| m.thumbnails.iter())
        .find_map(|t| resolve_url(&t.image.uri, &bases));
    if let Some(url) = thumbnails {
        return Some(url);
    }
    let media_content = entry
        .media
        .iter()
        .flat_map(|m| m.content.iter())
        .find(|c| c.content_type.as_ref().is_some_and(|t| is_image_type(&t.to_string())))
        .and_then(|c| c.url.as_ref())
        .and_then(|u| resolve_url(u.as_str(), &bases));
    if let Some(url) = media_content {
        return Some(url);
    }
    let enclosure = entry
        .links
        .iter()
        .filter(|l| l.rel.as_deref() == Some("enclosure"))
        .filter(|l| l.media_type.as_deref().is_some_and(is_image_type))
        .find_map(|l| resolve_url(&l.href, &bases));
    enclosure.or_else(|| cover_from_html(content_html, &bases))
}

/// First `<img>` in `html`, resolved against `bases`. Used as the last-resort
/// cover and, by the backfill, as the only source (media metadata is not
/// recoverable from stored HTML).
pub fn cover_from_html(html: &str, bases: &[&str]) -> Option<String> {
    let document = Html::parse_fragment(html);
    let selector = Selector::parse("img[src]").expect("static selector");
    document
        .select(&selector)
        .find_map(|img| resolve_url(img.value().attr("src")?, bases))
}

/// Absolute `http(s)` URL for `candidate`, resolving relative and
/// protocol-relative references against the first usable base.
///
/// Rejects every other scheme — notably `data:` payloads, which would bloat the
/// database and are not images the webview should be told to fetch.
pub fn resolve_url(candidate: &str, bases: &[&str]) -> Option<String> {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return None;
    }
    if let Ok(url) = Url::parse(candidate) {
        return http_url(url);
    }
    bases.iter().find_map(|base| {
        let base = Url::parse(base).ok()?;
        base.join(candidate).ok().and_then(http_url)
    })
}

fn http_url(url: Url) -> Option<String> {
    match url.scheme() {
        "http" | "https" => Some(url.to_string()),
        _ => None,
    }
}

fn is_image_type(media_type: &str) -> bool {
    media_type.to_ascii_lowercase().starts_with("image/")
}

/// Where relative image references can be resolved against, most specific
/// first: the entry's own base, its link, then the feed's site address.
fn bases_for<'a>(entry: &'a Entry, feed_site_url: Option<&'a str>) -> Vec<&'a str> {
    let mut bases = Vec::new();
    if let Some(base) = entry.base.as_deref() {
        bases.push(base);
    }
    if let Some(link) = entry.links.first() {
        bases.push(link.href.as_str());
    }
    if let Some(site) = feed_site_url {
        bases.push(site);
    }
    bases
}

fn truncate_at_word_boundary(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let cut = text
        .char_indices()
        .nth(max_chars)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    let head = &text[..cut];
    let head = match head.rfind(' ') {
        Some(i) if i > 0 => &head[..i],
        _ => head,
    };
    format!("{}…", head.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use feed_rs::parser;

    fn text_html(html: &str) -> String {
        plain_text(html)
    }

    #[test]
    fn plain_text_keeps_inline_text_joined() {
        assert_eq!(
            text_html("<p>The publisher's own <b>abstract</b>.</p>"),
            "The publisher's own abstract."
        );
    }

    #[test]
    fn plain_text_separates_adjacent_blocks() {
        assert_eq!(text_html("<p>first</p><p>second</p>"), "first second");
        assert_eq!(text_html("<ul><li>a</li><li>b</li></ul>"), "a b");
    }

    #[test]
    fn plain_text_drops_markup_and_collapses_whitespace() {
        let text = text_html("<p>Hello <b>there</b></p>\n\n<p>second   line</p>");
        assert_eq!(text, "Hello there second line");
    }

    #[test]
    fn plain_text_skips_script_and_style_bodies() {
        let text = text_html("<style>p{color:red}</style><p>Read</p><script>alert(1)</script>");
        assert_eq!(text, "Read");
    }

    #[test]
    fn plain_text_of_empty_markup_is_empty() {
        assert_eq!(text_html("   "), "");
        assert_eq!(text_html("<p></p>"), "");
    }

    #[test]
    fn excerpt_shorter_than_limit_is_untouched() {
        let text = excerpt_from_html("<p>Short body</p>");
        assert_eq!(text, "Short body");
        assert!(!text.ends_with('…'));
    }

    #[test]
    fn excerpt_is_truncated_at_a_word_boundary() {
        let long = format!("<p>{}</p>", "word ".repeat(100));
        let excerpt = excerpt_from_html(&long);
        assert!(excerpt.chars().count() <= EXCERPT_MAX_CHARS + 1, "bounded: {excerpt}");
        assert!(excerpt.ends_with('…'));
        assert!(!excerpt.trim_end_matches('…').ends_with("wor"), "cut mid-word: {excerpt}");
        assert!(!excerpt.contains("  "), "no collapsed leftovers: {excerpt}");
    }

    #[test]
    fn excerpt_counts_characters_not_bytes() {
        let long = format!("<p>{}</p>", "é".repeat(500));
        let excerpt = excerpt_from_html(&long);
        assert!(excerpt.chars().count() <= EXCERPT_MAX_CHARS + 1);
    }

    #[test]
    fn resolves_relative_and_protocol_relative_covers() {
        let bases = ["https://example.test/posts/1"];
        assert_eq!(
            resolve_url("cover.jpg", &bases).as_deref(),
            Some("https://example.test/posts/cover.jpg")
        );
        assert_eq!(
            resolve_url("//cdn.example.test/a.jpg", &bases).as_deref(),
            Some("https://cdn.example.test/a.jpg")
        );
        assert_eq!(
            resolve_url("http://plain.example/a.png", &[]).as_deref(),
            Some("http://plain.example/a.png")
        );
    }

    #[test]
    fn rejects_non_http_covers() {
        let bases = ["https://example.test/posts/1"];
        assert_eq!(resolve_url("data:image/png;base64,AAAA", &bases), None);
        assert_eq!(resolve_url("javascript:alert(1)", &bases), None);
        assert_eq!(resolve_url("   ", &bases), None);
    }

    fn parse_entry(entry_xml: &str) -> Entry {
        let feed = format!(
            r#"<?xml version="1.0"?><rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/">
            <channel><title>F</title><link>https://feed.example/</link>{entry_xml}</channel></rss>"#
        );
        parser::parse(feed.as_bytes()).unwrap().entries.remove(0)
    }

    #[test]
    fn cover_prefers_media_thumbnail() {
        let entry = parse_entry(
            r#"<item><title>T</title><guid>g</guid>
               <media:thumbnail url="https://img.example/thumb.jpg"/>
               <media:content url="https://img.example/content.jpg" type="image/jpeg"/>
               <description>&lt;img src="https://img.example/inline.jpg"/&gt;</description>
            </item>"#,
        );
        let html = ammonia::clean("<img src=\"https://img.example/inline.jpg\">");
        assert_eq!(
            cover_from_entry(&entry, &html, Some("https://site.example/")).as_deref(),
            Some("https://img.example/thumb.jpg")
        );
    }

    #[test]
    fn cover_falls_back_to_image_media_content() {
        let entry = parse_entry(
            r#"<item><title>T</title><guid>g</guid>
               <media:content url="https://img.example/content.jpg" type="image/jpeg"/>
               <description>&lt;img src="https://img.example/inline.jpg"/&gt;</description>
            </item>"#,
        );
        let html = ammonia::clean("<img src=\"https://img.example/inline.jpg\">");
        assert_eq!(
            cover_from_entry(&entry, &html, None).as_deref(),
            Some("https://img.example/content.jpg")
        );
    }

    #[test]
    fn cover_falls_back_to_image_enclosure() {
        let entry = parse_entry(
            r#"<item><title>T</title><guid>g</guid>
               <enclosure url="https://img.example/enclosure.png" type="image/png" length="1"/>
               <description>&lt;img src="https://img.example/inline.jpg"/&gt;</description>
            </item>"#,
        );
        let html = ammonia::clean("<img src=\"https://img.example/inline.jpg\">");
        assert_eq!(
            cover_from_entry(&entry, &html, None).as_deref(),
            Some("https://img.example/enclosure.png")
        );
    }

    #[test]
    fn cover_falls_back_to_first_image_in_content() {
        let entry = parse_entry(r#"<item><title>T</title><guid>g</guid></item>"#);
        let html = ammonia::clean("<p>x</p><img src=\"/first.jpg\"><img src=\"/second.jpg\">");
        assert_eq!(
            cover_from_entry(&entry, &html, Some("https://site.example/blog/")).as_deref(),
            Some("https://site.example/first.jpg")
        );
    }

    #[test]
    fn no_image_material_yields_no_cover() {
        let entry = parse_entry(r#"<item><title>T</title><guid>g</guid></item>"#);
        assert_eq!(cover_from_entry(&entry, "<p>text only</p>", None), None);
    }
}
