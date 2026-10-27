//! Checking a suggested source before it's offered: a page kizn fetched once
//! must be a real, readable page about the concept. Pure: the app fetches,
//! this decides. A model suggests sources; code decides whether they're real.

use std::sync::LazyLock;

use regex::Regex;

/// Readable text shorter than this isn't something to study from (an app
/// shell, a login wall, a stub).
pub const MIN_WORDS: usize = 150;

/// Why a fetched page isn't offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejected {
    /// Not a 2xx answer.
    Status(u16),
    /// Not HTML or text (a PDF, an image, a download).
    NotAPage(String),
    /// Scripts and almost no text: the page is drawn in the browser.
    AppShell,
    /// Too little readable text.
    TooShort(usize),
    /// Readable, but doesn't mention the concept's key terms.
    OffTopic,
    /// A video with no captions kizn can read.
    NoTranscript,
}

impl Rejected {
    /// The label stored as the reason (`<label>: <detail>`).
    pub fn label(&self) -> &'static str {
        match self {
            Rejected::Status(_) => "unreachable",
            Rejected::NotAPage(_) => "not a page",
            Rejected::AppShell => "empty app shell",
            Rejected::TooShort(_) => "too short",
            Rejected::OffTopic => "off topic",
            Rejected::NoTranscript => "no transcript",
        }
    }
}

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rejected::Status(s) => write!(f, "unreachable: HTTP {s}"),
            Rejected::NotAPage(t) => write!(f, "not a page: {t}"),
            Rejected::AppShell => write!(f, "empty app shell: the page needs a browser to show anything"),
            Rejected::TooShort(n) => write!(f, "too short: {n} words of readable text"),
            Rejected::OffTopic => write!(f, "off topic: it doesn't mention the concept's key terms"),
            Rejected::NoTranscript => write!(f, "no transcript: the video has no captions kizn can read"),
        }
    }
}

/// A page that passed: its title and how much there is to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub title: Option<String>,
    pub words: usize,
}

static DROP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(script|style|noscript|svg|template|head|nav|footer)\b.*?</(script|style|noscript|svg|template|head|nav|footer)\s*>").unwrap()
});
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap());
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z][A-Za-z'-]*").unwrap());
static SCRIPT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<script\b").unwrap());

const STOP: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "any", "can", "has", "had", "its", "one", "our", "out",
    "was", "who", "how", "why", "what", "when", "with", "this", "that", "from", "they", "them", "then", "than", "into",
    "onto", "each", "only", "also", "more", "most", "much", "very", "just", "your", "their", "there", "which", "while",
    "would", "could", "should", "about", "being", "does", "rather", "plus", "so", "a", "an", "is", "of", "to", "in",
    "on", "or", "by", "as", "at", "it", "be", "no", "if",
];

fn decode(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&rsquo;", "'")
}

/// The page's title and readable text: scripts, styles, navigation and
/// markup dropped.
pub fn readable(html: &str) -> (Option<String>, String) {
    let title = TITLE.captures(html).map(|c| decode(TAG.replace_all(&c[1], "").trim())).filter(|t| !t.is_empty());
    let body = DROP.replace_all(html, " ");
    let text = decode(&TAG.replace_all(&body, " "));
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (title, text)
}

/// A concept statement's key terms: its content words, lowercased.
pub fn key_terms(statement: &str) -> Vec<String> {
    let mut terms: Vec<String> = WORD
        .find_iter(statement)
        .map(|m| m.as_str().to_lowercase().trim_matches(|c| c == '\'' || c == '-').to_string())
        .filter(|w| w.len() >= 3 && !STOP.contains(&w.as_str()))
        .collect();
    terms.dedup();
    terms
}

/// Whether a fetched page can be offered for a concept.
pub fn verify(status: u16, content_type: &str, body: &str, concept: &str) -> Result<Verified, Rejected> {
    if !(200..300).contains(&status) {
        return Err(Rejected::Status(status));
    }
    let ct = content_type.to_lowercase();
    if !(ct.is_empty() || ct.contains("html") || ct.starts_with("text/")) {
        return Err(Rejected::NotAPage(content_type.to_string()));
    }
    let (title, text) = readable(body);
    match check_text(&text, concept) {
        Err(Rejected::TooShort(_)) if SCRIPT.is_match(body) => Err(Rejected::AppShell),
        Err(r) => Err(r),
        Ok(words) => Ok(Verified { title, words }),
    }
}

/// Whether some readable text (a page's, a transcript's) is enough to study
/// from and about the concept. Returns its word count.
pub fn check_text(text: &str, concept: &str) -> Result<usize, Rejected> {
    let words = WORD.find_iter(text).count();
    if words < MIN_WORDS {
        return Err(Rejected::TooShort(words));
    }
    let lower = text.to_lowercase();
    let terms = key_terms(concept);
    let found = terms.iter().filter(|t| lower.contains(t.as_str())).count();
    // At least half the key terms (at least one), so a page about something
    // else that shares a word or two doesn't pass.
    if found < terms.len().div_ceil(2).max(1) {
        return Err(Rejected::OffTopic);
    }
    Ok(words)
}

/// A piece of a page for the reader: a heading, a paragraph, a list item or
/// preformatted text (code).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Block {
    /// "h", "p", "li" or "pre".
    pub kind: String,
    pub text: String,
}

static BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<(h[1-6]|p|li|pre|blockquote)\b[^>]*>(.*?)</(?:h[1-6]|p|li|pre|blockquote)\s*>").unwrap()
});

/// A page's text as blocks, in order, for the reader (never its HTML).
pub fn blocks(html: &str) -> Vec<Block> {
    let body = DROP.replace_all(html, " ");
    let mut out: Vec<Block> = BLOCK
        .captures_iter(&body)
        .filter_map(|c| {
            let tag = c[1].to_lowercase();
            let kind = match tag.as_str() {
                t if t.starts_with('h') => "h",
                "li" => "li",
                "pre" => "pre",
                _ => "p",
            };
            let raw = decode(&TAG.replace_all(&c[2], ""));
            let text = if kind == "pre" {
                raw.trim_matches('\n').to_string()
            } else {
                raw.split_whitespace().collect::<Vec<_>>().join(" ")
            };
            (!text.trim().is_empty()).then(|| Block { kind: kind.into(), text })
        })
        .collect();
    if out.is_empty() {
        // No block tags (plain text): paragraphs from the readable text.
        let (_, text) = readable(html);
        if !text.is_empty() {
            out.push(Block { kind: "p".into(), text });
        }
    }
    out
}

// ---- Videos --------------------------------------------------------------------

/// A YouTube video's id, from a watch or short link.
pub fn youtube_id(url: &str) -> Option<String> {
    static ID: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^https?://(?:(?:www\.|m\.)?youtube\.com/(?:watch\?(?:.*&)?v=|shorts/|embed/)|youtu\.be/)([A-Za-z0-9_-]{6,})").unwrap()
    });
    ID.captures(url.trim()).map(|c| c[1].to_string())
}

/// A caption track to read: its URL, and whether it was generated
/// automatically (speech recognition) rather than written by a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptionTrack {
    pub url: String,
    pub automatic: bool,
}

/// The best caption track on a watch page: English written by a person,
/// then English generated, then any written, then anything.
pub fn caption_track(watch_html: &str) -> Option<CaptionTrack> {
    let at = watch_html.find("\"captionTracks\":")? + "\"captionTracks\":".len();
    let rest = &watch_html[at..];
    let start = rest.find('[')?;
    let mut depth = 0;
    let mut end = None;
    for (i, ch) in rest[start..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let tracks: Vec<serde_json::Value> = serde_json::from_str(&rest[start..end?]).ok()?;
    let english = |t: &&serde_json::Value| t["languageCode"].as_str().is_some_and(|l| l.starts_with("en"));
    let automatic = |t: &&serde_json::Value| t["kind"].as_str() == Some("asr");
    let pick = tracks
        .iter()
        .find(|t| english(t) && !automatic(t))
        .or_else(|| tracks.iter().find(|t| english(t)))
        .or_else(|| tracks.iter().find(|t| !automatic(t)))
        .or_else(|| tracks.first())?;
    Some(CaptionTrack { url: pick["baseUrl"].as_str()?.to_string(), automatic: automatic(&pick) })
}

/// A video's title from its watch page.
pub fn video_title(watch_html: &str) -> Option<String> {
    readable(watch_html).0.map(|t| t.trim_end_matches(" - YouTube").to_string())
}

/// One line of a transcript, and when it's said (seconds in).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Cue {
    pub start: f64,
    pub text: String,
}

/// A caption track's lines (YouTube's timed-text XML).
pub fn transcript(xml: &str) -> Vec<Cue> {
    static TEXT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?is)<text\b[^>]*\bstart="([\d.]+)"[^>]*>(.*?)</text>"#).unwrap());
    TEXT.captures_iter(xml)
        .filter_map(|c| {
            let text = decode(&decode(&c[2])).split_whitespace().collect::<Vec<_>>().join(" ");
            (!text.is_empty()).then(|| Cue { start: c[1].parse().unwrap_or(0.0), text })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn article(words: &str) -> String {
        format!(
            "<html><head><title>Tags &amp; digests</title><script>var x=1</script></head><body><nav>Home Docs</nav>\
             <article><h1>Tags</h1><p>{}</p></article></body></html>",
            words.repeat(40)
        )
    }

    #[test]
    fn a_readable_page_on_the_concept_passes() {
        let page = article("A tag is a movable name, while a digest is fixed to the content. ");
        let v = verify(200, "text/html; charset=utf-8", &page, "A tag is a movable name; a digest is fixed").unwrap();
        assert_eq!(v.title.as_deref(), Some("Tags & digests"));
        assert!(v.words > MIN_WORDS);
    }

    #[test]
    fn an_app_shell_is_rejected() {
        let shell = "<html><head><title>Docs</title></head><body><div id=\"root\"></div><script src=\"/app.js\"></script></body></html>";
        assert_eq!(verify(200, "text/html", shell, "A tag is a movable name"), Err(Rejected::AppShell));
    }

    #[test]
    fn off_topic_missing_and_not_a_page_are_rejected() {
        let garden = article("Tomatoes need sun, water and patience through the summer months. ");
        assert_eq!(
            verify(200, "text/html", &garden, "A tag is a movable name; a digest is fixed"),
            Err(Rejected::OffTopic)
        );
        assert_eq!(verify(404, "text/html", "", "x"), Err(Rejected::Status(404)));
        assert!(matches!(verify(200, "application/pdf", "", "x"), Err(Rejected::NotAPage(_))));
    }

    #[test]
    fn blocks_keep_structure_and_drop_the_rest() {
        let html = "<html><head><title>T</title><script>x()</script></head><body><nav><p>Menu</p></nav>\
            <h2>Tags</h2><p>A tag is a <b>movable</b> name.</p><ul><li>One</li><li>Two</li></ul>\
            <pre>docker pull api:2.4\n</pre></body></html>";
        let b = blocks(html);
        let kinds: Vec<&str> = b.iter().map(|b| b.kind.as_str()).collect();
        assert_eq!(kinds, ["h", "p", "li", "li", "pre"]);
        assert_eq!(b[1].text, "A tag is a movable name.");
        assert_eq!(b[4].text, "docker pull api:2.4");
    }

    #[test]
    fn youtube_links_and_caption_tracks() {
        assert_eq!(youtube_id("https://www.youtube.com/watch?v=abc123XYZ&t=3").as_deref(), Some("abc123XYZ"));
        assert_eq!(youtube_id("https://youtu.be/abc123XYZ").as_deref(), Some("abc123XYZ"));
        assert_eq!(youtube_id("https://example.com/watch?v=abc123XYZ"), None);
        let page = r#"var ytInitialPlayerResponse = {"captions":{"playerCaptionsTracklistRenderer":{"captionTracks":[
            {"baseUrl":"https://www.youtube.com/api/timedtext?v=x\u0026lang=en\u0026kind=asr","languageCode":"en","kind":"asr"},
            {"baseUrl":"https://www.youtube.com/api/timedtext?v=x\u0026lang=en","languageCode":"en"}]}}};"#;
        let t = caption_track(page).unwrap();
        assert_eq!(
            t,
            CaptionTrack { url: "https://www.youtube.com/api/timedtext?v=x&lang=en".into(), automatic: false }
        );
        assert_eq!(caption_track("<html>no captions here</html>"), None);
        let cues = transcript(
            r#"<transcript><text start="1.5" dur="2">A tag &amp;amp; a digest</text><text start="4" dur="1">moves</text></transcript>"#,
        );
        assert_eq!(
            cues,
            vec![Cue { start: 1.5, text: "A tag & a digest".into() }, Cue { start: 4.0, text: "moves".into() }]
        );
    }

    #[test]
    fn key_terms_skip_small_words() {
        assert_eq!(
            key_terms("A tag is a movable name; a digest is fixed"),
            ["tag", "movable", "name", "digest", "fixed"]
        );
    }
}
