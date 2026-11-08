import { describe, expect, it } from 'vitest';
import type { Card, TimelineEvent } from '../api/types';
import {
  cardLabel,
  cardsOf,
  changesIn,
  formatLeft,
  lastPrompt,
  lastPromptChanges,
  openTasks,
  proposedCard,
  spreadOf,
  spreadText,
  timeLeft,
} from './asks';

const M: Card = { value: 2 };
const L: Card = { value: 3 };

describe('the spread', () => {
  it('runs in the set’s order, without ? and ☕', () => {
    const votes: Card[] = [M, L, M, 'break'];
    const spread = spreadOf(votes)!;
    expect(spreadText(spread.low, spread.high, 't-shirt')).toBe('From M to L');
    expect(proposedCard(votes)).toEqual(M);
  });

  it('says when everyone agrees', () => {
    const spread = spreadOf([{ value: 3 }, { value: 3 }])!;
    expect(spreadText(spread.low, spread.high, 'story-points')).toBe('Everyone agrees: 5');
  });

  it('is empty when nobody picked a size', () => {
    expect(spreadOf(['unsure', 'break'])).toBeNull();
    expect(spreadText(null, null, 't-shirt')).toBe('No one picked a size');
  });
});

describe('the proposed card', () => {
  it('is the larger on a tie', () => {
    expect(proposedCard([{ value: 3 }, { value: 3 }, { value: 4 }, { value: 4 }])).toEqual({ value: 4 });
    expect(cardLabel({ value: 4 }, 'story-points')).toBe('8');
  });

  it('is ? only when every vote is ? or ☕', () => {
    expect(proposedCard(['unsure'])).toBe('unsure');
    expect(proposedCard(['unsure', 'unsure', { value: 0 }])).toEqual({ value: 0 });
    expect(proposedCard(['break'])).toBe('break');
    expect(proposedCard([])).toBeNull();
  });
});

describe('cards', () => {
  it('are the values, then ? and ☕', () => {
    expect(cardsOf('t-shirt').map((card) => cardLabel(card, 't-shirt'))).toEqual(['XS', 'S', 'M', 'L', 'XL', '?', '☕']);
    expect(cardsOf('story-points').map((card) => cardLabel(card, 'story-points'))).toEqual(['1', '2', '3', '5', '8', '13', '?', '☕']);
  });
});

describe('the countdown', () => {
  it('counts down from what the host sent', () => {
    expect(timeLeft(120_000, 1_000, 31_000)).toBe(90_000);
    expect(formatLeft(90_000)).toBe('1:30');
    expect(formatLeft(59_001)).toBe('1:00');
  });

  it('stops at 0:00 until the host closes it', () => {
    expect(timeLeft(5_000, 0, 60_000)).toBe(0);
    expect(formatLeft(0)).toBe('0:00');
  });
});

describe('open tasks', () => {
  it('are the unchecked lines of tasks.md', () => {
    const text = [
      '## 3. Viewers',
      '- [x] 3.0 Done already',
      '- [ ] 3.1 Show unreachable after three missed heartbeats',
      '  - [ ] 3.2 Nested, still open',
      'Not a task',
    ].join('\n');
    expect(openTasks(text)).toEqual(['3.1 Show unreachable after three missed heartbeats', '3.2 Nested, still open']);
  });

  it('are cut to the longest task', () => {
    expect(openTasks(`- [ ] ${'é'.repeat(400)}`)[0]).toHaveLength(300);
  });
});

const stamp = (outputSeq: number) => ({ outputSeq, at: 0 });
const change = (path: string): TimelineEvent => ({ ...stamp(0), kind: 'file-change', promptId: 'p2', toolUseId: path, path, change: 'edited', hunks: [] });

describe('the last prompt', () => {
  const timeline: TimelineEvent[] = [
    { ...stamp(0), kind: 'prompt', promptId: 'p1', text: 'First' },
    change('old.rs'),
    { ...stamp(0), kind: 'prompt', promptId: 'p2', text: '  Show unreachable  ' },
    change('a.rs'),
    { ...stamp(0), kind: 'response', promptId: 'p2', text: 'Done' },
    change('README.md'),
  ];

  it('gives the task its text', () => {
    expect(lastPrompt(timeline)).toBe('Show unreachable');
    expect(lastPrompt([])).toBeNull();
  });

  it('gives the changes since it, up to now', () => {
    const target = lastPromptChanges(timeline);
    expect(target).toEqual({ changes: { fromSeq: 2, toSeq: 5 } });
    expect(changesIn(timeline, 2, 5).map((c) => c.path)).toEqual(['a.rs', 'README.md']);
    // A change after the range is not in it.
    expect(changesIn([...timeline, change('later.rs')], 2, 5)).toHaveLength(2);
  });

  it('gives nothing when no file changed since', () => {
    expect(lastPromptChanges(timeline.slice(0, 3))).toBeNull();
  });
});
