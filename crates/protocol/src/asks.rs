//! The host's asks (`read-and-estimate` design decision 3): a request to read something and a
//! vote on how big a task is. The host asks, everyone answers, and the host sees who answered.
//!
//! These types travel in postcard control messages and, inside timeline entries and the session
//! process's events, as JSON; so every enum is externally tagged. Postcard ignores the names.

use serde::{Deserialize, Serialize};

/// Longest question text accepted with a read answer, in characters.
pub const MAX_QUESTION_CHARS: usize = 280;

/// Longest task accepted for a vote, in characters.
pub const MAX_TASK_CHARS: usize = 300;

/// Longest label of what to read, in characters.
pub const MAX_READ_LABEL_CHARS: usize = 255;

/// The time limits a read request may have, in seconds; 0 is no limit.
pub const READ_LIMITS_S: [u16; 4] = [0, 120, 300, 600];

/// What the host asks everyone to read. Paths are relative to the session folder, as in
/// `FileChange` entries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum ReadTarget {
    /// A Markdown document the agent created or changed.
    Document { path: String },
    /// A code file the agent changed.
    Code { path: String },
    /// A file shared in the session, by its id.
    Shared { file: u32 },
    /// Every file change after the timeline event `from_seq` (the host's last prompt) up to and
    /// including `to_seq` (the newest event when asked), so the range is fixed.
    Changes { from_seq: u64, to_seq: u64 },
}

impl ReadTarget {
    /// The kind's fixed name, for diagnostics; never the path.
    pub fn kind(&self) -> &'static str {
        match self {
            ReadTarget::Document { .. } => "document",
            ReadTarget::Code { .. } => "code",
            ReadTarget::Shared { .. } => "shared",
            ReadTarget::Changes { .. } => "changes",
        }
    }
}

/// The cards the host picked for a vote. Both sets also have ? and ☕.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CardSet {
    StoryPoints,
    TShirt,
}

impl CardSet {
    /// The set's value cards, smallest first; `Card::Value` indexes into it.
    pub fn cards(self) -> &'static [&'static str] {
        match self {
            CardSet::StoryPoints => &["1", "2", "3", "5", "8", "13"],
            CardSet::TShirt => &["XS", "S", "M", "L", "XL"],
        }
    }

    /// Whether `card` belongs to this set.
    pub fn accepts(self, card: Card) -> bool {
        match card {
            Card::Value(index) => usize::from(index) < self.cards().len(),
            Card::Unsure | Card::Break => true,
        }
    }

    /// The set's fixed name, for diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            CardSet::StoryPoints => "story_points",
            CardSet::TShirt => "t_shirt",
        }
    }
}

/// The most value cards any set has: a larger index is malformed whatever the set.
pub const MAX_SET_CARDS: usize = 6;

/// One card of a set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Card {
    /// The set's value card at this index.
    Value(u8),
    /// ? : no idea.
    Unsure,
    /// ☕ : I need a break.
    Break,
}

impl Card {
    /// What the card shows, in `set`.
    pub fn label(self, set: CardSet) -> &'static str {
        match self {
            Card::Value(index) => set.cards().get(usize::from(index)).copied().unwrap_or("?"),
            Card::Unsure => "?",
            Card::Break => "☕",
        }
    }
}

/// A viewer's answer to a read request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadAnswer {
    /// Not answered yet, or the answer taken back.
    Reading,
    Ready,
    /// A question, with a text only the host sees.
    Question {
        text: Option<String>,
    },
}

/// How everyone sees a participant during a read request; a question's text is not in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadState {
    Reading,
    Ready,
    Question,
    /// On an app of the previous protocol, which can't answer.
    OlderApp,
}

impl From<&ReadAnswer> for ReadState {
    fn from(answer: &ReadAnswer) -> Self {
        match answer {
            ReadAnswer::Reading => ReadState::Reading,
            ReadAnswer::Ready => ReadState::Ready,
            ReadAnswer::Question { .. } => ReadState::Question,
        }
    }
}

/// The open read request as one participant sees it. Camel case for the windows, which get it
/// as JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadView {
    pub id: u32,
    pub target: ReadTarget,
    /// What to show for the target, such as a file's name.
    pub label: String,
    /// 0 for no limit.
    pub limit_s: u16,
    /// Time left when this was sent; `None` without a limit. Each window counts down from it.
    pub left_ms: Option<u64>,
    /// Every participant present who is asked, by participant id, the host left out.
    pub people: Vec<(u32, ReadState)>,
    /// This participant's own answer, with their own question's text.
    pub mine: ReadAnswer,
    /// The question texts by participant id: only in the host's own view, empty for viewers.
    #[serde(default)]
    pub questions: Vec<(u32, String)>,
}

/// The open vote as one participant sees it. No value but the participant's own is in it
/// before the reveal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteView {
    pub id: u32,
    /// From 1; *Vote again* starts the next.
    pub round: u16,
    pub task: String,
    pub cards: CardSet,
    /// Who has voted this round, by participant id, 0 for the host.
    pub voted: Vec<u32>,
    pub mine: Option<Card>,
    /// Every vote by participant id, once the host revealed them.
    pub revealed: Option<Vec<(u32, Card)>>,
    /// Participants on an app of the previous protocol, which can't vote.
    pub older: Vec<u32>,
}

/// A viewer's answer to one of the host's asks, sent as a request so it gets a `Reply`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum Answer {
    Read {
        ask: u32,
        answer: ReadAnswer,
    },
    /// `None` takes the vote back.
    Vote {
        ask: u32,
        round: u16,
        card: Option<Card>,
    },
}

/// Whether `text` is a question text the host accepts: at most [`MAX_QUESTION_CHARS`]
/// characters and no control characters but line breaks.
pub fn valid_question(text: &str) -> bool {
    text.chars().count() <= MAX_QUESTION_CHARS && !text.chars().any(|c| c.is_control() && c != '\n')
}

/// Whether `task` is a task the host can put to a vote: 1 to [`MAX_TASK_CHARS`] characters, not
/// blank, and no control characters but line breaks and tabs.
pub fn valid_task(task: &str) -> bool {
    !task.trim().is_empty()
        && task.chars().count() <= MAX_TASK_CHARS
        && !task.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}

/// Whether `label` can name what to read: 1 to [`MAX_READ_LABEL_CHARS`] characters, not blank,
/// no control characters.
pub fn valid_read_label(label: &str) -> bool {
    !label.trim().is_empty()
        && label.chars().count() <= MAX_READ_LABEL_CHARS
        && !label.chars().any(char::is_control)
}

/// Whether an answer from a viewer is well formed, whatever ask it names: a question within
/// bounds and a card any set could have. The host checks the card against the vote's own set.
pub fn valid_answer(answer: &Answer) -> bool {
    match answer {
        Answer::Read { answer: ReadAnswer::Question { text: Some(text) }, .. } => {
            valid_question(text)
        }
        Answer::Read { .. } => true,
        Answer::Vote { card: Some(Card::Value(index)), .. } => usize::from(*index) < MAX_SET_CARDS,
        Answer::Vote { .. } => true,
    }
}

/// The card to propose keeping: the most common value card, the larger on a tie; ? or ☕ only
/// when no vote is a value card, ? before ☕ on a tie. `None` without votes.
pub fn proposed(votes: &[Card]) -> Option<Card> {
    let mut counts: Vec<(Card, usize)> = Vec::new();
    for &card in votes {
        match counts.iter_mut().find(|(c, _)| *c == card) {
            Some((_, n)) => *n += 1,
            None => counts.push((card, 1)),
        }
    }
    let rank = |card: Card| match card {
        Card::Value(index) => (2, index),
        Card::Unsure => (1, 0),
        Card::Break => (0, 0),
    };
    counts
        .into_iter()
        .max_by_key(|&(card, n)| {
            let (kind, index) = rank(card);
            (kind == 2, n, kind, index)
        })
        .map(|(card, _)| card)
}

/// The lowest and highest value card among `votes`, in the set's order; `None` when no vote is
/// a value card.
pub fn spread(votes: &[Card]) -> Option<(Card, Card)> {
    let values = votes.iter().filter_map(|card| match card {
        Card::Value(index) => Some(*index),
        _ => None,
    });
    let (low, high) = values.fold(None, |range: Option<(u8, u8)>, v| {
        Some(range.map_or((v, v), |(lo, hi)| (lo.min(v), hi.max(v))))
    })?;
    Some((Card::Value(low), Card::Value(high)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_view() -> ReadView {
        ReadView {
            id: 1,
            target: ReadTarget::Document { path: "openspec/changes/x/design.md".into() },
            label: "design.md".into(),
            limit_s: 300,
            left_ms: Some(299_500),
            people: vec![(1, ReadState::Ready), (2, ReadState::Question), (3, ReadState::OlderApp)],
            mine: ReadAnswer::Question { text: Some("Why not reuse the bucket?".into()) },
            questions: vec![],
        }
    }

    fn vote_view() -> VoteView {
        VoteView {
            id: 2,
            round: 1,
            task: "Show unreachable after three missed heartbeats".into(),
            cards: CardSet::TShirt,
            voted: vec![0, 2],
            mine: Some(Card::Value(2)),
            revealed: Some(vec![(0, Card::Value(2)), (2, Card::Break)]),
            older: vec![3],
        }
    }

    #[test]
    fn views_round_trip_in_postcard_and_json() {
        let read = read_view();
        let bytes = postcard::to_allocvec(&read).unwrap();
        assert_eq!(postcard::from_bytes::<ReadView>(&bytes).unwrap(), read);
        let vote = vote_view();
        let bytes = postcard::to_allocvec(&vote).unwrap();
        assert_eq!(postcard::from_bytes::<VoteView>(&bytes).unwrap(), vote);
        for target in [
            ReadTarget::Code { path: "src/a.rs".into() },
            ReadTarget::Shared { file: 3 },
            ReadTarget::Changes { from_seq: 4, to_seq: 9 },
        ] {
            let bytes = postcard::to_allocvec(&target).unwrap();
            assert_eq!(postcard::from_bytes::<ReadTarget>(&bytes).unwrap(), target);
        }
    }

    #[test]
    fn json_for_the_windows() {
        assert_eq!(
            serde_json::to_value(read_view()).unwrap(),
            serde_json::json!({
                "id": 1,
                "target": { "document": { "path": "openspec/changes/x/design.md" } },
                "label": "design.md",
                "limitS": 300,
                "leftMs": 299_500,
                "people": [[1, "ready"], [2, "question"], [3, "older-app"]],
                "mine": { "question": { "text": "Why not reuse the bucket?" } },
                "questions": [],
            })
        );
        assert_eq!(
            serde_json::to_value(vote_view()).unwrap(),
            serde_json::json!({
                "id": 2,
                "round": 1,
                "task": "Show unreachable after three missed heartbeats",
                "cards": "t-shirt",
                "voted": [0, 2],
                "mine": { "value": 2 },
                "revealed": [[0, { "value": 2 }], [2, "break"]],
                "older": [3],
            })
        );
        let changes = ReadTarget::Changes { from_seq: 4, to_seq: 9 };
        let json = serde_json::json!({ "changes": { "fromSeq": 4, "toSeq": 9 } });
        assert_eq!(serde_json::to_value(&changes).unwrap(), json);
        assert_eq!(serde_json::from_value::<ReadTarget>(json).unwrap(), changes);
        let answer = Answer::Vote { ask: 2, round: 1, card: None };
        let json = serde_json::json!({ "vote": { "ask": 2, "round": 1, "card": null } });
        assert_eq!(serde_json::to_value(&answer).unwrap(), json);
    }

    #[test]
    fn cards_belong_to_their_set() {
        assert_eq!(CardSet::StoryPoints.cards().len(), MAX_SET_CARDS);
        assert!(CardSet::StoryPoints.accepts(Card::Value(5)));
        assert!(!CardSet::TShirt.accepts(Card::Value(5)), "t-shirt sizes stop at XL");
        assert!(CardSet::TShirt.accepts(Card::Break));
        assert_eq!(Card::Value(5).label(CardSet::StoryPoints), "13");
        assert_eq!(Card::Value(2).label(CardSet::TShirt), "M");
        assert_eq!(Card::Break.label(CardSet::TShirt), "☕");
    }

    #[test]
    fn answers_are_checked() {
        let question = |text: &str| Answer::Read {
            ask: 1,
            answer: ReadAnswer::Question { text: Some(text.into()) },
        };
        assert!(valid_answer(&question("Why not reuse\nthe bucket?")));
        assert!(valid_answer(&question(&"é".repeat(MAX_QUESTION_CHARS))));
        assert!(!valid_answer(&question(&"a".repeat(MAX_QUESTION_CHARS + 1))));
        assert!(!valid_answer(&question("tab\there")));
        assert!(!valid_answer(&question("run\r")));
        assert!(!valid_answer(&question("a\u{1b}[2J")));
        let vote = |card| Answer::Vote { ask: 1, round: 1, card: Some(card) };
        assert!(valid_answer(&vote(Card::Value(5))));
        assert!(!valid_answer(&vote(Card::Value(6))));
        assert!(valid_answer(&vote(Card::Unsure)));
    }

    #[test]
    fn tasks_and_labels() {
        assert!(valid_task("Show unreachable after three missed heartbeats"));
        assert!(valid_task(&"é".repeat(MAX_TASK_CHARS)));
        assert!(!valid_task(&"a".repeat(MAX_TASK_CHARS + 1)));
        assert!(!valid_task("  "));
        assert!(!valid_task("bell\u{7}"));
        assert!(valid_read_label("design.md"));
        assert!(!valid_read_label("two\nlines"));
    }

    #[test]
    fn the_proposed_card() {
        let m = Card::Value(2);
        let l = Card::Value(3);
        assert_eq!(proposed(&[m, l, m, Card::Break]), Some(m));
        assert_eq!(
            proposed(&[Card::Value(3), Card::Value(3), Card::Value(4), Card::Value(4)]),
            Some(Card::Value(4))
        );
        assert_eq!(proposed(&[Card::Unsure]), Some(Card::Unsure));
        assert_eq!(proposed(&[Card::Unsure, Card::Unsure, Card::Value(0)]), Some(Card::Value(0)));
        assert_eq!(proposed(&[]), None);
    }

    #[test]
    fn the_spread() {
        let m = Card::Value(2);
        let l = Card::Value(3);
        assert_eq!(spread(&[m, l, m, Card::Break]), Some((m, l)));
        assert_eq!(spread(&[Card::Unsure, Card::Break]), None);
    }
}
