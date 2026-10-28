//! Study, thin: for a concept the learner missed, the tutor suggests pages to
//! read, and kizn fetches each one to check it's real, readable and on the
//! concept before offering it (kizn_core::source). The model suggests; code
//! decides what's offered. Studying never changes a concept's state.

use std::time::Duration;

use kizn_core::progress::ConceptState;
use kizn_core::source::{
    Rejected, caption_track, check_text, page_blocks, transcript, verify, video_title, youtube_id,
};
use kizn_store::SourceRow;
use serde_json::{Value, json};

use crate::command::{
    BlockView, ConceptPageView, CueView, NoteView, ReadingItem, ReadingListView, ResourceView, Role, SourceView,
};
use crate::practice::days_until;
use crate::{AppError, Ctx};

/// Verified resources offered per concept.
pub const MAX_VERIFIED: usize = 4;
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;

const SUGGEST_SOURCES: &str = "kizn-task: suggest-sources
You suggest resources an adult learner can use to learn one concept, in any subject (history, a language, music, management, science, software…): web pages to read and YouTube videos to watch. kizn fetches every one and only offers those it can hold as text about the concept (a page's readable text, a video's captions), so only suggest resources you're confident exist at that exact URL.
- Prefer stable, well-known references for the subject, at the address you know best: an encyclopedia's article on it (Wikipedia, Britannica, the Stanford Encyclopedia of Philosophy…), open course material (Khan Academy, OpenStax, a university course page), the subject's own official sources where it has them (a standards body, a government agency, a museum or archive, a publisher, software documentation), and established educational explainers. A reference's main page on the concept beats a deep link you're unsure of; reputable sources beat blogs.
- Include a YouTube video when a demonstration, a performance or a lecture would suit the concept, from an established educational channel, and only if it has captions.
- No paywalled or login-only pages, no PDFs.
- Up to 6, each a different URL than those listed as tried (those failed: don't guess near them).
- \"why\": one line on what the learner gets from it.
Reply with JSON only: {\"sources\": [{\"url\": \"https://...\", \"title\": \"...\", \"why\": \"...\"}]}";

const FIND_SOURCES: &str = "kizn-task: find-sources
Search the web for resources that teach one concept well to one adult learner, in any subject: clear explanations, tutorials, open course material, encyclopedia articles, and YouTube talks or demonstrations (with captions). Not API references, changelogs, forum threads, paywalled or login-only pages, or PDFs. Search two or three times with different wording, then stop: kizn opens and checks each page you found itself.";

const JUDGE_SOURCE: &str = "kizn-task: judge-source
Would this page help this learner understand this concept? \"explains\": it explains the concept (what it is, how or why), rather than only mentioning it. \"fits\": false only when it's plainly not for learning, such as a reference listing (settings, system tables, API signatures, config snippets) with little explanation; technical depth is fine when it explains. Judge from the excerpts given; when in doubt, it fits.
Reply with JSON only: {\"explains\": true, \"fits\": true, \"why\": \"...\"}";

fn judge_schema() -> Value {
    json!({
        "type": "object",
        "properties": {"explains": {"type": "boolean"}, "fits": {"type": "boolean"}, "why": {"type": "string"}},
        "required": ["explains", "fits", "why"], "additionalProperties": false
    })
}

/// The stretches of a source's text that best match `query`.
pub(crate) fn excerpts_of(kind: &str, content: &str, query: &str, n: usize) -> Vec<String> {
    let chunks = if kind == "video" {
        let cues: Vec<kizn_core::source::Cue> = serde_json::from_str(content).unwrap_or_default();
        kizn_core::source::cue_chunks(&cues, 60.0)
    } else {
        let blocks: Vec<kizn_core::source::Block> = serde_json::from_str(content).unwrap_or_default();
        kizn_core::source::page_chunks(&blocks, 120)
    };
    kizn_core::source::relevant(&chunks, query, n).into_iter().map(|c| c.text).collect()
}

fn sources_schema() -> Value {
    json!({"type": "object", "properties": {"sources": {"type": "array", "items": {"type": "object"}}}, "required": ["sources"]})
}

fn host(url: &str) -> String {
    kizn_model::reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
        .unwrap_or_default()
}

/// What came of checking a resource: whether it can be offered, and the text
/// kizn keeps for reading it (a page's blocks, a video's transcript).
pub(crate) struct Checked {
    status: &'static str,
    reason: Option<String>,
    words: Option<i64>,
    title: Option<String>,
    kind: &'static str,
    content: Option<String>,
    automatic: bool,
}

impl Checked {
    fn failed(kind: &'static str, why: String) -> Checked {
        Checked { status: "failed", reason: Some(why), words: None, title: None, kind, content: None, automatic: false }
    }

    fn rejected(kind: &'static str, r: Rejected) -> Checked {
        Checked {
            status: "rejected",
            reason: Some(r.to_string()),
            words: None,
            title: None,
            kind,
            content: None,
            automatic: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        self,
        id: String,
        concept_id: &str,
        url: &str,
        title: Option<String>,
        why: Option<&str>,
        added_by: &str,
        at: String,
    ) -> SourceRow {
        SourceRow {
            id,
            concept_id: concept_id.into(),
            url: url.into(),
            title: title.or(self.title).unwrap_or_else(|| host(url)),
            why: why.map(str::to_string),
            status: self.status.into(),
            reason: self.reason,
            words: self.words,
            checked_at: at,
            kind: self.kind.into(),
            content_json: self.content,
            automatic: self.automatic,
            added_by: added_by.into(),
        }
    }
}

pub(crate) fn source_view(s: &SourceRow) -> SourceView {
    SourceView {
        id: s.id.clone(),
        kind: s.kind.clone(),
        url: s.url.clone(),
        title: s.title.clone(),
        why: s.why.clone(),
        host: host(&s.url),
        words: s.words.unwrap_or(0),
        automatic: s.automatic,
        own: s.added_by == "learner",
    }
}

impl Ctx {
    /// The concepts that need work (seen: missed and not yet recalled), in
    /// topic and outcome order, with what there is to learn from.
    pub(crate) fn reading_list(&self, topic_id: Option<&str>) -> Result<ReadingListView, AppError> {
        let topics: Vec<kizn_store::TopicRow> = match topic_id {
            Some(id) => vec![self.store.topic(id)?.ok_or_else(|| AppError::Refused("there's no such topic".into()))?],
            None => self.store.all_topics()?.into_iter().filter(|t| t.status == "confirmed").collect(),
        };
        let mut concepts = vec![];
        for t in topics {
            let standings = self.standings(&t.id)?;
            for o in self.store.outcomes(&t.id)? {
                for c in self.store.concepts(&o.id)? {
                    if standings.get(&c.id).map(|s| s.state) != Some(ConceptState::Seen) {
                        continue;
                    }
                    let sources = self.store.sources(&c.id)?;
                    if sources.is_empty() {
                        self.start_finding(&c.id);
                    }
                    let verified =
                        |kind: &str| sources.iter().filter(|s| s.status == "verified" && s.kind == kind).count() as i64;
                    concepts.push(ReadingItem {
                        studied: self.store.studied_at(&c.id)?.is_some(),
                        pages: verified("page"),
                        videos: verified("video"),
                        finding: self.sourcing.lock().unwrap().contains(&c.id),
                        has_note: self.store.note(&c.id)?.is_some(),
                        concept_id: c.id,
                        statement: c.statement,
                        topic_id: t.id.clone(),
                        topic_title: t.title.clone(),
                        outcome_ord: o.ord,
                    });
                }
            }
        }
        let studied = concepts.iter().filter(|c| c.studied).count() as i64;
        Ok(ReadingListView { concepts, studied })
    }

    /// A resource for the reader. One checked before kizn kept its text is
    /// fetched again now, and kept.
    pub(crate) async fn resource(&self, source_id: &str) -> Result<ResourceView, AppError> {
        let mut s =
            self.store.source(source_id)?.ok_or_else(|| AppError::Refused("there's no such resource".into()))?;
        if s.status != "verified" {
            return Err(AppError::Refused("kizn couldn't read that one".into()));
        }
        let concept =
            self.store.concept(&s.concept_id)?.ok_or_else(|| AppError::Refused("its concept is gone".into()))?;
        let outcome =
            self.store.outcome(&concept.outcome_id)?.ok_or_else(|| AppError::Refused("its outcome is gone".into()))?;
        // Text kept by an older kizn that couldn't parse it (a whole page as
        // one block, as Markdown pages came out) is read again.
        let unparsed = s.kind == "page"
            && s.content_json.as_deref().is_some_and(|c| {
                serde_json::from_str::<Vec<kizn_core::source::Block>>(c).is_ok_and(|b| {
                    b.len() <= 1 && b.iter().map(|b| b.text.split_whitespace().count()).sum::<usize>() > 300
                })
            });
        if s.content_json.is_none() || unparsed {
            let checked = self.check_resource(&s.url, &concept.statement).await;
            match checked.content {
                Some(content) => {
                    self.store.set_source_content(&s.id, &content)?;
                    s.content_json = Some(content);
                }
                None => {
                    return Err(AppError::Refused(format!(
                        "kizn couldn't read it again: {}",
                        checked.reason.unwrap_or_default()
                    )));
                }
            }
        }
        let content = s.content_json.as_deref().unwrap_or("[]");
        let (blocks, cues) = if s.kind == "video" {
            let cues: Vec<kizn_core::source::Cue> = serde_json::from_str(content).unwrap_or_default();
            (vec![], cues.into_iter().map(|c| CueView { start: c.start, text: c.text }).collect())
        } else {
            let blocks: Vec<kizn_core::source::Block> = serde_json::from_str(content).unwrap_or_default();
            (blocks.into_iter().map(|b| BlockView { kind: b.kind, text: b.text }).collect(), vec![])
        };
        Ok(ResourceView {
            source: source_view(&s),
            concept_id: concept.id.clone(),
            concept: concept.statement,
            topic_id: outcome.topic_id,
            blocks,
            cues,
            note: self.store.note(&concept.id)?,
        })
    }

    pub(crate) fn save_note(&self, concept_id: &str, text: &str) -> Result<NoteView, AppError> {
        self.store.concept(concept_id)?.ok_or_else(|| AppError::Refused("there's no such concept".into()))?;
        self.store.save_note(concept_id, text, &self.now())?;
        Ok(NoteView { concept_id: concept_id.into(), text: text.trim().to_string() })
    }

    /// The learner's own resource: fetched once and checked like kizn's.
    pub(crate) async fn add_source(&self, concept_id: &str, url: &str) -> Result<ConceptPageView, AppError> {
        let url = url.trim();
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(AppError::Refused(
                "that doesn't look like a web address (it should start with https://)".into(),
            ));
        }
        let concept =
            self.store.concept(concept_id)?.ok_or_else(|| AppError::Refused("there's no such concept".into()))?;
        if self.store.sources(concept_id)?.iter().any(|s| s.url == url && s.status == "verified") {
            return Err(AppError::Refused("that one is already here".into()));
        }
        let checked = self.check_resource(url, &concept.statement).await;
        let ok = checked.status == "verified";
        let reason = checked.reason.clone();
        self.store.insert_source(&checked.row(self.id(), concept_id, url, None, None, "learner", self.now()))?;
        if !ok {
            return Err(AppError::Refused(format!("kizn couldn't use it: {}", reason.unwrap_or_default())));
        }
        if let Ok(Some(o)) = self.store.outcome(&concept.outcome_id) {
            self.changed(&o.topic_id);
        }
        self.concept_page(concept_id)
    }

    /// Fetches a resource once and checks it can be studied from and is
    /// about the concept: a page's readable text, or a YouTube video's
    /// transcript (no captions, not offered).
    pub(crate) async fn check_resource(&self, url: &str, concept: &str) -> Checked {
        let fetch = |u: String| async move { self.deps.http.fetch_page(&u, FETCH_TIMEOUT, MAX_PAGE_BYTES).await };
        if youtube_id(url).is_some() {
            let page = match fetch(url.to_string()).await {
                Ok(p) if (200..300).contains(&p.status) => p,
                Ok(p) => return Checked::rejected("video", Rejected::Status(p.status)),
                Err(e) => return Checked::failed("video", format!("unreachable: {e}")),
            };
            let Some(track) = caption_track(&page.body) else {
                return Checked::rejected("video", Rejected::NoTranscript);
            };
            let cues = match fetch(track.url.clone()).await {
                Ok(t) if (200..300).contains(&t.status) => transcript(&t.body),
                Ok(_) | Err(_) => vec![],
            };
            if cues.is_empty() {
                return Checked::rejected("video", Rejected::NoTranscript);
            }
            let text = cues.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join(" ");
            return match check_text(&text, concept) {
                Err(r) => Checked::rejected("video", r),
                Ok(words) => Checked {
                    status: "verified",
                    reason: None,
                    words: Some(words as i64),
                    title: video_title(&page.body),
                    kind: "video",
                    content: serde_json::to_string(&cues).ok(),
                    automatic: track.automatic,
                },
            };
        }
        match fetch(url.to_string()).await {
            Err(e) => Checked::failed("page", format!("unreachable: {e}")),
            Ok(page) => match verify(page.status, &page.content_type, &page.body, concept) {
                Err(r) => Checked::rejected("page", r),
                Ok(v) => Checked {
                    status: "verified",
                    reason: None,
                    words: Some(v.words as i64),
                    title: v.title,
                    kind: "page",
                    content: serde_json::to_string(&page_blocks(&page.content_type, &page.body)).ok(),
                    automatic: false,
                },
            },
        }
    }

    pub(crate) fn concept_page(&self, concept_id: &str) -> Result<ConceptPageView, AppError> {
        let concept =
            self.store.concept(concept_id)?.ok_or_else(|| AppError::Refused("there's no such concept".into()))?;
        let outcome = self
            .store
            .outcome(&concept.outcome_id)?
            .ok_or_else(|| AppError::Refused("that concept's outcome is gone".into()))?;
        let topic = self
            .store
            .topic(&outcome.topic_id)?
            .ok_or_else(|| AppError::Refused("that concept's topic is gone".into()))?;
        let rows = self.store.sources(concept_id)?;
        let verified: Vec<SourceView> = rows.iter().filter(|s| s.status == "verified").map(source_view).collect();
        let finding = if self.sourcing.lock().unwrap().contains(concept_id) {
            "finding"
        } else if !verified.is_empty() {
            "found"
        } else if rows.is_empty() {
            "not_started"
        } else {
            "none"
        };
        let standing = self.standing_of_concept(&topic.id, concept_id)?;
        let now = self.deps.clock.now();
        let (lecture_state, lecture, lecture_reason) = self.lecture_view(concept_id)?;
        Ok(ConceptPageView {
            lecture_state,
            lecture,
            lecture_reason,
            concept_id: concept.id,
            statement: concept.statement,
            topic_id: topic.id,
            topic_title: topic.title,
            outcome_ord: outcome.ord,
            state: standing.state.name().into(),
            next_due_in_days: standing.due.map(|d| days_until(d, now)),
            studied: self.store.studied_at(concept_id)?.is_some(),
            sources: verified,
            finding: finding.into(),
            checked: rows.iter().filter(|s| !s.url.is_empty()).count() as i64,
            note: self.store.note(concept_id)?,
        })
    }

    /// The page, starting what it needs: the search for sources if there's
    /// nothing yet (its lecture follows), or the lecture if the sources are
    /// there and it isn't. Written when it's opened, not before.
    pub(crate) fn open_concept(&self, concept_id: &str) -> Result<ConceptPageView, AppError> {
        let page = self.concept_page(concept_id)?;
        if page.finding == "not_started" {
            self.start_finding(concept_id);
            return self.concept_page(concept_id);
        }
        if page.finding == "found" && page.lecture_state == "none" {
            self.start_lecture(concept_id);
            return self.concept_page(concept_id);
        }
        Ok(page)
    }

    /// Starts the lecture over: kizn's sources set aside (the learner's own
    /// kept), the old lecture gone, then a fresh search and a new lecture.
    pub(crate) fn rewrite_lecture(&self, concept_id: &str) -> Result<ConceptPageView, AppError> {
        if self.sourcing.lock().unwrap().contains(concept_id) || self.lecturing.lock().unwrap().contains(concept_id) {
            return Err(AppError::Refused("kizn is already working on this one's reading".into()));
        }
        self.store.forget_lecture(concept_id)?;
        self.start_finding(concept_id);
        self.concept_page(concept_id)
    }

    /// Looks again for sources; the lecture is written again from them.
    pub(crate) fn find_again(&self, concept_id: &str) -> Result<ConceptPageView, AppError> {
        self.start_finding(concept_id);
        self.concept_page(concept_id)
    }

    pub(crate) fn mark_studied(&self, concept_id: &str) -> Result<ConceptPageView, AppError> {
        let page = self.concept_page(concept_id)?;
        self.store.mark_studied(concept_id, &self.now())?;
        self.changed(&page.topic_id);
        self.concept_page(concept_id)
    }

    /// Looks for sources in the background, once at a time per concept.
    pub(crate) fn start_finding(&self, concept_id: &str) {
        if !self.sourcing.lock().unwrap().insert(concept_id.to_string()) {
            return;
        }
        let ctx = self.clone();
        let id = concept_id.to_string();
        tokio::spawn(async move {
            if let Err(e) = ctx.find_sources(&id).await {
                eprintln!("kizn: finding sources: {e}");
            }
            ctx.sourcing.lock().unwrap().remove(&id);
            // Looked for because it was opened or missed: its lecture next.
            ctx.start_lecture(&id);
            if let Ok(Some(c)) = ctx.store.concept(&id)
                && let Ok(Some(o)) = ctx.store.outcome(&c.outcome_id)
            {
                ctx.changed(&o.topic_id);
            }
        });
    }

    /// For a concept just missed with nothing to read yet: start looking, so
    /// the sources are there by the time the summary is.
    pub(crate) fn find_if_missed(&self, concept_id: &str, state: ConceptState) {
        if state == ConceptState::Seen && self.store.sources(concept_id).is_ok_and(|s| s.is_empty()) {
            self.start_finding(concept_id);
        }
    }

    /// Looks for resources: with a web search where a provider has one (a
    /// model's memory of URLs is often stale or off), then, if that's not
    /// enough, suggested from memory, with a second round told what failed.
    async fn find_sources(&self, concept_id: &str) -> Result<(), AppError> {
        if let Some(searcher) = self.searcher() {
            self.search_round(concept_id, &searcher).await?;
        }
        for _ in 0..2 {
            if self.suggest_round(concept_id).await? > 0 {
                break;
            }
            let verified = self.store.sources(concept_id)?.iter().filter(|s| s.status == "verified").count();
            if verified >= MAX_VERIFIED {
                break;
            }
        }
        Ok(())
    }

    /// A role's model whose provider can search the web: the tutor's, else
    /// the author's, else the checker's.
    fn searcher(&self) -> Option<crate::topics::RoleModel> {
        [Role::Tutor, Role::Author, Role::Checker]
            .into_iter()
            .filter_map(|r| self.role_model(r).ok())
            .find(|m| m.client.can_search())
    }

    /// One search: the model searches, kizn checks and judges what it found.
    /// Returns how many passed.
    async fn search_round(&self, concept_id: &str, searcher: &crate::topics::RoleModel) -> Result<usize, AppError> {
        let concept = self.store.concept(concept_id)?.ok_or_else(|| AppError::Refused("no such concept".into()))?;
        let outcome = self.store.outcome(&concept.outcome_id)?.ok_or_else(|| AppError::Refused("no outcome".into()))?;
        let topic = self.store.topic(&outcome.topic_id)?.ok_or_else(|| AppError::Refused("no topic".into()))?;
        let tried = self.store.sources(concept_id)?;
        let mut verified = tried.iter().filter(|s| s.status == "verified").count();
        let before = verified;
        if verified >= MAX_VERIFIED {
            return Ok(0);
        }
        let ask = format!(
            "Find resources to learn this concept: \"{}\".\n\n<input>\n{}\n</input>",
            concept.statement,
            json!({ "topic": topic.title, "goal": topic.goal, "outcome": outcome.statement, "concept": concept.statement })
        );
        let found = match searcher.client.search(FIND_SOURCES, &ask, 3).await {
            Ok((found, _)) => found,
            Err(e) => {
                eprintln!("kizn: searching for sources: {e}");
                return Ok(0);
            }
        };
        for f in found.iter().take(10) {
            if verified >= MAX_VERIFIED {
                break;
            }
            if tried.iter().any(|t| t.url == f.url) || !(f.url.starts_with("https://") || f.url.starts_with("http://"))
            {
                continue;
            }
            let title = Some(f.title.clone()).filter(|t| !t.trim().is_empty());
            if self.check_and_keep(concept_id, &f.url, title, None).await? {
                verified += 1;
            }
        }
        Ok(verified - before)
    }

    /// Fetches and checks a resource, has the checker judge whether it
    /// explains the concept at the learner's level, and keeps the result.
    /// Returns whether it can be offered.
    async fn check_and_keep(
        &self,
        concept_id: &str,
        url: &str,
        title: Option<String>,
        why: Option<&str>,
    ) -> Result<bool, AppError> {
        let concept = self.store.concept(concept_id)?.ok_or_else(|| AppError::Refused("no such concept".into()))?;
        let mut checked = self.check_resource(url, &concept.statement).await;
        let title = title.or(checked.title.clone());
        if checked.status == "verified"
            && let Some(why_not) = self.judged_unfit(concept_id, url, title.as_deref(), &checked).await
        {
            checked.status = "rejected";
            checked.reason = Some(format!("not a fit: {why_not}"));
        }
        let ok = checked.status == "verified";
        self.store.insert_source(&checked.row(self.id(), concept_id, url, title, why, "kizn", self.now()))?;
        Ok(ok)
    }

    /// Why the checker finds a page won't help this learner with this
    /// concept, or nothing if it will (or can't be asked).
    async fn judged_unfit(
        &self,
        concept_id: &str,
        url: &str,
        title: Option<&str>,
        checked: &Checked,
    ) -> Option<String> {
        let concept = self.store.concept(concept_id).ok()??;
        let outcome = self.store.outcome(&concept.outcome_id).ok()??;
        let topic = self.store.topic(&outcome.topic_id).ok()??;
        let checker = self.role_model(Role::Checker).ok()?;
        let excerpts = excerpts_of(checked.kind, checked.content.as_deref()?, &concept.statement, 3);
        let ask = format!(
            "<input>\n{}\n</input>",
            json!({ "goal": topic.goal, "outcome": outcome.statement, "concept": concept.statement,
                    "url": url, "title": title, "excerpts": excerpts })
        );
        let (reply, _) = checker.client.complete_json(JUDGE_SOURCE, &ask, &judge_schema(), checker.mode).await.ok()?;
        let fits = reply["explains"].as_bool().unwrap_or(true) && reply["fits"].as_bool().unwrap_or(true);
        (!fits).then(|| reply["why"].as_str().unwrap_or("the checker says it won't help").to_string())
    }

    /// One round: the tutor suggests, kizn checks. Returns how many passed.
    async fn suggest_round(&self, concept_id: &str) -> Result<usize, AppError> {
        let concept = self.store.concept(concept_id)?.ok_or_else(|| AppError::Refused("no such concept".into()))?;
        let outcome = self.store.outcome(&concept.outcome_id)?.ok_or_else(|| AppError::Refused("no outcome".into()))?;
        let topic = self.store.topic(&outcome.topic_id)?.ok_or_else(|| AppError::Refused("no topic".into()))?;
        let tried = self.store.sources(concept_id)?;
        let mut verified = tried.iter().filter(|s| s.status == "verified").count();
        let before = verified;
        if verified >= MAX_VERIFIED {
            return Ok(0);
        }
        let tutor = self.role_model(Role::Tutor).or_else(|_| self.role_model(Role::Author))?;
        let ask = format!(
            "Suggest up to 6 resources to learn this concept: \"{}\".\n\n<input>\n{}\n</input>",
            concept.statement,
            json!({
                "topic": topic.title, "goal": topic.goal, "outcome": outcome.statement, "concept": concept.statement,
                "tried": tried.iter().map(|s| s.url.as_str()).filter(|u| !u.is_empty()).collect::<Vec<_>>(),
            })
        );
        let reply = match tutor.client.complete_json(SUGGEST_SOURCES, &ask, &sources_schema(), tutor.mode).await {
            Ok((reply, _)) => reply,
            Err(e) => {
                // Recorded so the page can say it couldn't look, not "nothing found".
                self.store.insert_source(&SourceRow {
                    id: self.id(),
                    concept_id: concept_id.into(),
                    url: String::new(),
                    title: String::new(),
                    why: None,
                    status: "failed".into(),
                    reason: Some(format!("the tutor couldn't suggest any: {e}")),
                    words: None,
                    checked_at: self.now(),
                    kind: "page".into(),
                    content_json: None,
                    automatic: false,
                    added_by: "kizn".into(),
                })?;
                return Ok(0);
            }
        };
        let candidates = reply["sources"].as_array().cloned().unwrap_or_default();
        for c in candidates.iter().take(6) {
            if verified >= MAX_VERIFIED {
                break;
            }
            let Some(url) =
                c["url"].as_str().map(str::trim).filter(|u| u.starts_with("https://") || u.starts_with("http://"))
            else {
                continue;
            };
            if tried.iter().any(|t| t.url == url) {
                continue;
            }
            let title = c["title"].as_str().map(str::to_string).filter(|t| !t.trim().is_empty());
            if self.check_and_keep(concept_id, url, title, c["why"].as_str()).await? {
                verified += 1;
            }
        }
        Ok(verified - before)
    }
}
