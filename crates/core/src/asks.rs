//! The host's open read request and vote (`read-and-estimate` design decision 1), kept by the
//! sharing side next to the viewers' requests. Answers are kept by participant id, so a viewer
//! who leaves or reconnects finds them again; only participants present are shown and counted.
//! The host is participant 0: it votes like everyone, and is not asked to read.
//!
//! A vote's value leaves this module only in the participant's own view and, after the reveal,
//! in everyone's (design decision 4).

use std::collections::HashMap;
use std::time::Duration;

use specdeck_protocol::asks::{
    self, Card, CardSet, READ_LIMITS_S, ReadAnswer, ReadState, ReadTarget, ReadView, VoteView,
};
use specdeck_protocol::timeline::Entry;
use tokio::time::Instant;

use crate::diag;

/// The host's participant id.
pub const HOST: u32 = 0;

/// Why the host's ask was not taken, for the host's window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskRefusal {
    /// One of its kind is open already.
    AlreadyOpen,
    /// None of its kind is open.
    NotOpen,
    /// Not one of the time limits.
    Limit,
    /// Nothing that can be read: not in the timeline or the shared files.
    Target,
    /// The label is blank, too long or has control characters.
    Label,
    /// The task is blank, too long or has control characters.
    Task,
    /// Not a card of the vote's set.
    Card,
    /// Not before the reveal: nothing is revealed yet, or nobody has voted.
    NotRevealed,
    /// Not after the reveal.
    Revealed,
}

impl AskRefusal {
    /// What the host's window shows; it checks first, so the host rarely sees these.
    pub fn message(self) -> &'static str {
        match self {
            AskRefusal::AlreadyOpen => "One is open already. Close it first.",
            AskRefusal::NotOpen => "That has already closed.",
            AskRefusal::Limit => "The time limit is none, 2, 5 or 10 minutes.",
            AskRefusal::Target => "That can't be read in this session.",
            AskRefusal::Label => "That can't be named that way.",
            AskRefusal::Task => "A task is 1 to 300 characters.",
            AskRefusal::Card => "That card is not in this vote's set.",
            AskRefusal::NotRevealed => "Reveal the votes first.",
            AskRefusal::Revealed => "The votes are revealed already.",
        }
    }
}

/// An answer that names an ask that is not open, an old round, a vote after the reveal, or a
/// card of another set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gone;

/// Who is in the session now: each viewer present, and whether their app can answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Present {
    viewers: Vec<(u32, bool)>,
}

impl Present {
    /// `viewers` pairs each viewer id with whether it speaks the asks' protocol version.
    pub fn new(mut viewers: Vec<(u32, bool)>) -> Self {
        viewers.sort_by_key(|(id, _)| *id);
        viewers.dedup_by_key(|(id, _)| *id);
        Self { viewers }
    }

    fn answering(&self) -> impl Iterator<Item = u32> + '_ {
        self.viewers.iter().filter(|(_, can)| *can).map(|(id, _)| *id)
    }

    fn older(&self) -> impl Iterator<Item = u32> + '_ {
        self.viewers.iter().filter(|(_, can)| !*can).map(|(id, _)| *id)
    }

    /// Everyone who can vote: the host, then each viewer that can answer.
    fn voters(&self) -> impl Iterator<Item = u32> + '_ {
        std::iter::once(HOST).chain(self.answering())
    }
}

struct OpenRead {
    id: u32,
    target: ReadTarget,
    label: String,
    limit_s: u16,
    deadline: Option<Instant>,
    answers: HashMap<u32, ReadAnswer>,
}

struct OpenVote {
    id: u32,
    task: String,
    cards: CardSet,
    round: u16,
    revealed: bool,
    votes: HashMap<u32, Card>,
}

#[derive(Default)]
pub struct Asks {
    read: Option<OpenRead>,
    vote: Option<OpenVote>,
    /// The last id given; ids count from 1 across both kinds, so a stale answer can always be
    /// told apart.
    last_id: u32,
}

impl Asks {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&mut self) -> u32 {
        self.last_id += 1;
        self.last_id
    }

    pub fn read_open(&self) -> bool {
        self.read.is_some()
    }

    pub fn vote_open(&self) -> bool {
        self.vote.is_some()
    }

    /// When the open read request's time is up.
    pub fn deadline(&self) -> Option<Instant> {
        self.read.as_ref().and_then(|read| read.deadline)
    }

    /// Opens a read request; the caller has checked that the target exists.
    pub fn open_read(
        &mut self,
        target: ReadTarget,
        label: String,
        limit_s: u16,
        now: Instant,
    ) -> Result<u32, AskRefusal> {
        if self.read.is_some() {
            return Err(AskRefusal::AlreadyOpen);
        }
        if !READ_LIMITS_S.contains(&limit_s) {
            return Err(AskRefusal::Limit);
        }
        if !asks::valid_read_label(&label) {
            return Err(AskRefusal::Label);
        }
        let id = self.next_id();
        diag::ask_read_opened(id, target.kind(), limit_s);
        let deadline = (limit_s > 0).then(|| now + Duration::from_secs(limit_s.into()));
        self.read =
            Some(OpenRead { id, target, label, limit_s, deadline, answers: HashMap::new() });
        Ok(id)
    }

    /// A viewer's answer to the read request `ask`. A question's blank text counts as none.
    pub fn answer_read(&mut self, who: u32, ask: u32, answer: ReadAnswer) -> Result<(), Gone> {
        let read = self.read.as_mut().filter(|read| read.id == ask && who != HOST).ok_or(Gone)?;
        let answer = match answer {
            ReadAnswer::Question { text } => ReadAnswer::Question {
                text: text.filter(|t| !t.trim().is_empty() && asks::valid_question(t)),
            },
            answer => answer,
        };
        read.answers.insert(who, answer);
        Ok(())
    }

    /// Closes the read request and returns its timeline entry, counting the viewers present
    /// who could answer.
    pub fn close_read(&mut self, present: &Present, time_up: bool) -> Result<Entry, AskRefusal> {
        let read = self.read.take().ok_or(AskRefusal::NotOpen)?;
        let (mut ready, mut questions, mut of) = (0u16, 0u16, 0u16);
        for id in present.answering() {
            of += 1;
            match read.answers.get(&id) {
                Some(ReadAnswer::Ready) => ready += 1,
                Some(ReadAnswer::Question { .. }) => questions += 1,
                _ => {}
            }
        }
        diag::ask_read_closed(read.id, ready, questions, of, time_up);
        Ok(Entry::ReadDone {
            id: read.id,
            target: read.target,
            label: read.label,
            ready,
            questions,
            of,
            time_up,
        })
    }

    pub fn start_vote(&mut self, task: String, cards: CardSet) -> Result<u32, AskRefusal> {
        if self.vote.is_some() {
            return Err(AskRefusal::AlreadyOpen);
        }
        let task = task.trim().to_owned();
        if !asks::valid_task(&task) {
            return Err(AskRefusal::Task);
        }
        let id = self.next_id();
        diag::ask_vote_opened(id, cards.as_str());
        self.vote =
            Some(OpenVote { id, task, cards, round: 1, revealed: false, votes: HashMap::new() });
        Ok(id)
    }

    /// Someone's vote in round `round` of vote `ask`; `None` takes it back. Only before the
    /// reveal, and only a card of the vote's set.
    pub fn vote(&mut self, who: u32, ask: u32, round: u16, card: Option<Card>) -> Result<(), Gone> {
        let vote = self
            .vote
            .as_mut()
            .filter(|vote| vote.id == ask && vote.round == round && !vote.revealed)
            .ok_or(Gone)?;
        match card {
            Some(card) if !vote.cards.accepts(card) => return Err(Gone),
            Some(card) => vote.votes.insert(who, card),
            None => vote.votes.remove(&who),
        };
        Ok(())
    }

    /// The host's own vote in the round open now.
    pub fn host_vote(&mut self, card: Option<Card>) -> Result<(), AskRefusal> {
        let vote = self.vote.as_ref().ok_or(AskRefusal::NotOpen)?;
        if vote.revealed {
            return Err(AskRefusal::Revealed);
        }
        if card.is_some_and(|card| !vote.cards.accepts(card)) {
            return Err(AskRefusal::Card);
        }
        let (id, round) = (vote.id, vote.round);
        self.vote(HOST, id, round, card).map_err(|Gone| AskRefusal::NotOpen)
    }

    /// Reveals the votes, once someone present has voted.
    pub fn reveal(&mut self, present: &Present) -> Result<(), AskRefusal> {
        let vote = self.vote.as_mut().ok_or(AskRefusal::NotOpen)?;
        if vote.revealed {
            return Err(AskRefusal::Revealed);
        }
        if !present.voters().any(|id| vote.votes.contains_key(&id)) {
            return Err(AskRefusal::NotRevealed);
        }
        vote.revealed = true;
        diag::ask_vote_round(vote.id, vote.round, true);
        Ok(())
    }

    /// After the reveal: a new round with the same task and cards, and no votes.
    pub fn vote_again(&mut self) -> Result<(), AskRefusal> {
        let vote = self.vote.as_mut().ok_or(AskRefusal::NotOpen)?;
        if !vote.revealed {
            return Err(AskRefusal::NotRevealed);
        }
        vote.round += 1;
        vote.revealed = false;
        vote.votes.clear();
        diag::ask_vote_round(vote.id, vote.round, false);
        Ok(())
    }

    /// Closes the vote and keeps nothing.
    pub fn stop_vote(&mut self, present: &Present) -> Result<(), AskRefusal> {
        let vote = self.vote.take().ok_or(AskRefusal::NotOpen)?;
        let votes = present.voters().filter(|id| vote.votes.contains_key(id)).count();
        diag::ask_vote_closed(vote.id, false, votes as u16);
        Ok(())
    }

    /// After the reveal: closes the vote keeping `card`, and returns its timeline entry with the
    /// spread of the votes of those present.
    pub fn keep(&mut self, card: Card, present: &Present) -> Result<Entry, AskRefusal> {
        let vote = self.vote.as_ref().ok_or(AskRefusal::NotOpen)?;
        if !vote.revealed {
            return Err(AskRefusal::NotRevealed);
        }
        if !vote.cards.accepts(card) {
            return Err(AskRefusal::Card);
        }
        let vote = self.vote.take().expect("checked above");
        let cast: Vec<Card> =
            present.voters().filter_map(|id| vote.votes.get(&id).copied()).collect();
        let range = asks::spread(&cast);
        diag::ask_vote_closed(vote.id, true, cast.len() as u16);
        Ok(Entry::Estimated {
            id: vote.id,
            task: vote.task,
            cards: vote.cards,
            kept: card,
            low: range.map(|(low, _)| low),
            high: range.map(|(_, high)| high),
            votes: cast.len() as u16,
        })
    }

    /// The read request as `who` sees it: the host gets every question's text, a viewer only
    /// their own.
    pub fn read_view(&self, who: u32, present: &Present, now: Instant) -> Option<ReadView> {
        let read = self.read.as_ref()?;
        let mut people: Vec<(u32, ReadState)> = present
            .answering()
            .map(|id| (id, read.answers.get(&id).map_or(ReadState::Reading, ReadState::from)))
            .chain(present.older().map(|id| (id, ReadState::OlderApp)))
            .collect();
        people.sort_by_key(|(id, _)| *id);
        let questions = if who == HOST {
            present
                .answering()
                .filter_map(|id| match read.answers.get(&id) {
                    Some(ReadAnswer::Question { text: Some(text) }) => Some((id, text.clone())),
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };
        Some(ReadView {
            id: read.id,
            target: read.target.clone(),
            label: read.label.clone(),
            limit_s: read.limit_s,
            left_ms: read
                .deadline
                .map(|deadline| deadline.saturating_duration_since(now).as_millis() as u64),
            people,
            mine: read.answers.get(&who).cloned().unwrap_or(ReadAnswer::Reading),
            questions,
        })
    }

    /// The vote as `who` sees it: before the reveal, only their own card.
    pub fn vote_view(&self, who: u32, present: &Present) -> Option<VoteView> {
        let vote = self.vote.as_ref()?;
        let cast: Vec<(u32, Card)> =
            present.voters().filter_map(|id| vote.votes.get(&id).map(|card| (id, *card))).collect();
        Some(VoteView {
            id: vote.id,
            round: vote.round,
            task: vote.task.clone(),
            cards: vote.cards,
            voted: cast.iter().map(|(id, _)| *id).collect(),
            mine: vote.votes.get(&who).copied(),
            revealed: vote.revealed.then_some(cast),
            older: present.older().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn present() -> Present {
        Present::new(vec![(2, true), (1, true), (3, false)])
    }

    fn design() -> ReadTarget {
        ReadTarget::Document { path: "openspec/changes/x/design.md".into() }
    }

    fn open_read(asks: &mut Asks, limit_s: u16) -> u32 {
        asks.open_read(design(), "design.md".into(), limit_s, Instant::now()).unwrap()
    }

    #[test]
    fn one_read_request_at_a_time_and_one_vote_beside_it() {
        let mut asks = Asks::new();
        let read = open_read(&mut asks, 0);
        assert_eq!(
            asks.open_read(design(), "design.md".into(), 0, Instant::now()),
            Err(AskRefusal::AlreadyOpen)
        );
        let vote = asks.start_vote("Estimate me".into(), CardSet::StoryPoints).unwrap();
        assert_ne!(read, vote, "ids count across both kinds");
        assert_eq!(asks.start_vote("Again".into(), CardSet::TShirt), Err(AskRefusal::AlreadyOpen));
        assert!(asks.read_open() && asks.vote_open());
        asks.close_read(&present(), false).unwrap();
        assert!(!asks.read_open() && asks.vote_open(), "each closes on its own");
        assert_eq!(open_read(&mut asks, 0), 3);
    }

    #[test]
    fn what_the_host_may_ask() {
        let mut asks = Asks::new();
        let now = Instant::now();
        assert_eq!(asks.open_read(design(), "d".into(), 60, now), Err(AskRefusal::Limit));
        assert_eq!(asks.open_read(design(), " ".into(), 0, now), Err(AskRefusal::Label));
        assert_eq!(asks.start_vote("x".repeat(301), CardSet::StoryPoints), Err(AskRefusal::Task));
        assert_eq!(asks.start_vote("  ".into(), CardSet::StoryPoints), Err(AskRefusal::Task));
        assert_eq!(asks.reveal(&present()), Err(AskRefusal::NotOpen));
        assert_eq!(asks.close_read(&present(), false), Err(AskRefusal::NotOpen));
    }

    #[test]
    fn read_answers_counts_and_question_texts() {
        let mut asks = Asks::new();
        let id = open_read(&mut asks, 300);
        let question = |text: &str| ReadAnswer::Question { text: Some(text.into()) };
        asks.answer_read(1, id, ReadAnswer::Ready).unwrap();
        asks.answer_read(2, id, question("Why not reuse the bucket?")).unwrap();
        // Someone who left keeps their answer but is not counted.
        asks.answer_read(9, id, ReadAnswer::Ready).unwrap();
        assert_eq!(asks.answer_read(1, id + 1, ReadAnswer::Ready), Err(Gone));
        assert_eq!(asks.answer_read(HOST, id, ReadAnswer::Ready), Err(Gone));

        let now = Instant::now();
        let host = asks.read_view(HOST, &present(), now).unwrap();
        assert_eq!(
            host.people,
            [(1, ReadState::Ready), (2, ReadState::Question), (3, ReadState::OlderApp)]
        );
        assert_eq!(host.questions, [(2, "Why not reuse the bucket?".to_owned())]);
        assert!(host.left_ms.is_some_and(|left| left <= 300_000 && left > 299_000));
        let other = asks.read_view(1, &present(), now).unwrap();
        assert!(other.questions.is_empty(), "no other viewer's question text");
        assert_eq!(other.mine, ReadAnswer::Ready);
        let asker = asks.read_view(2, &present(), now).unwrap();
        assert_eq!(asker.mine, question("Why not reuse the bucket?"));

        // Answers can change while open.
        asks.answer_read(1, id, ReadAnswer::Reading).unwrap();
        let entry = asks.close_read(&present(), true).unwrap();
        assert_eq!(
            entry,
            Entry::ReadDone {
                id,
                target: design(),
                label: "design.md".into(),
                ready: 0,
                questions: 1,
                of: 2,
                time_up: true,
            }
        );
        assert_eq!(asks.answer_read(2, id, ReadAnswer::Ready), Err(Gone), "closed");
    }

    #[test]
    fn a_blank_question_text_counts_as_none() {
        let mut asks = Asks::new();
        let id = open_read(&mut asks, 0);
        let blank = ReadAnswer::Question { text: Some("  ".into()) };
        asks.answer_read(1, id, blank).unwrap();
        let view = asks.read_view(1, &present(), Instant::now()).unwrap();
        assert_eq!(view.mine, ReadAnswer::Question { text: None });
        assert_eq!(view.left_ms, None, "no time limit");
    }

    #[test]
    fn votes_stay_hidden_until_the_reveal() {
        let mut asks = Asks::new();
        let id = asks.start_vote("Show unreachable".into(), CardSet::StoryPoints).unwrap();
        assert_eq!(asks.reveal(&present()), Err(AskRefusal::NotRevealed), "nobody voted");
        asks.vote(1, id, 1, Some(Card::Value(4))).unwrap();
        asks.host_vote(Some(Card::Value(3))).unwrap();
        for who in [HOST, 1, 2] {
            let view = asks.vote_view(who, &present()).unwrap();
            assert_eq!(view.voted, [HOST, 1]);
            assert_eq!(view.revealed, None);
            assert_eq!(view.older, [3]);
        }
        assert_eq!(asks.vote_view(1, &present()).unwrap().mine, Some(Card::Value(4)));
        assert_eq!(asks.vote_view(2, &present()).unwrap().mine, None);
        assert_eq!(asks.vote_view(HOST, &present()).unwrap().mine, Some(Card::Value(3)));

        // A vote can be changed and taken back until the reveal.
        asks.vote(2, id, 1, Some(Card::Value(0))).unwrap();
        asks.vote(2, id, 1, None).unwrap();
        asks.vote(1, id, 1, Some(Card::Value(5))).unwrap();
        asks.reveal(&present()).unwrap();
        let view = asks.vote_view(2, &present()).unwrap();
        assert_eq!(view.revealed, Some(vec![(HOST, Card::Value(3)), (1, Card::Value(5))]));
        assert_eq!(asks.vote(1, id, 1, Some(Card::Value(0))), Err(Gone), "not after the reveal");
        assert_eq!(asks.host_vote(None), Err(AskRefusal::Revealed));
        assert_eq!(asks.reveal(&present()), Err(AskRefusal::Revealed));
    }

    #[test]
    fn cards_of_another_set_and_old_rounds_are_gone() {
        let mut asks = Asks::new();
        let id = asks.start_vote("t".into(), CardSet::TShirt).unwrap();
        assert_eq!(asks.vote(1, id, 1, Some(Card::Value(5))), Err(Gone), "no 13 in t-shirts");
        assert_eq!(asks.host_vote(Some(Card::Value(5))), Err(AskRefusal::Card));
        asks.vote(1, id, 1, Some(Card::Break)).unwrap();
        assert_eq!(asks.vote(1, id + 1, 1, Some(Card::Value(0))), Err(Gone), "another ask");
        asks.reveal(&present()).unwrap();
        asks.vote_again().unwrap();
        assert_eq!(asks.vote(1, id, 1, Some(Card::Value(0))), Err(Gone), "an old round");
        asks.vote(1, id, 2, Some(Card::Value(0))).unwrap();
    }

    #[test]
    fn vote_again_keeps_the_task_and_cards_and_clears_the_votes() {
        let mut asks = Asks::new();
        let id = asks.start_vote("Show unreachable".into(), CardSet::StoryPoints).unwrap();
        assert_eq!(asks.vote_again(), Err(AskRefusal::NotRevealed));
        asks.vote(1, id, 1, Some(Card::Value(2))).unwrap();
        asks.reveal(&present()).unwrap();
        asks.vote_again().unwrap();
        let view = asks.vote_view(1, &present()).unwrap();
        assert_eq!(
            (view.round, view.task.as_str(), view.cards),
            (2, "Show unreachable", CardSet::StoryPoints)
        );
        assert!(view.voted.is_empty() && view.mine.is_none() && view.revealed.is_none());
    }

    #[test]
    fn keep_needs_the_reveal_and_a_card_of_the_set() {
        let mut asks = Asks::new();
        let id = asks.start_vote("Show unreachable".into(), CardSet::StoryPoints).unwrap();
        asks.vote(1, id, 1, Some(Card::Value(2))).unwrap();
        asks.vote(2, id, 1, Some(Card::Value(4))).unwrap();
        asks.host_vote(Some(Card::Unsure)).unwrap();
        // Someone who left is not counted.
        asks.vote(9, id, 1, Some(Card::Value(5))).unwrap();
        assert_eq!(asks.keep(Card::Value(3), &present()), Err(AskRefusal::NotRevealed));
        asks.reveal(&present()).unwrap();
        let tshirt_only = CardSet::TShirt.cards().len() as u8;
        assert_eq!(asks.keep(Card::Value(tshirt_only + 1), &present()), Err(AskRefusal::Card));
        let entry = asks.keep(Card::Value(3), &present()).unwrap();
        assert_eq!(
            entry,
            Entry::Estimated {
                id,
                task: "Show unreachable".into(),
                cards: CardSet::StoryPoints,
                kept: Card::Value(3),
                low: Some(Card::Value(2)),
                high: Some(Card::Value(4)),
                votes: 3,
            }
        );
        assert!(!asks.vote_open());
    }

    #[test]
    fn a_stopped_vote_records_nothing() {
        let mut asks = Asks::new();
        asks.start_vote("t".into(), CardSet::TShirt).unwrap();
        asks.stop_vote(&present()).unwrap();
        assert!(asks.vote_view(1, &present()).is_none());
        assert_eq!(asks.stop_vote(&present()), Err(AskRefusal::NotOpen));
    }

    #[test]
    fn leaving_and_coming_back_keeps_a_vote() {
        let mut asks = Asks::new();
        let id = asks.start_vote("t".into(), CardSet::StoryPoints).unwrap();
        asks.vote(2, id, 1, Some(Card::Value(4))).unwrap();
        let without = Present::new(vec![(1, true)]);
        let view = asks.vote_view(HOST, &without).unwrap();
        assert!(view.voted.is_empty(), "not shown while away");
        let back = asks.vote_view(2, &present()).unwrap();
        assert_eq!((back.voted, back.mine), (vec![2], Some(Card::Value(4))));
    }

    #[test]
    fn time_left_counts_down() {
        let mut asks = Asks::new();
        let start = Instant::now();
        asks.open_read(design(), "design.md".into(), 120, start).unwrap();
        assert_eq!(asks.deadline(), Some(start + Duration::from_secs(120)));
        let later = start + Duration::from_secs(100);
        assert_eq!(asks.read_view(1, &present(), later).unwrap().left_ms, Some(20_000));
        let past = start + Duration::from_secs(200);
        assert_eq!(asks.read_view(1, &present(), past).unwrap().left_ms, Some(0));
    }
}
