//! When the host hears about the answers to their read requests and votes
//! (`host-ask-notifications` design decision 1): a viewer's question, everyone having read, the
//! time running out while someone still reads, and everyone having voted. It follows the host's
//! own views of the asks, so it needs nothing the host doesn't already get; the app decides
//! whether each kind is shown.

use std::collections::HashSet;

use specdeck_protocol::asks::{ReadState, ReadView, VoteView};

/// The host's own participant id, as in a vote's `voted`.
const HOST: u32 = 0;

/// Something about the host's asks worth a notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AskNotice {
    /// `viewer` has a question about the read request `ask`, which asks to read `label`.
    Question { ask: u32, viewer: u32, text: Option<String>, label: String },
    /// Nobody who can answer is still reading.
    EveryoneRead { label: String, ready: usize, questions: usize },
    /// The time ran out while someone was still reading.
    TimeUp { label: String, ready: usize, questions: usize, reading: usize },
    /// Every viewer who can vote has voted this round; `votes` counts the host's too.
    EveryoneVoted { task: String, votes: usize, host_missing: bool },
}

/// What the host was last told about the open read request and vote.
#[derive(Default)]
pub struct AskNotices {
    read: Option<OpenRead>,
    /// The read request that closed last, whose view may still be heard once more.
    closed: Option<u32>,
    vote: Option<OpenVote>,
}

/// The open read request as last seen.
struct OpenRead {
    view: ReadView,
    /// Viewers whose question was notified; once each per request.
    asked: HashSet<u32>,
    everyone: bool,
}

/// The open vote's round as last seen.
struct OpenVote {
    id: u32,
    round: u16,
    everyone: bool,
}

/// How many who can answer are ready, have a question, and still read.
fn counts(view: &ReadView) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for (_, state) in &view.people {
        match state {
            ReadState::Ready => counts.0 += 1,
            ReadState::Question => counts.1 += 1,
            ReadState::Reading => counts.2 += 1,
            ReadState::OlderApp => {}
        }
    }
    counts
}

impl AskNotices {
    pub fn new() -> Self {
        Self::default()
    }

    /// The host's new view of the read request; what it brings to notify.
    pub fn read(&mut self, view: Option<&ReadView>) -> Vec<AskNotice> {
        let Some(view) = view.filter(|view| Some(view.id) != self.closed) else {
            self.read = None;
            return Vec::new();
        };
        let state = match &mut self.read {
            Some(state) if state.view.id == view.id => state,
            state => state.insert(OpenRead {
                view: view.clone(),
                asked: HashSet::new(),
                everyone: false,
            }),
        };
        state.view = view.clone();
        let mut notices = Vec::new();
        for (viewer, read) in &view.people {
            if *read == ReadState::Question && state.asked.insert(*viewer) {
                let text = view.questions.iter().find(|(id, _)| id == viewer);
                notices.push(AskNotice::Question {
                    ask: view.id,
                    viewer: *viewer,
                    text: text.map(|(_, text)| text.clone()),
                    label: view.label.clone(),
                });
            }
        }
        let (ready, questions, reading) = counts(view);
        let everyone = ready + questions > 0 && reading == 0;
        if everyone && !state.everyone {
            notices.push(AskNotice::EveryoneRead { label: view.label.clone(), ready, questions });
        }
        state.everyone = everyone;
        notices
    }

    /// The read request closed; when its time was up while someone still read, the outcome as
    /// last seen.
    pub fn read_closed(&mut self, time_up: bool) -> Option<AskNotice> {
        let state = self.read.take()?;
        self.closed = Some(state.view.id);
        let (ready, questions, reading) = counts(&state.view);
        (time_up && reading > 0).then_some(AskNotice::TimeUp {
            label: state.view.label,
            ready,
            questions,
            reading,
        })
    }

    /// The host's new view of the vote, with the viewers present (participant ids); what it
    /// brings to notify. Call it again when only the viewers changed.
    pub fn vote(&mut self, view: Option<&VoteView>, viewers: &[u32]) -> Vec<AskNotice> {
        let Some(view) = view else {
            self.vote = None;
            return Vec::new();
        };
        let state = match &mut self.vote {
            Some(state) if state.id == view.id && state.round == view.round => state,
            state => state.insert(OpenVote { id: view.id, round: view.round, everyone: false }),
        };
        if view.revealed.is_some() {
            return Vec::new();
        }
        let mut voters = viewers.iter().filter(|id| !view.older.contains(id)).peekable();
        let everyone = voters.peek().is_some() && voters.all(|id| view.voted.contains(id));
        let notify = everyone && !state.everyone;
        state.everyone = everyone;
        if !notify {
            return Vec::new();
        }
        vec![AskNotice::EveryoneVoted {
            task: view.task.clone(),
            votes: view.voted.len(),
            host_missing: !view.voted.contains(&HOST),
        }]
    }
}

#[cfg(test)]
mod tests {
    use ReadState::{OlderApp, Question, Reading, Ready};
    use specdeck_protocol::asks::{Card, CardSet, ReadAnswer, ReadTarget};

    use super::*;

    fn read(id: u32, people: &[(u32, ReadState)], questions: &[(u32, &str)]) -> ReadView {
        ReadView {
            id,
            target: ReadTarget::Document { path: "design.md".into() },
            label: "design.md".into(),
            limit_s: 300,
            left_ms: Some(1000),
            people: people.to_vec(),
            mine: ReadAnswer::Reading,
            questions: questions.iter().map(|(id, text)| (*id, (*text).into())).collect(),
        }
    }

    fn vote(round: u16, voted: &[u32], older: &[u32]) -> VoteView {
        VoteView {
            id: 7,
            round,
            task: "Show unreachable".into(),
            cards: CardSet::StoryPoints,
            voted: voted.to_vec(),
            mine: None,
            revealed: None,
            older: older.to_vec(),
        }
    }

    fn question(viewer: u32, text: Option<&str>) -> AskNotice {
        question_in(1, viewer, text)
    }

    fn question_in(ask: u32, viewer: u32, text: Option<&str>) -> AskNotice {
        AskNotice::Question { ask, viewer, text: text.map(Into::into), label: "design.md".into() }
    }

    #[test]
    fn a_question_notifies_once_per_viewer_per_request() {
        let mut notices = AskNotices::new();
        assert!(notices.read(Some(&read(1, &[(1, Reading), (2, Reading)], &[]))).is_empty());
        assert_eq!(
            notices.read(Some(&read(1, &[(1, Question), (2, Reading)], &[(1, "Why?")]))),
            [question(1, Some("Why?"))]
        );
        // Edited, then taken back and asked again: nothing more about viewer 1.
        assert!(
            notices
                .read(Some(&read(1, &[(1, Question), (2, Reading)], &[(1, "Why not?")])))
                .is_empty()
        );
        assert!(notices.read(Some(&read(1, &[(1, Reading), (2, Reading)], &[]))).is_empty());
        assert!(notices.read(Some(&read(1, &[(1, Question), (2, Reading)], &[]))).is_empty());
        // Another viewer, without a text: a notification of its own.
        assert_eq!(
            notices.read(Some(&read(1, &[(1, Question), (2, Question)], &[]))),
            [
                question(2, None),
                AskNotice::EveryoneRead { label: "design.md".into(), ready: 0, questions: 2 }
            ]
        );
        // A new request starts over.
        assert_eq!(
            notices.read(Some(&read(2, &[(1, Question), (2, Reading)], &[]))),
            [question_in(2, 1, None)]
        );
    }

    #[test]
    fn two_questions_at_once_notify_each() {
        let mut notices = AskNotices::new();
        notices.read(Some(&read(1, &[(1, Reading), (2, Reading), (3, Reading)], &[])));
        let both = notices.read(Some(&read(1, &[(1, Question), (2, Question), (3, Reading)], &[])));
        assert_eq!(both, [question(1, None), question(2, None)]);
    }

    fn everyone(ready: usize, questions: usize) -> AskNotice {
        AskNotice::EveryoneRead { label: "design.md".into(), ready, questions }
    }

    #[test]
    fn everyone_has_read_when_nobody_still_reads() {
        let mut notices = AskNotices::new();
        notices.read(Some(&read(1, &[(1, Ready), (2, Question), (3, Reading)], &[])));
        assert_eq!(
            notices.read(Some(&read(1, &[(1, Ready), (2, Question), (3, Ready)], &[]))),
            [everyone(2, 1)]
        );
        // Still everyone: not again.
        assert!(
            notices.read(Some(&read(1, &[(1, Ready), (2, Ready), (3, Ready)], &[]))).is_empty()
        );
        // A viewer joins, so someone reads; once they are ready, everyone again.
        let joined = [(1, Ready), (2, Ready), (3, Ready), (4, Reading)];
        assert!(notices.read(Some(&read(1, &joined, &[]))).is_empty());
        let done = [(1, Ready), (2, Ready), (3, Ready), (4, Ready)];
        assert_eq!(notices.read(Some(&read(1, &done, &[]))), [everyone(4, 0)]);
    }

    #[test]
    fn the_last_reader_leaving_means_everyone_has_read() {
        let mut notices = AskNotices::new();
        notices.read(Some(&read(1, &[(1, Ready), (2, Reading)], &[])));
        assert_eq!(notices.read(Some(&read(1, &[(1, Ready)], &[]))), [everyone(1, 0)]);
    }

    #[test]
    fn older_apps_and_nobody_present_are_not_everyone() {
        let mut notices = AskNotices::new();
        assert!(notices.read(Some(&read(1, &[], &[]))).is_empty());
        assert!(notices.read(Some(&read(1, &[(1, OlderApp)], &[]))).is_empty());
        // An older app doesn't hold it up either.
        assert_eq!(
            notices.read(Some(&read(1, &[(1, OlderApp), (2, Ready)], &[]))),
            [everyone(1, 0)]
        );
    }

    #[test]
    fn time_up_only_while_someone_still_reads() {
        let mut notices = AskNotices::new();
        let people = [(1, Ready), (2, Ready), (3, Ready), (4, Question), (5, Reading)];
        notices.read(Some(&read(1, &people, &[])));
        assert_eq!(
            notices.read_closed(true),
            Some(AskNotice::TimeUp {
                label: "design.md".into(),
                ready: 3,
                questions: 1,
                reading: 1
            })
        );
        notices.read(None);

        // Everyone had read: the time running out adds nothing.
        notices.read(Some(&read(2, &[(1, Ready)], &[])));
        assert_eq!(notices.read_closed(true), None);

        // The host going on never notifies.
        notices.read(Some(&read(3, &[(1, Reading)], &[])));
        assert_eq!(notices.read_closed(false), None);
        assert_eq!(notices.read_closed(true), None, "nothing open any more");
        // The closed request's view, heard once more before the one without it, starts nothing.
        assert!(notices.read(Some(&read(3, &[(1, Question)], &[]))).is_empty());
        assert_eq!(notices.read_closed(true), None);
    }

    fn voted(votes: usize, host_missing: bool) -> AskNotice {
        AskNotice::EveryoneVoted { task: "Show unreachable".into(), votes, host_missing }
    }

    #[test]
    fn everyone_has_voted_without_the_host() {
        let mut notices = AskNotices::new();
        let viewers = [1, 2, 3];
        assert!(notices.vote(Some(&vote(1, &[1, 2], &[])), &viewers).is_empty());
        assert_eq!(notices.vote(Some(&vote(1, &[1, 2, 3], &[])), &viewers), [voted(3, true)]);
        // The host votes too: still everyone, not again.
        assert!(notices.vote(Some(&vote(1, &[0, 1, 2, 3], &[])), &viewers).is_empty());
        // A new round notifies again, now with the host's vote in.
        assert!(notices.vote(Some(&vote(2, &[], &[])), &viewers).is_empty());
        assert_eq!(notices.vote(Some(&vote(2, &[0, 1, 2, 3], &[])), &viewers), [voted(4, false)]);
    }

    #[test]
    fn the_viewers_changing_can_make_everyone() {
        let mut notices = AskNotices::new();
        notices.vote(Some(&vote(1, &[1], &[3])), &[1, 2, 3]);
        // Viewer 2 leaves before voting; viewer 3 is on an older app.
        assert_eq!(notices.vote(Some(&vote(1, &[1], &[3])), &[1, 3]), [voted(1, true)]);
        // Someone joins, and votes.
        assert!(notices.vote(Some(&vote(1, &[1], &[3])), &[1, 3, 4]).is_empty());
        assert_eq!(notices.vote(Some(&vote(1, &[1, 4], &[3])), &[1, 3, 4]), [voted(2, true)]);
    }

    #[test]
    fn nothing_after_the_reveal_or_with_only_the_host() {
        let mut notices = AskNotices::new();
        assert!(notices.vote(Some(&vote(1, &[0], &[])), &[]).is_empty(), "only the host");
        assert!(notices.vote(Some(&vote(1, &[0], &[2])), &[2]).is_empty(), "only an older app");
        let revealed = VoteView { revealed: Some(vec![(1, Card::Value(2))]), ..vote(1, &[1], &[]) };
        assert!(notices.vote(Some(&revealed), &[1]).is_empty());
        // A viewer joins after the reveal.
        assert!(notices.vote(Some(&revealed), &[1, 2]).is_empty());
    }
}
