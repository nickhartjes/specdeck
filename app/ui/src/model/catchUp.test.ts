import { describe, expect, it } from 'vitest';
import type { TimelineEvent } from '../api/types';
import { eventsBefore, firstSentence, missedTurns, outcomeOf, ranFor, SEEN_GRACE_MS, soFar, turnEnd, turnFacts } from './catchUp';
import { buildTimeline } from './timeline';

const MIN = 60_000;
const T0 = 1_700_000_000_000;
const at = (minutes: number, seq = 0) => ({ at: T0 + minutes * MIN, outputSeq: seq });

/** Prompt `n` at minute `start`, answered at minute `end` (or never). */
function prompt(n: number, start: number, end: number | null, extra: TimelineEvent[] = []): TimelineEvent[] {
  return [
    { ...at(start), kind: 'prompt', promptId: `p${n}`, text: `Prompt ${n}` },
    ...extra,
    ...(end === null ? [] : [{ ...at(end), kind: 'response', promptId: `p${n}`, text: `Done with ${n}.` } as TimelineEvent]),
  ];
}

function call(promptId: string, id: string, minute: number, toolName: string, input: Record<string, unknown>, status: 'succeeded' | 'failed' | 'not-run' = 'succeeded', permission?: 'allowed' | 'denied'): TimelineEvent[] {
  return [
    { ...at(minute), kind: 'tool-call', promptId, toolUseId: id, toolName, input },
    { ...at(minute), kind: 'tool-result', promptId, toolUseId: id, status, ...(permission && { permission }) },
  ];
}

function edit(promptId: string, id: string, minute: number, path: string): TimelineEvent[] {
  return [
    ...call(promptId, id, minute, 'Edit', { file_path: path }),
    { ...at(minute), kind: 'file-change', promptId, toolUseId: id, path, change: 'edited', hunks: [] },
  ];
}

function plan(minute: number, change: string, done: number, total: number): TimelineEvent {
  return { ...at(minute), kind: 'plan', change, documents: [], tasks: { done, total } };
}

describe('which prompts fold', () => {
  // Six finished prompts, each answered after a minute, and a seventh running.
  const events = [1, 2, 3, 4, 5, 6].flatMap((n) => prompt(n, n * 5, n * 5 + 1)).concat(prompt(7, 35, null));
  const { turns } = buildTimeline(events);

  it('folds nothing without a join time', () => {
    expect(missedTurns(turns, null).size).toBe(0);
  });

  it('folds the prompts that finished before joining, not the running one', () => {
    expect([...missedTurns(turns, T0 + 40 * MIN)]).toEqual(['p1', 'p2', 'p3', 'p4', 'p5', 'p6']);
  });

  it('counts a prompt that ended just before joining as seen', () => {
    // p6 ended at minute 31; joining 30 seconds later.
    const missed = missedTurns(turns, T0 + 31 * MIN + 30_000);
    expect(missed.has('p5')).toBe(true);
    expect(missed.has('p6')).toBe(false);
    expect(SEEN_GRACE_MS).toBe(MIN);
  });

  it('never folds the latest prompt when joining, also once a newer one starts', () => {
    const before = [1, 2].flatMap((n) => prompt(n, n * 5, n * 5 + 1));
    expect([...missedTurns(buildTimeline(before).turns, T0 + 100 * MIN)]).toEqual(['p1']);
    const after = buildTimeline([...before, ...prompt(3, 110, null)]).turns;
    expect([...missedTurns(after, T0 + 100 * MIN)]).toEqual(['p1']);
  });

  it('ends a prompt without a reply at the next prompt', () => {
    const noReply = buildTimeline([...prompt(1, 0, null), ...prompt(2, 10, 11), ...prompt(3, 20, null)]).turns;
    expect(turnEnd(noReply, 0)).toBe(T0 + 10 * MIN);
    expect(turnEnd(noReply, 2)).toBeNull();
    expect([...missedTurns(noReply, T0 + 30 * MIN)]).toEqual(['p1', 'p2']);
  });

  it('does not fold prompts that ended after joining', () => {
    expect(missedTurns(turns, T0 + 2 * MIN).size).toBe(0);
  });

  it('finds the events before joining', () => {
    expect(eventsBefore(events, T0 + 12 * MIN)).toBe(4);
    expect(eventsBefore(events, T0)).toBe(0);
    expect(eventsBefore(events, T0 + 99 * MIN)).toBe(events.length);
  });
});

describe('outcome of a folded prompt', () => {
  const outcome = (events: TimelineEvent[], promptId = 'p1') => {
    const turn = buildTimeline(events).turns.find((t) => t.promptId === promptId)!;
    return outcomeOf(turn, turnFacts(events).get(promptId));
  };
  const docs = ['proposal.md', 'design.md', 'specs/heartbeat/spec.md', 'tasks.md'].map((f) => `openspec/changes/viewer-heartbeat/${f}`);

  it('names the plan a prompt wrote', () => {
    const events = prompt(1, 0, 5, [...docs.flatMap((path, i) => edit('p1', `t${i}`, 1, path)), plan(2, 'viewer-heartbeat', 0, 6)]);
    expect(outcome(events)).toBe('Wrote the plan: viewer-heartbeat');
  });

  it('counts changed files and tasks ticked, not the plan files', () => {
    const events = [
      ...prompt(1, 0, 5, [...docs.flatMap((path, i) => edit('p1', `t${i}`, 1, path)), plan(2, 'viewer-heartbeat', 0, 6)]),
      ...prompt(2, 10, 20, [
        ...['a.ts', 'b.ts', 'c.ts', 'd.ts'].flatMap((path, i) => edit('p2', `u${i}`, 11, `src/${path}`)),
        ...edit('p2', 'u9', 12, 'src/a.ts'),
        plan(13, 'viewer-heartbeat', 3, 6),
      ]),
    ];
    // Ticking tasks in tasks.md changes a plan file: the plan was updated.
    expect(outcome(events, 'p2')).toBe('4 files changed · 3 of 6 tasks done');
    expect(outcome([...events, ...edit('p2', 'u10', 14, docs[3]!)], 'p2')).toBe('Updated the plan: viewer-heartbeat · 4 files changed · 3 of 6 tasks done');
  });

  it('leaves tasks out when the count did not change', () => {
    const events = [...prompt(1, 0, 1, [plan(0, 'x', 2, 6)]), ...prompt(2, 2, 3, [...edit('p2', 't', 2, 'a.ts'), plan(2, 'x', 2, 6)])];
    expect(outcome(events, 'p2')).toBe('1 file changed');
    expect(outcome(events, 'p1')).toBe('2 of 6 tasks done');
  });

  it("falls back to the reply's first sentence", () => {
    const events: TimelineEvent[] = [
      ...prompt(1, 0, null, call('p1', 't', 0, 'Read', { file_path: 'config.ts' })),
      { ...at(1), kind: 'response', promptId: 'p1', text: 'The heartbeat runs every **5 seconds**. It is set in `config.ts`.' },
    ];
    expect(outcome(events)).toBe('The heartbeat runs every 5 seconds.');
  });

  it('says the host denied a step', () => {
    const events = prompt(1, 0, 5, [...edit('p1', 't1', 1, 'a.ts'), ...call('p1', 't2', 2, 'Bash', { command: 'rm -rf build' }, 'not-run', 'denied')]);
    expect(outcome(events)).toBe('1 file changed · the host denied a step');
  });

  it('mentions a failed step only when the prompt ended on it', () => {
    const recovered = prompt(1, 0, 5, [...call('p1', 't1', 1, 'Bash', { command: 'npm test' }, 'failed'), ...edit('p1', 't2', 2, 'a.ts'), ...call('p1', 't3', 3, 'Bash', { command: 'npm test' })]);
    expect(outcome(recovered)).toBe('1 file changed');
    const failed = prompt(1, 0, 5, [...edit('p1', 't2', 2, 'a.ts'), ...call('p1', 't3', 3, 'Bash', { command: 'npm test' }, 'failed')]);
    expect(outcome(failed)).toBe('1 file changed · ended on a failed step');
    expect(outcome(prompt(1, 0, null, call('p1', 't', 1, 'Bash', { command: 'false' }, 'failed')))).toBe('Ended on a failed step');
  });

  it('has no outcome without facts or a reply', () => {
    expect(outcome(prompt(1, 0, null))).toBeNull();
  });

  it('cuts a long sentence and strips Markdown', () => {
    expect(firstSentence('## Done\n\n- I [fixed](https://x) it')).toBe('Done I fixed it');
    const long = firstSentence('word '.repeat(60))!;
    expect(long.length).toBe(160);
    expect(long.endsWith('…')).toBe(true);
    expect(firstSentence('```\ncode\n```')).toBeNull();
  });
});

describe('so far', () => {
  const events: TimelineEvent[] = [
    { ...at(0), kind: 'session-started' },
    ...prompt(1, 1, 5, [
      ...edit('p1', 't1', 2, 'openspec/changes/viewer-heartbeat/proposal.md'),
      { ...at(2), kind: 'document', promptId: 'p1', toolUseId: 't1', path: 'openspec/changes/viewer-heartbeat/proposal.md', text: '# Proposal' },
      ...edit('p1', 't2', 3, 'notes.md'),
      { ...at(3), kind: 'document', promptId: 'p1', toolUseId: 't2', path: 'notes.md', text: 'notes' },
      plan(3, 'viewer-heartbeat', 0, 6),
    ]),
    { ...at(6), kind: 'file-shared', file: 1, name: 'wireframe.png', fileKind: 'image', mediaType: 'image/png', size: 10, sha256: '', by: 2, byLabel: 'Sanne' },
    { ...at(7), kind: 'link', id: 1, url: 'https://excalidraw.com/#room=a,b', title: 'Board', by: 'Nick', sessionOnly: true },
    ...prompt(2, 10, 20, [...['a.ts', 'b.ts', 'c.ts'].flatMap((path, i) => edit('p2', `u${i}`, 11, `src/${path}`)), ...edit('p2', 'u9', 12, 'notes.md'), plan(13, 'viewer-heartbeat', 3, 6)]),
  ];

  it('sums up what happened before joining', () => {
    const joined = T0 + 40 * MIN;
    const summary = soFar(events.slice(0, eventsBefore(events, joined)), joined);
    expect(summary).toMatchObject({
      ranMs: 40 * MIN,
      prompts: 2,
      documents: ['openspec/changes/viewer-heartbeat/proposal.md', 'notes.md'],
      shared: [{ file: 1, name: 'wireframe.png' }],
      links: [1],
      codeFiles: 3,
    });
    expect(summary.plans.map((p) => [p.change, p.tasks.done, p.tasks.total])).toEqual([['viewer-heartbeat', 3, 6]]);
  });

  it('ignores what happened after joining', () => {
    const joined = T0 + 30 * MIN;
    const later = [...events, ...prompt(3, 45, 50, [...edit('p3', 'v', 46, 'src/e.ts'), plan(47, 'viewer-heartbeat', 4, 6)])];
    expect(soFar(later.slice(0, eventsBefore(later, joined)), joined)).toEqual(soFar(events, joined));
  });

  it('says how long the session ran', () => {
    expect(ranFor(30_000)).toBeNull();
    expect(ranFor(40 * MIN)).toBe('40 minutes');
    expect(ranFor(MIN)).toBe('1 minute');
    expect(ranFor(65 * MIN + 59_000)).toBe('1 hour 5 minutes');
    expect(ranFor(120 * MIN)).toBe('2 hours');
  });
});
