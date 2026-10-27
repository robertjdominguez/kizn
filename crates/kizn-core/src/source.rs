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
    let words = WORD.find_iter(&text).count();
    if words < MIN_WORDS {
        return Err(if SCRIPT.is_match(body) { Rejected::AppShell } else { Rejected::TooShort(words) });
    }
    let lower = text.to_lowercase();
    let terms = key_terms(concept);
    let found = terms.iter().filter(|t| lower.contains(t.as_str())).count();
    // At least half the key terms (at least one), so a page about something
    // else that shares a word or two doesn't pass.
    if found < terms.len().div_ceil(2).max(1) {
        return Err(Rejected::OffTopic);
    }
    Ok(Verified { title, words })
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
    fn key_terms_skip_small_words() {
        assert_eq!(
            key_terms("A tag is a movable name; a digest is fixed"),
            ["tag", "movable", "name", "digest", "fixed"]
        );
    }
}
