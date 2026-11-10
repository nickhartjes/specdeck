import { describe, expect, it } from 'vitest';
import type { TimelineEvent, ToolOutcome } from '../api/types';
import { buildTimeline, SHOWN_IN_PART, toolSummary, turnText } from './timeline';

let at = 1000;
const stamp = () => ({ outputSeq: at - 1000, at: at++ });

// The edit session recorded from Claude Code (crates/core/tests/fixtures/claude-hooks).
const editSession: TimelineEvent[] = [
  { ...stamp(), kind: 'session-started' },
  { ...stamp(), kind: 'prompt', promptId: 'p1', text: 'Write notes.txt, then change it' },
  { ...stamp(), kind: 'message', promptId: 'p1', messageId: 'm1', text: 'Writing ', final: false },
  { ...stamp(), kind: 'message', promptId: 'p1', messageId: 'm1', text: 'the file.', final: true },
  { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 't1', toolName: 'Write', input: { file_path: 'notes.txt', content: 'one\n' } },
  { ...stamp(), kind: 'tool-result', promptId: 'p1', toolUseId: 't1', status: 'succeeded' },
  { ...stamp(), kind: 'file-change', promptId: 'p1', toolUseId: 't1', path: 'notes.txt', change: 'created', hunks: [{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 1, lines: ['+one'] }] },
  { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 't2', toolName: 'Edit', input: { file_path: 'notes.txt' } },
  { ...stamp(), kind: 'tool-result', promptId: 'p1', toolUseId: 't2', status: 'succeeded' },
  { ...stamp(), kind: 'file-change', promptId: 'p1', toolUseId: 't2', path: 'notes.txt', change: 'edited', hunks: [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines: ['-one', '+two'] }] },
  { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 't3', toolName: 'Bash', input: { command: 'echo $PROBE' } },
  { ...stamp(), kind: 'tool-result', promptId: 'p1', toolUseId: 't3', status: 'not-run' },
  { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 't4', toolName: 'Read', input: { file_path: 'README.md' } },
  { ...stamp(), kind: 'tool-result', promptId: 'p1', toolUseId: 't4', status: 'succeeded' },
  { ...stamp(), kind: 'file-read', promptId: 'p1', toolUseId: 't4', path: 'README.md' },
  { ...stamp(), kind: 'response', promptId: 'p1', text: 'I wrote notes.txt and changed it.' },
];

describe('timeline model', () => {
  it('groups a prompt with its text and tool calls in order', () => {
    const { turns } = buildTimeline(editSession);
    expect(turns).toHaveLength(1);
    const turn = turns[0]!;
    expect(turn.prompt).toBe('Write notes.txt, then change it');
    expect(turn.items.map((item) => item.kind)).toEqual(['text', 'tool', 'tool', 'tool', 'tool']);
    expect(turn.items[0]).toMatchObject({ kind: 'text', text: 'Writing the file.' });
    expect(turn.response).toBe('I wrote notes.txt and changed it.');
  });

  it('keeps when the reply came', () => {
    const turn = buildTimeline(editSession).turns[0]!;
    expect(turn.respondedAt).toBe(editSession.at(-1)!.at);
    expect(buildTimeline(editSession.slice(0, -1)).turns[0]!.respondedAt).toBeNull();
  });

  it('pairs tool calls with their results', () => {
    const tools = buildTimeline(editSession).turns[0]!.items.filter((item) => item.kind === 'tool');
    expect(tools.map((tool) => tool.status)).toEqual(['succeeded', 'succeeded', 'not-run', 'succeeded']);
    expect(tools[0]!.changes[0]!.change).toBe('created');
  });

  it('lists touched files, changed ones first, with every diff in order', () => {
    const { files } = buildTimeline(editSession);
    expect(files.map((file) => [file.path, file.readOnly])).toEqual([
      ['notes.txt', false],
      ['README.md', true],
    ]);
    expect(files[0]!.changes.map((change) => change.change)).toEqual(['created', 'edited']);
  });

  it('tracks when the timeline is unavailable and available again', () => {
    const unavailable: TimelineEvent = { ...stamp(), kind: 'timeline-unavailable', reason: 'No events arrived from the agent.' };
    expect(buildTimeline([unavailable]).unavailable).toBe('No events arrived from the agent.');
    expect(buildTimeline([unavailable, { ...stamp(), kind: 'timeline-available' }]).unavailable).toBeNull();
    expect(buildTimeline([unavailable]).unavailableSandbox).toBe(false);
  });

  it('marks the explanation for a sandbox, and ignores a help page it does not know', () => {
    const sandbox: TimelineEvent = { ...stamp(), kind: 'timeline-unavailable', reason: 'SpecDeck has not been reached from the sandbox yet.', help: 'sandbox' };
    expect(buildTimeline([sandbox]).unavailableSandbox).toBe(true);
    expect(buildTimeline([sandbox, { ...stamp(), kind: 'timeline-available' }]).unavailableSandbox).toBe(false);
    const unknown: TimelineEvent = { ...stamp(), kind: 'timeline-unavailable', reason: 'Something else.', help: 'unknown' };
    expect(buildTimeline([sandbox, unknown]).unavailableSandbox).toBe(false);
  });

  it('summarises tool calls and makes turns searchable', () => {
    const turn = buildTimeline(editSession).turns[0]!;
    const bash = turn.items.find((item) => item.kind === 'tool' && item.toolName === 'Bash');
    expect(bash && bash.kind === 'tool' && toolSummary(bash)).toBe('echo $PROBE');
    expect(turnText(turn)).toContain('Writing the file.');
    expect(turnText(turn)).toContain('echo $PROBE');
  });
});

describe('usage', () => {
  it('gives each prompt the tokens used since it started', () => {
    const usage = (input: number, output: number): TimelineEvent => ({ ...stamp(), kind: 'usage', tokens: { input, output, cacheRead: 0, cacheWrite: 0 } });
    const { turns } = buildTimeline([
      { ...stamp(), kind: 'prompt', promptId: 'p1', text: 'one' },
      usage(100, 20),
      usage(300, 50),
      { ...stamp(), kind: 'prompt', promptId: 'p2', text: 'two' },
      { ...stamp(), kind: 'prompt', promptId: 'p3', text: 'three' },
      usage(400, 90),
    ]);
    expect(turns.map((turn) => turn.tokens)).toEqual([350, null, 140]);
  });
});

describe('attention', () => {
  it('shows where the agent waited for the host', () => {
    const { turns } = buildTimeline([
      { ...stamp(), kind: 'prompt', promptId: 'p1', text: 'run the tests' },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 't1', toolName: 'Bash', input: { command: 'npm test' } },
      { ...stamp(), kind: 'attention', message: 'Claude needs your permission to use Bash' },
    ]);
    expect(turns[0]!.items.at(-1)).toMatchObject({ kind: 'notice', text: 'Claude needs your permission to use Bash' });
    expect(turnText(turns[0]!)).toContain('permission to use Bash');
  });
});

describe('host interactions', () => {
  it('keeps what the host decided and what a helper reported later', () => {
    const answers: ToolOutcome = { kind: 'answers', answers: [{ question: 'Which database?', selected: ['Postgres'] }] };
    const { turns } = buildTimeline([
      { ...stamp(), kind: 'prompt', promptId: 'p1', text: 'go' },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 'q', toolName: 'AskUserQuestion', input: {} },
      { ...stamp(), kind: 'tool-result', promptId: 'p1', toolUseId: 'q', status: 'succeeded', outcome: answers },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 'b', toolName: 'Bash', input: {} },
      { ...stamp(), kind: 'tool-result', promptId: 'p1', toolUseId: 'b', status: 'not-run', permission: 'denied' },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 'h', toolName: 'Agent', input: {} },
      { ...stamp(), kind: 'tool-result', promptId: 'p1', toolUseId: 'h', status: 'succeeded' },
      { ...stamp(), kind: 'tool-result', promptId: 'p1', toolUseId: 'h', status: 'succeeded', outcome: { kind: 'report', text: 'Done.', truncated: false } },
    ]);
    expect(turns[0]!.items).toMatchObject([
      { toolName: 'AskUserQuestion', status: 'succeeded', outcome: answers },
      { toolName: 'Bash', status: 'not-run', permission: 'denied' },
      { toolName: 'Agent', status: 'succeeded', outcome: { kind: 'report', text: 'Done.' } },
    ]);
  });
});

describe('entries the host cut to fit in one message', () => {
  it('carry a note, and whole entries do not', () => {
    const { turns, files } = buildTimeline([
      { ...stamp(), kind: 'prompt', promptId: 'p1', text: 'a long prompt', truncated: true },
      { ...stamp(), kind: 'message', promptId: 'p1', messageId: 'm1', text: 'short', final: true },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 't1', toolName: 'Bash', input: { command: 'cut' }, truncated: true },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 't2', toolName: 'Write', input: { file_path: 'big.rs' } },
      { ...stamp(), kind: 'file-change', promptId: 'p1', toolUseId: 't2', path: 'big.rs', change: 'created', hunks: [], truncated: true },
      { ...stamp(), kind: 'response', promptId: 'p1', text: 'done', truncated: true },
    ]);
    const turn = turns[0]!;
    expect(turn.prompt).toBe(`a long prompt\n\n${SHOWN_IN_PART}`);
    expect(turn.response).toBe(`done\n\n${SHOWN_IN_PART}`);
    expect(turn.items[0]).toMatchObject({ kind: 'text', text: 'short' });
    expect(turn.items[1]).toMatchObject({ kind: 'tool', truncated: true });
    expect(turn.items[2]).not.toHaveProperty('truncated');
    expect(files[0]!.changes[0]).toMatchObject({ path: 'big.rs', truncated: true });
  });
});

describe('prompts sent from a suggestion', () => {
  const mark = (by: string, edited = false): TimelineEvent => ({ ...stamp(), kind: 'suggested', by, edited });
  const prompt = (n: number): TimelineEvent => ({ ...stamp(), kind: 'prompt', promptId: `p${n}`, text: `Prompt ${n}` });

  it('marks only the next prompt', () => {
    const model = buildTimeline([prompt(1), mark('Sanne', true), prompt(2), prompt(3)]);
    expect(model.turns.map((turn) => turn.suggested)).toEqual([undefined, { by: 'Sanne', edited: true }, undefined]);
    expect(model.suggested.map((item) => item.by)).toEqual(['Sanne']);
  });

  it('takes the last of two marks before a prompt, which was the one submitted', () => {
    const model = buildTimeline([mark('Sanne'), mark('Mo'), prompt(1)]);
    expect(model.turns[0]!.suggested).toEqual({ by: 'Mo', edited: false });
  });

  it('marks a prompt in a late joiner’s history', () => {
    // The history starts in the middle of a turn, whose prompt came before it.
    const model = buildTimeline([
      { ...stamp(), kind: 'message', promptId: 'p0', messageId: 'm0', text: 'Earlier', final: true },
      mark('Sanne'),
      prompt(1),
    ]);
    expect(model.turns.map((turn) => turn.suggested?.by)).toEqual([undefined, 'Sanne']);
  });
});

describe('shared links', () => {
  const link = (id: number, url: string | null, title?: string): TimelineEvent => ({ ...stamp(), kind: 'link', id, url, title, by: 'Nick', sessionOnly: false });

  it('places a link under the prompt it was shared during, and one before any prompt on its own', () => {
    const model = buildTimeline([
      link(1, 'https://example.com/early'),
      { ...stamp(), kind: 'prompt', promptId: 'p1', text: 'Go' },
      { ...stamp(), kind: 'message', promptId: 'p1', messageId: 'm1', text: 'Working', final: true },
      link(2, 'https://example.com/during', 'Staging'),
    ]);
    expect(model.earlyLinks.map((l) => l.linkId)).toEqual([1]);
    expect(model.turns[0]!.items.map((item) => item.kind)).toEqual(['text', 'link']);
    expect(model.links.map((l) => l.url)).toEqual(['https://example.com/early', 'https://example.com/during']);
    expect(turnText(model.turns[0]!)).toContain('Staging https://example.com/during');
  });

  it('drops the address of a removed link and renders its turn again', () => {
    const shared = [{ ...stamp(), kind: 'prompt', promptId: 'p1', text: 'Go' } as TimelineEvent, link(1, 'https://example.com/')];
    const before = buildTimeline(shared).turns[0]!.revision;
    const after = buildTimeline([...shared, { ...stamp(), kind: 'link-removed', id: 1 }]);
    expect(after.links[0]!.url).toBeNull();
    expect(after.turns[0]!.revision).toBeGreaterThan(before);
  });

  it('does not search the key of a link for this session only', () => {
    const model = buildTimeline([
      { ...stamp(), kind: 'prompt', promptId: 'p1', text: 'Go' },
      { ...stamp(), kind: 'link', id: 1, url: 'https://excalidraw.com/#room=a,SECRET', title: 'Draw', by: 'Nick', sessionOnly: true },
    ]);
    expect(turnText(model.turns[0]!)).not.toContain('SECRET');
  });
});

describe("Claude's browser", () => {
  it('keeps the element a click acts on', () => {
    const target = { kind: 'button', label: 'Sign in', sensitive: false };
    const { turns } = buildTimeline([
      { ...stamp(), kind: 'prompt', promptId: 'p1', text: 'sign in' },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 'c', toolName: 'mcp__specdeck__click', input: { element: 3 }, target },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 'b', toolName: 'Bash', input: {} },
    ]);
    expect(turns[0]!.items[0]).toMatchObject({ toolName: 'mcp__specdeck__click', target });
    expect(turns[0]!.items[1]).not.toHaveProperty('target');
  });

  it("keeps the elements of another browser's batch", () => {
    const targets = [{ kind: 'email field', label: 'Email', sensitive: false }, null];
    const { turns } = buildTimeline([
      { ...stamp(), kind: 'prompt', promptId: 'p1', text: 'sign in' },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 'c', toolName: 'mcp__claude-in-chrome__browser_batch', input: { actions: [] }, targets },
      { ...stamp(), kind: 'tool-call', promptId: 'p1', toolUseId: 'b', toolName: 'Bash', input: {} },
    ]);
    expect(turns[0]!.items[0]).toMatchObject({ targets });
    expect(turns[0]!.items[1]).not.toHaveProperty('targets');
  });
});
