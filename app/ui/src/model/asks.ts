// The host's asks (`read-and-estimate`): a request to read something and a vote on a task's
// size. What the windows work out themselves: the time left, the cards and the spread of a vote,
// the card to propose keeping, and what the dialogs offer to read or estimate.

import type { Card, CardSet, ReadState, ReadTarget, TimelineEvent } from '../api/types';

/** Longest question text, in characters; `MAX_QUESTION_CHARS` in crates/protocol. */
export const MAX_QUESTION_CHARS = 280;

/** Longest task, in characters; `MAX_TASK_CHARS` in crates/protocol. */
export const MAX_TASK_CHARS = 300;

/** The time limits a read request may have, in seconds; 0 is none. */
export const READ_LIMITS: { seconds: number; label: string }[] = [
  { seconds: 0, label: 'No time limit' },
  { seconds: 120, label: '2 minutes' },
  { seconds: 300, label: '5 minutes' },
  { seconds: 600, label: '10 minutes' },
];

/** Each set's value cards, smallest first; `CardSet::cards` in crates/protocol. */
export const CARD_SETS: Record<CardSet, { name: string; inSentence: string; values: string[] }> = {
  'story-points': { name: 'Story points', inSentence: 'story points', values: ['1', '2', '3', '5', '8', '13'] },
  't-shirt': { name: 'T-shirt sizes', inSentence: 'T-shirt sizes', values: ['XS', 'S', 'M', 'L', 'XL'] },
};

/** Every card of a set, in the order shown: the values, then ? and ☕. */
export function cardsOf(set: CardSet): Card[] {
  return [...CARD_SETS[set].values.map((_, value) => ({ value })), 'unsure', 'break'];
}

export function cardLabel(card: Card, set: CardSet): string {
  if (card === 'unsure') return '?';
  if (card === 'break') return '☕';
  return CARD_SETS[set].values[card.value] ?? '?';
}

/** What a card means, for screen readers and titles. */
export function cardName(card: Card, set: CardSet): string {
  if (card === 'unsure') return 'No idea';
  if (card === 'break') return 'I need a break';
  return cardLabel(card, set);
}

export function sameCard(a: Card | null | undefined, b: Card | null | undefined): boolean {
  if (a === null || a === undefined || b === null || b === undefined) return a === b;
  if (typeof a === 'string' || typeof b === 'string') return a === b;
  return a.value === b.value;
}

function valueOf(card: Card): number | null {
  return typeof card === 'string' ? null : card.value;
}

/** The lowest and highest value card among `votes`; null when none is a value card. ? and ☕ don't count. */
export function spreadOf(votes: Card[]): { low: Card; high: Card } | null {
  const values = votes.map(valueOf).filter((v): v is number => v !== null);
  if (values.length === 0) return null;
  return { low: { value: Math.min(...values) }, high: { value: Math.max(...values) } };
}

/** The spread in words, read in the set's own order. */
export function spreadText(low: Card | null, high: Card | null, set: CardSet): string {
  if (!low || !high) return 'No one picked a size';
  if (sameCard(low, high)) return `Everyone agrees: ${cardLabel(low, set)}`;
  return `From ${cardLabel(low, set)} to ${cardLabel(high, set)}`;
}

/**
 * The card to propose keeping: the most common value card, the larger on a tie; ? or ☕ only
 * when no vote is a value card, ? before ☕. Null without votes. As `proposed` in crates/protocol.
 */
export function proposedCard(votes: Card[]): Card | null {
  const values = votes.map(valueOf).filter((v): v is number => v !== null);
  if (values.length > 0) {
    const counts = new Map<number, number>();
    for (const v of values) counts.set(v, (counts.get(v) ?? 0) + 1);
    let best = values[0]!;
    for (const [v, n] of counts) {
      const bestN = counts.get(best)!;
      if (n > bestN || (n === bestN && v > best)) best = v;
    }
    return { value: best };
  }
  if (votes.includes('unsure')) return 'unsure';
  return votes.includes('break') ? 'break' : null;
}

/** Milliseconds left of a read request: `leftMs` when it arrived, counted down since. Never below 0. */
export function timeLeft(leftMs: number, receivedAt: number, now: number): number {
  return Math.max(0, leftMs - (now - receivedAt));
}

/** Time left as m:ss, rounded up so 0:00 means the time is up. */
export function formatLeft(ms: number): string {
  const seconds = Math.ceil(ms / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}

/** How a participant's state reads in the host's list. */
export const READ_STATE_TEXT: Record<ReadState, string> = {
  reading: 'Reading',
  ready: 'Ready',
  question: 'Has a question',
  'older-app': 'On an older app',
};

/** The open tasks of a `tasks.md` text: each `- [ ]` line without its box, cut to the longest task. */
export function openTasks(text: string): string[] {
  return text
    .split('\n')
    .map((line) => /^\s*[-*] \[ \] (.+)$/.exec(line)?.[1]?.trim())
    .filter((task): task is string => !!task)
    .map((task) => [...task].slice(0, MAX_TASK_CHARS).join(''));
}

/** The text of the host's last prompt, cut to the longest task; null before any. */
export function lastPrompt(timeline: TimelineEvent[]): string | null {
  for (let i = timeline.length - 1; i >= 0; i--) {
    const event = timeline[i]!;
    if (event.kind === 'prompt') return [...event.text.trim()].slice(0, MAX_TASK_CHARS).join('') || null;
  }
  return null;
}

/**
 * The changes in the last prompt, as a read target: the timeline from the last prompt to the
 * newest event now. Null when the agent changed no file since that prompt. For a participant of
 * this release the timeline holds every event, so an event's place is its sequence number.
 */
export function lastPromptChanges(timeline: TimelineEvent[]): Extract<ReadTarget, { changes: unknown }> | null {
  let prompt = -1;
  for (let i = timeline.length - 1; i >= 0; i--) {
    if (timeline[i]!.kind === 'prompt') {
      prompt = i;
      break;
    }
  }
  if (prompt < 0) return null;
  const changed = timeline.slice(prompt + 1).some((event) => event.kind === 'file-change');
  return changed ? { changes: { fromSeq: prompt, toSeq: timeline.length - 1 } } : null;
}

/** The file changes in a `changes` range, in the order they were made. */
export function changesIn(timeline: TimelineEvent[], fromSeq: number, toSeq: number) {
  return timeline
    .slice(fromSeq + 1, toSeq + 1)
    .filter((event): event is Extract<TimelineEvent, { kind: 'file-change' }> => event.kind === 'file-change');
}

/** The id of a changes tab for a range. */
export function changesTabPath(fromSeq: number, toSeq: number): string {
  return `${fromSeq}-${toSeq}`;
}

/** What a read target is called, for the kind of thing it is. */
export function targetKind(target: ReadTarget): string {
  if ('document' in target) return 'document';
  if ('code' in target) return 'code file';
  if ('shared' in target) return 'shared file';
  return 'changes';
}

/** The card set the host used last, kept in this browser; story points the first time. */
const LAST_SET_KEY = 'specdeck.lastCardSet';

export function lastCardSet(): CardSet {
  try {
    const stored = localStorage.getItem(LAST_SET_KEY);
    return stored === 't-shirt' ? 't-shirt' : 'story-points';
  } catch {
    return 'story-points';
  }
}

export function rememberCardSet(set: CardSet) {
  try {
    localStorage.setItem(LAST_SET_KEY, set);
  } catch {
    // Not kept: the dialog starts on story points next time.
  }
}
