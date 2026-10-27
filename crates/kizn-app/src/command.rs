use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Everything the UI can ask for. One variant per intent; each has exactly
/// one [`Reply`] variant (the pairing is spelled out in `ui/src/rpc.ts`).
#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Command {
    GetToday,
    GetAppInfo,
    GetSettings,
    /// Adds or changes a provider. `key`: absent keeps the current key, an
    /// empty string removes it, anything else goes to the keychain.
    SaveProvider {
        name: String,
        protocol: String,
        base_url: String,
        model: String,
        #[ts(optional)]
        key: Option<String>,
        /// Which Settings preset it came from ("openai", "ollama", …), for
        /// its logo and links. Nothing depends on it.
        #[ts(optional)]
        preset: Option<String>,
    },
    /// The models an endpoint offers, for the model picker. Uses `key` if
    /// given, else the saved key of provider `name`.
    ListModels {
        protocol: String,
        base_url: String,
        #[ts(optional)]
        key: Option<String>,
        #[ts(optional)]
        name: Option<String>,
    },
    RemoveProvider {
        name: String,
    },
    TestProvider {
        name: String,
    },
    AssignRole {
        role: Role,
        #[ts(optional)]
        provider: Option<String>,
    },
    /// Starts a draft from a goal; the author drafts it in the background.
    /// From a suggested foundation, it's linked underneath the topic it was
    /// for before drafting begins.
    StartTopic {
        title: String,
        goal: String,
        #[serde(default)]
        #[ts(optional)]
        foundation: Option<String>,
    },
    /// What to start a suggested foundation with: its name and a goal to
    /// edit, in the learner's words rather than kizn's.
    FoundationPrefill {
        suggestion_id: String,
    },
    StopDrafting {
        topic_id: String,
    },
    /// Carries on a draft that failed or was stopped, from where it left off.
    ResumeDrafting {
        topic_id: String,
    },
    /// Replaces a draft with the learner's shaped version.
    SaveDraft {
        topic_id: String,
        draft: DraftInput,
    },
    ConfirmTopic {
        topic_id: String,
    },
    GetTopic {
        topic_id: String,
    },
    /// Tries writing an outcome's items again now.
    RetryBank {
        outcome_id: String,
    },
    /// Builds a practice set: one topic's, or (no topic) across topics.
    StartPractice {
        #[ts(optional)]
        topic_id: Option<String>,
    },
    /// Answers a card: the option chosen, by its position in `order` (the
    /// stored options' indices, in the order they were shown).
    Answer {
        item_id: String,
        order: Vec<u32>,
        chosen: u32,
    },
    /// Where things stand after the answers given in a set.
    GetSummary {
        attempt_ids: Vec<String>,
    },
    /// Every concept's state and next recall, per outcome.
    GetProgress {
        topic_id: String,
    },
    /// A concept's page: its verified sources (kizn starts looking if it has
    /// none yet) and whether it's studied.
    GetConcept {
        concept_id: String,
    },
    /// Looks for sources again (after nothing could be checked).
    FindSources {
        concept_id: String,
    },
    MarkStudied {
        concept_id: String,
    },
    /// Links one topic above another: "rests_on" (the lower is a foundation
    /// of the upper) or "part_of" (the lower is a piece of the upper).
    /// Refused when it would make a loop.
    LinkTopics {
        upper_id: String,
        lower_id: String,
        kind: String,
    },
    Unlink {
        link_id: String,
    },
    /// Checking the ground for a confirmed topic: its kept foundations and
    /// their questions.
    GetGround {
        topic_id: String,
    },
    AnswerGround {
        item_id: String,
        order: Vec<u32>,
        chosen: u32,
    },
    /// What to do with a foundation: "later" (a suggestion on the map) or
    /// "dismiss" (not needed). Starting one is StartTopic with `foundation`.
    ResolveFoundation {
        suggestion_id: String,
        action: String,
    },
    /// The map: every topic and suggested foundation, and how they link.
    GetMap,
    /// Answers a flag: "add", "link" or "dismiss".
    ResolveFlag {
        for_topic: String,
        title: String,
        action: String,
    },
    /// Keeps or drops a foundation proposed for a draft.
    KeepFoundation {
        suggestion_id: String,
        keep: bool,
    },
    /// Every concept that needs work (missed, not yet recalled), with what
    /// there is to learn from: one topic's, or across topics. Starts looking
    /// for resources for any that have none.
    GetReadingList {
        #[ts(optional)]
        topic_id: Option<String>,
    },
    /// A resource to read in kizn: a page's text or a video's transcript.
    GetResource {
        source_id: String,
    },
    /// A concept's note (one per concept; empty removes it).
    SaveNote {
        concept_id: String,
        text: String,
    },
    /// The learner's own resource for a concept: fetched and checked like
    /// kizn's; refused with the reason when it can't be read.
    AddSource {
        concept_id: String,
        url: String,
    },
    /// "This key is wrong": the answer stops counting, the question is
    /// retired, and a replacement is written for its slot.
    Dispute {
        attempt_id: String,
    },
    /// Asks the tutor about an anchor: a new conversation, or `thread_id`'s.
    /// The reply streams as events.
    Ask {
        anchor: Anchor,
        #[ts(optional)]
        thread_id: Option<String>,
        text: String,
        /// A passage (or a video's moment) the question is about.
        #[ts(optional)]
        passage: Option<String>,
    },
    /// An anchor's conversations, newest first.
    GetThreads {
        anchor: Anchor,
    },
    GetThread {
        thread_id: String,
    },
}

/// A draft as the learner shaped it.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DraftInput {
    pub outcomes: Vec<DraftOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DraftOutcome {
    /// The outcome's id if it came from the draft; absent for a new one.
    #[ts(optional)]
    pub id: Option<String>,
    pub statement: String,
    /// A Bloom tier ("evaluate") or its plain words ("judge and recommend").
    pub target: String,
    pub reason: String,
    pub concepts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
#[ts(export)]
pub enum Reply {
    Today(TodayView),
    AppInfo(AppInfo),
    Settings(SettingsView),
    Models(Vec<String>),
    Topic(TopicView),
    Prefill(PrefillView),
    Deck(DeckView),
    Answered(AnswerView),
    Summary(SummaryView),
    Progress(ProgressView),
    Concept(ConceptPageView),
    Disputed(DisputeView),
    Thread(ThreadView),
    Resource(ResourceView),
    Ground(GroundView),
    Map(MapView),
    Done,
    ReadingList(ReadingListView),
    Note(NoteView),
    Threads(Vec<ThreadSummary>),
}

/// After a dispute: each concept the question named, back where it was
/// before the answer.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DisputeView {
    pub attempt_id: String,
    pub concepts: Vec<ConceptChange>,
}

/// A concept to study: what it says, where it sits, and the sources kizn
/// checked and could read. Only verified sources are ever listed.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConceptPageView {
    pub concept_id: String,
    pub statement: String,
    pub topic_id: String,
    pub topic_title: String,
    #[ts(type = "number")]
    pub outcome_ord: i64,
    /// "unseen", "seen", "recalled" or "retained".
    pub state: String,
    #[ts(type = "number | null")]
    pub next_due_in_days: Option<i64>,
    pub studied: bool,
    pub sources: Vec<SourceView>,
    /// "finding" (looking now), "found", "none" (looked; nothing could be
    /// checked), or "not_started".
    pub finding: String,
    /// How many suggested pages kizn has fetched for it, whatever came of it.
    #[ts(type = "number")]
    pub checked: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SourceView {
    pub id: String,
    /// "page" or "video".
    pub kind: String,
    pub url: String,
    pub title: String,
    pub why: Option<String>,
    /// The site, for the row ("docs.docker.com").
    pub host: String,
    #[ts(type = "number")]
    pub words: i64,
    /// A video's captions were generated, not written by a person.
    pub automatic: bool,
    /// The learner added it.
    pub own: bool,
}

/// The concepts that need work, and how far the studying has got.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReadingListView {
    pub concepts: Vec<ReadingItem>,
    #[ts(type = "number")]
    pub studied: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReadingItem {
    pub concept_id: String,
    pub statement: String,
    pub topic_id: String,
    pub topic_title: String,
    #[ts(type = "number")]
    pub outcome_ord: i64,
    pub studied: bool,
    /// Resources kizn holds for it, by kind.
    #[ts(type = "number")]
    pub pages: i64,
    #[ts(type = "number")]
    pub videos: i64,
    /// kizn is looking for resources now.
    pub finding: bool,
    pub has_note: bool,
}

/// A resource in kizn's reader: the concept it's for, and its text.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ResourceView {
    pub source: SourceView,
    pub concept_id: String,
    pub concept: String,
    pub topic_id: String,
    /// A page's text.
    pub blocks: Vec<BlockView>,
    /// A video's transcript.
    pub cues: Vec<CueView>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BlockView {
    /// "h", "p", "li" or "pre".
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CueView {
    /// Seconds into the video.
    pub start: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NoteView {
    pub concept_id: String,
    pub text: String,
}

/// A practice set: cards with their options in the order to show them. No
/// keys and no whys: those come back with the answer.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeckView {
    pub topic_id: Option<String>,
    pub cards: Vec<CardView>,
    /// The topic has never been practised: the first set (onboarding first).
    pub first_set: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CardView {
    pub item_id: String,
    /// "due" (a recall come back) or "new" (placement).
    pub reason: String,
    pub stem: String,
    pub options: Vec<String>,
    /// The stored options' indices, in the order shown; sent back with the answer.
    pub order: Vec<u32>,
    pub topic_title: String,
    #[ts(type = "number")]
    pub outcome_ord: i64,
    pub concept: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AnswerView {
    pub attempt_id: String,
    pub correct: bool,
    /// Positions in the order shown.
    pub picked: u32,
    pub key: u32,
    /// Every option, in the order shown, with its why.
    pub options: Vec<OptionFeedback>,
    pub source: Option<String>,
    pub model_knowledge: bool,
    pub concepts: Vec<ConceptChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OptionFeedback {
    pub text: String,
    pub correct: bool,
    pub why: String,
    pub misconception: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConceptChange {
    pub concept: String,
    /// "unseen", "seen", "recalled" or "retained".
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SummaryView {
    #[ts(type = "number")]
    pub answered: i64,
    #[ts(type = "number")]
    pub correct: i64,
    /// Concepts answered right this set, and wrong.
    pub held: Vec<ConceptLine>,
    pub slipped: Vec<ConceptLine>,
    /// The earliest next recall among the concepts touched.
    pub next_due: Option<String>,
    /// That, in days from now, rounded up (1 is tomorrow); 0 if it's due now.
    #[ts(type = "number | null")]
    pub next_due_in_days: Option<i64>,
    /// Every answer was a concept's first: a starting point.
    pub first_set: bool,
    pub outcomes: Vec<OutcomeStanding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConceptLine {
    pub concept_id: String,
    pub concept: String,
    /// Marked studied (for "learn these first").
    pub studied: bool,
    pub state: String,
}

/// An outcome's concepts by state, and its next recall.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OutcomeStanding {
    pub outcome_id: String,
    #[ts(type = "number")]
    pub ord: i64,
    pub statement: String,
    #[ts(type = "number")]
    pub concepts: i64,
    #[ts(type = "number")]
    pub unseen: i64,
    #[ts(type = "number")]
    pub recalled: i64,
    #[ts(type = "number")]
    pub retained: i64,
    pub next_due: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProgressView {
    pub topic_id: String,
    pub outcomes: Vec<OutcomeProgress>,
    #[ts(type = "number")]
    pub attempts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OutcomeProgress {
    pub standing: OutcomeStanding,
    pub concepts: Vec<ConceptProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConceptProgress {
    pub concept_id: String,
    pub statement: String,
    pub state: String,
    pub due: Option<String>,
    #[ts(type = "number")]
    pub answers: i64,
}

/// Today's one next step.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum NextStep {
    /// No topics yet.
    StartTopic,
    /// Only drafts: one is waiting to be shaped and confirmed.
    FinishSetup { topic_id: String, title: String },
    /// Questions are still being written for the first set.
    Preparing { topic_id: String, title: String },
    /// Something to practise now: recalls due and/or a starting point.
    Practise {
        #[ts(type = "number")]
        due: i64,
        #[ts(type = "number")]
        new: i64,
        /// A topic whose starting point this is (never practised before).
        first_set: Option<String>,
        /// The titles of the topics in the set, in order.
        topics: Vec<String>,
    },
    /// Nothing to practise, but concepts missed and not yet read up on.
    Learn {
        #[ts(type = "number")]
        count: i64,
        /// The first of them, to open its reading.
        concept_id: String,
        /// Days until the next recall.
        #[ts(type = "number | null")]
        next_due_in_days: Option<i64>,
    },
    /// Nothing to practise until then.
    NothingDue {
        next_at: Option<String>,
        #[ts(type = "number | null")]
        next_due_in_days: Option<i64>,
    },
}

/// A day of the learner's week on Today.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DayView {
    /// YYYY-MM-DD, in the learner's time.
    pub date: String,
    pub practised: bool,
    /// Today.
    pub today: bool,
    /// Later this week.
    pub future: bool,
}

/// Concepts by state, across topics.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StateCounts {
    #[ts(type = "number")]
    pub retained: i64,
    #[ts(type = "number")]
    pub recalled: i64,
    #[ts(type = "number")]
    pub to_learn: i64,
    #[ts(type = "number")]
    pub unseen: i64,
}

/// The map: topics and suggested foundations as nodes, links as edges.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MapView {
    pub nodes: Vec<MapNode>,
    pub edges: Vec<MapEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MapNode {
    /// A topic's id, or a suggestion's.
    pub id: String,
    pub title: String,
    /// "topic" or "suggestion".
    pub kind: String,
    /// A topic's status ("draft", "confirmed"); "suggested" for a suggestion.
    pub status: String,
    /// 0 with nothing below it.
    #[ts(type = "number")]
    pub layer: i64,
    /// A topic's concepts by state, for its fill.
    #[ts(type = "number")]
    pub concepts: i64,
    #[ts(type = "number")]
    pub retained: i64,
    #[ts(type = "number")]
    pub recalled: i64,
    #[ts(type = "number")]
    pub seen: i64,
    /// A suggestion's why, and the topic it's for.
    pub why: Option<String>,
    pub for_topic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MapEdge {
    pub upper: String,
    pub lower: String,
    /// "rests_on" or "part_of".
    pub kind: String,
    /// To a suggestion: drawn dashed.
    pub suggested: bool,
}

/// Misses that point at a foundation.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FlagView {
    pub for_topic: String,
    pub for_title: String,
    /// The foundation they point at.
    pub title: String,
    /// A topic the learner has, when it is one.
    pub topic_id: Option<String>,
    /// "new" (add it underneath), "link" (a topic they have, not linked yet)
    /// or "review" (already one of this topic's foundations).
    pub kind: String,
    #[ts(type = "number")]
    pub misses: i64,
}

/// Checking the ground for a topic.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GroundView {
    pub topic_id: String,
    pub topic_title: String,
    /// The questions are still being written.
    pub writing: bool,
    pub foundations: Vec<GroundFoundation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GroundFoundation {
    pub suggestion_id: String,
    pub title: String,
    pub why: Option<String>,
    /// The suggestion's status: "kept" (not checked yet), "solid" (checked,
    /// nothing to do), "later" (on the map to start with; a shaky one's
    /// default), "dismissed" or "started".
    pub status: String,
    /// "solid" or "shaky" once every question is answered.
    pub result: Option<String>,
    pub questions: Vec<GroundQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GroundQuestion {
    pub id: String,
    pub stem: String,
    pub options: Vec<String>,
    pub order: Vec<u32>,
    /// Once answered: the position picked, whether it was right, the key's
    /// position and every option's why.
    pub answered: Option<u32>,
    pub correct: Option<bool>,
    pub key: Option<u32>,
    pub feedback: Vec<OptionFeedback>,
}

/// The start of a topic set up from a suggested foundation.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PrefillView {
    pub suggestion_id: String,
    pub title: String,
    /// A goal to edit: why it matters, from the topic it was suggested for.
    pub goal: String,
    /// The topic it's a foundation for.
    pub for_title: String,
}

/// A foundation proposed for a topic.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SuggestionView {
    pub id: String,
    pub title: String,
    pub why: Option<String>,
    /// "draft", "check" or "flag".
    pub source: String,
    /// "open", "kept", "dropped", "later", "started", "linked" or "dismissed".
    pub status: String,
    /// The topic it is: one the learner has, or one started from it.
    pub topic_id: Option<String>,
    pub topic_title: Option<String>,
}

/// A topic at the other end of a link.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LinkedTopic {
    pub link_id: String,
    pub topic_id: String,
    pub title: String,
}

/// A topic, draft or confirmed, with its outcomes, concepts and bank.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TopicView {
    pub id: String,
    pub title: String,
    pub goal: String,
    /// "draft" or "confirmed".
    pub status: String,
    /// "drafting", "drafted", "stopped" or "failed".
    pub draft_state: String,
    pub draft_error: Option<String>,
    pub outcomes: Vec<OutcomeView>,
    /// A practice set can be built now (placement or due recalls).
    pub can_practise: bool,
    /// The topic has been practised before (its first set is done or begun).
    pub practised: bool,
    /// When nothing can be practised now: days until the next recall
    /// (rounded up; 1 is tomorrow).
    #[ts(type = "number | null")]
    pub next_due_in_days: Option<i64>,
    /// What this topic's next set holds: recalls due, and new concepts (a
    /// starting point) on outcome `new_outcome_ord`.
    #[ts(type = "number")]
    pub due_now: i64,
    #[ts(type = "number")]
    pub new_now: i64,
    #[ts(type = "number | null")]
    pub new_outcome_ord: Option<i64>,
    /// Its foundations, the topics it supports, its parts, and the broader
    /// topics it's part of.
    pub rests_on: Vec<LinkedTopic>,
    pub supports: Vec<LinkedTopic>,
    pub parts: Vec<LinkedTopic>,
    pub part_of: Vec<LinkedTopic>,
    /// Foundations proposed for it (from its draft, its check, its misses).
    pub foundations: Vec<SuggestionView>,
    /// Foundations its recent misses point at.
    pub flags: Vec<FlagView>,
    /// Its layer on the map: 0 with nothing below it.
    #[ts(type = "number")]
    pub layer: i64,
    /// Active items across the topic, and what a full bank holds.
    #[ts(type = "number")]
    pub active: i64,
    #[ts(type = "number")]
    pub expected: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OutcomeView {
    pub id: String,
    #[ts(type = "number")]
    pub ord: i64,
    pub statement: String,
    /// The Bloom tier, and what the learner reads ("judge and recommend").
    pub target: String,
    pub target_plain: String,
    pub reason: String,
    /// "pending", "drafting" or "done".
    pub concepts_state: String,
    /// "waiting", "writing", "ready", "resting" (nothing usable yet; kizn
    /// tries again by itself) or "failed" (something may need fixing).
    pub bank_state: String,
    pub bank_error: Option<String>,
    /// When a resting or failed outcome is tried again, in UTC.
    pub retry_at: Option<String>,
    #[ts(type = "number")]
    pub active: i64,
    #[ts(type = "number")]
    pub expected: i64,
    pub concepts: Vec<ConceptView>,
    /// The kinds of practice the outcome gets, in order, each with how many
    /// questions it has of how many it holds: one recall slot per concept,
    /// then use it, at the target (when above use it), explain it back.
    pub slots: Vec<SlotView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SlotView {
    /// "recall", "apply", "target" or "explain".
    pub kind: String,
    /// The concept, for a recall slot.
    pub concept: Option<String>,
    #[ts(type = "number")]
    pub have: i64,
    #[ts(type = "number")]
    pub size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConceptView {
    pub id: String,
    pub statement: String,
    /// "unseen", "seen", "recalled" or "retained" (unseen until practised).
    pub state: String,
    pub studied: bool,
}

/// What a provider is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Role {
    Author,
    Checker,
    Tutor,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Author, Role::Checker, Role::Tutor];

    pub fn key(self) -> &'static str {
        match self {
            Role::Author => "author",
            Role::Checker => "checker",
            Role::Tutor => "tutor",
        }
    }
}

/// Settings → Models.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsView {
    pub providers: Vec<ProviderView>,
    pub roles: RolesView,
    /// Advice and warnings, one line each.
    pub notes: Vec<String>,
    pub config_path: String,
    /// When config.toml can't be read, why (and providers is empty).
    pub config_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProviderView {
    pub name: String,
    pub preset: Option<String>,
    pub protocol: String,
    pub base_url: String,
    pub model: String,
    pub key: KeyView,
    /// The last test, if it still applies to this provider as configured.
    pub check: Option<CheckView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum KeyView {
    None,
    Keychain { present: bool },
    Env { var: String, present: bool },
    Refused { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CheckView {
    pub tested_at: String,
    pub reachable: bool,
    /// "native", "prompted" or "none".
    pub structured: String,
    pub streaming: bool,
    pub summary: String,
    pub problems: Vec<String>,
    /// The roles this provider can take, from this test.
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RolesView {
    pub author: Option<String>,
    pub checker: Option<String>,
    pub tutor: Option<String>,
}

/// Pushed to the UI (SSE in the browser loop, Tauri events in the window).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum Event {
    /// config.toml changed on disk (by kizn or by hand).
    ConfigChanged,
    /// A topic's draft or bank moved on: refetch it.
    TopicChanged { topic_id: String },
    /// More of the tutor's reply.
    TutorDelta { thread_id: String, text: String },
    /// The tutor's reply is finished (and kept), or failed.
    TutorDone { thread_id: String, error: Option<String> },
}

/// What a conversation with the tutor is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum Anchor {
    /// A practice question (as the learner last answered it).
    Card {
        item_id: String,
    },
    Concept {
        concept_id: String,
    },
    /// A resource (a page or a video) kizn holds as text.
    Resource {
        source_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ThreadView {
    pub id: String,
    /// The first question.
    pub title: String,
    pub anchor: Anchor,
    pub messages: Vec<MessageView>,
    /// The tutor is answering now (follow TutorDelta / TutorDone).
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MessageView {
    /// "learner" or "tutor".
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    #[ts(type = "number")]
    pub messages: i64,
    pub updated_at: String,
}

/// The home screen.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TodayView {
    pub topics: Vec<TopicSummary>,
    /// Whether any role (author, checker, tutor) points at a provider.
    pub model_configured: bool,
    pub next: NextStep,
    /// Today's date, in the learner's time (YYYY-MM-DD).
    pub date: String,
    /// Monday to Sunday, this week.
    pub week: Vec<DayView>,
    pub counts: StateCounts,
    /// Flags across topics.
    pub flags: Vec<FlagView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TopicSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    /// Some outcome's bank isn't ready yet.
    pub preparing: bool,
    #[ts(type = "number")]
    pub outcomes: i64,
    #[ts(type = "number")]
    pub concepts: i64,
    #[ts(type = "number")]
    pub active: i64,
    #[ts(type = "number")]
    pub expected: i64,
    /// Every concept's state, outcome by outcome: the topic card's marks.
    pub states: Vec<String>,
    #[ts(type = "number")]
    pub due_now: i64,
    #[ts(type = "number")]
    pub new_now: i64,
    pub practised: bool,
    #[ts(type = "number | null")]
    pub next_due_in_days: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub config_dir: String,
    pub schema_version: u32,
}

/// What went wrong, in words the UI can show as they are.
#[derive(Debug, Clone, Serialize, Deserialize, TS, thiserror::Error)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
#[ts(export)]
pub enum AppError {
    #[error("{0}")]
    Store(String),
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Io(String),
    /// A request kizn won't carry out, and why.
    #[error("{0}")]
    Refused(String),
}

impl From<kizn_store::StoreError> for AppError {
    fn from(e: kizn_store::StoreError) -> Self {
        AppError::Store(e.to_string())
    }
}
