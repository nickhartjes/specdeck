import { describe, expect, it } from 'vitest';
import { liveLine, planChangeOf, summarize, type Step } from './steps';
import type { FileChange, ToolItem, ToolStatus, Turn } from './timeline';

let n = 0;
function tool(toolName: string, input: Record<string, unknown>, status: ToolStatus = 'succeeded', changes: FileChange[] = []): ToolItem {
  n += 1;
  return { kind: 'tool', id: `t${n}`, toolName, input, status, changes, outputSeq: n, at: n };
}

function change(path: string, kind: FileChange['change'] = 'edited'): FileChange {
  return { path, change: kind, hunks: [], promptId: 'p', toolUseId: 't', at: 0, outputSeq: 0 };
}

function turn(items: Turn['items']): Turn {
  return { promptId: 'p', prompt: 'do it', at: 0, outputSeq: 0, items, response: null, tokens: null, revision: 0 };
}

const steps = (items: Turn['items'], seenChanges = new Set<string>()) =>
  summarize(turn(items), seenChanges).filter((entry): entry is Step => entry.kind === 'step');

describe('summary steps', () => {
  it('describes a command by what Claude said it does, in the past tense once done', () => {
    const [done, failed, running, odd] = steps([
      tool('Bash', { command: 'npm test', description: 'Run the test suite' }),
      tool('Bash', { command: 'npm test', description: 'Run the test suite' }, 'failed'),
      tool('Bash', { command: 'git rev-parse HEAD', description: 'Check the current commit' }, 'running'),
      tool('Bash', { command: 'x', description: 'Frobnicate the widgets' }),
    ]);
    expect([done!.label, done!.outcome]).toEqual(['Ran the test suite', 'succeeded']);
    expect([failed!.label, failed!.outcome]).toEqual(['Ran the test suite', 'failed']);
    expect([running!.label, running!.live]).toEqual(['Checking the current commit', 'checking the current commit']);
    expect(odd!.label).toBe('Frobnicate the widgets');
  });

  it('says a command ran when it has no description, and keeps the command for its detail', () => {
    const [step] = steps([tool('Bash', { command: 'make' })]);
    expect(step!.label).toBe('Ran a command');
    expect(step!.tools[0]!.input.command).toBe('make');
  });

  it('groups reads and searches, and file changes', () => {
    const result = steps([
      tool('Read', { file_path: '/w/a.ts' }),
      tool('Read', { file_path: '/w/b.ts' }),
      tool('Read', { file_path: '/w/a.ts' }),
      tool('Grep', { pattern: 'health' }),
      tool('Edit', { file_path: '/w/a.ts' }, 'succeeded', [change('a.ts')]),
      tool('Write', { file_path: '/w/c.ts' }, 'failed', [change('c.ts', 'created')]),
      tool('Read', { file_path: '/w/d.ts' }),
    ]);
    expect(result.map((s) => [s.label, s.outcome])).toEqual([
      ['Looked at 2 files and searched the code', 'succeeded'],
      ['Changed 2 files', 'failed'],
      ['Looked at 1 file', 'succeeded'],
    ]);
    expect(result[1]!.tools).toHaveLength(2);
  });

  it('calls changes to an OpenSpec change writing or updating the plan', () => {
    const plan = (file: string) => tool('Write', { file_path: `/w/openspec/changes/demo/${file}` }, 'succeeded', [change(`openspec/changes/demo/${file}`, 'created')]);
    const seen = new Set<string>();
    const first = steps([plan('proposal.md'), plan('tasks.md')], seen);
    expect(first.map((s) => [s.label, s.plan])).toEqual([['Wrote the plan', 'demo']]);
    const later = steps([plan('design.md')], seen);
    expect(later.map((s) => s.label)).toEqual(['Updated the plan']);
  });

  it('names subagents, questions, to-do lists and other tools', () => {
    expect(
      steps([
        tool('Task', { description: 'Review the API changes' }),
        tool('AskUserQuestion', { questions: [{ question: 'Which database?' }] }),
        tool('TodoWrite', { todos: [] }),
        tool('WebFetch', { url: 'https://example.com' }),
      ]).map((s) => s.label),
    ).toEqual(['Asked a helper: Review the API changes', 'Asked: Which database?', 'Updated its to-do list', 'Used WebFetch']);
  });

  it('keeps the agent text and waiting notices between the steps', () => {
    const entries = summarize(
      turn([
        { kind: 'text', id: 'm1', text: 'Let me look.', outputSeq: 1 },
        tool('Read', { file_path: '/w/a.ts' }),
        { kind: 'notice', id: 'n1', text: 'Claude needs your permission to use Bash', outputSeq: 2 },
      ]),
      new Set(),
    );
    expect(entries.map((e) => e.kind)).toEqual(['text', 'step', 'notice']);
  });
});

describe("Claude's browser", () => {
  const browser = (name: string, input: Record<string, unknown> = {}, status?: ToolStatus) => tool(`mcp__specdeck__${name}`, input, status);

  it('says what one call did, with the address', () => {
    const [opened] = steps([browser('navigate', { url: 'http://localhost:5173/login' })]);
    expect(opened!.label).toBe('Opened localhost:5173/login in its browser');
    // The details keep the tool's own name and input.
    expect(opened!.tools[0]!.toolName).toBe('mcp__specdeck__navigate');
  });

  it('makes one step of a run of browser calls, and says what the last one does while it runs', () => {
    const run = [
      browser('navigate', { url: 'http://localhost:5173/login' }),
      browser('type', { element: 2, text: 'anna@example.com' }),
      browser('click', { element: 4 }),
      browser('press', { key: 'Enter' }),
      browser('screenshot', {}, 'running'),
    ];
    const [step] = steps(run);
    expect(step!.label).toBe(
      'Used its browser: opened localhost:5173/login, typed on the page, clicked on the page, pressed Enter, took a screenshot',
    );
    expect(step!.live).toBe('using its browser: taking a screenshot');
  });

  it('names every tool in plain words', () => {
    const labels = ['read_page', 'back', 'console', 'screenshot'].map((name) => steps([browser(name)])[0]!.label);
    expect(labels).toEqual([
      'Read the page in its browser',
      'Went back a page in its browser',
      'Read the console in its browser',
      'Took a screenshot in its browser',
    ]);
  });

  it('keeps other MCP tools as they are', () => {
    expect(steps([tool('mcp__github__create_issue', {})])[0]!.label).toBe('Used mcp__github__create_issue');
  });
});

describe('live line', () => {
  it('says what the running step does, or what the agent waits for', () => {
    const running = turn([tool('Bash', { description: 'Run the test suite' }, 'running')]);
    expect(liveLine(running, new Set(), 'Claude')).toBe('Claude is running the test suite');
    const waiting = turn([tool('Bash', { description: 'Run it' }, 'running'), { kind: 'notice', id: 'n', text: 'Claude needs your permission to use Bash', outputSeq: 9 }]);
    expect(liveLine(waiting, new Set(), 'Claude')).toBe('Waiting for the host: Claude needs your permission to use Bash');
    expect(liveLine(turn([tool('Read', { file_path: '/a' }, 'running')]), new Set(), 'Claude')).toBe('Claude is looking at 1 file');
    expect(liveLine(turn([tool('Read', { file_path: '/a' })]), new Set(), 'Claude')).toBeNull();
  });
});

describe('plan files', () => {
  it('belong to a change unless archived', () => {
    expect(planChangeOf('openspec/changes/viewer-views/specs/a/spec.md')).toBe('viewer-views');
    expect(planChangeOf('/repo/openspec/changes/demo/tasks.md')).toBe('demo');
    expect(planChangeOf('openspec/changes/archive/2026-10-31-x/tasks.md')).toBeNull();
    expect(planChangeOf('openspec/specs/a/spec.md')).toBeNull();
  });
});

describe('host interactions', () => {
  const questions = [
    { question: 'Which database?', options: [{ label: 'Postgres' }, { label: 'SQLite' }] },
    { question: 'Which features?', multiSelect: true, options: [{ label: 'Auth' }, { label: 'Export' }] },
  ];
  const asked = (status: ToolStatus, outcome?: ToolItem['outcome']) => ({ ...tool('AskUserQuestion', { questions }, status), outcome });

  it('names the first question and its answer, and how many more there were', () => {
    const answers = [
      { question: 'Which database?', selected: ['Postgres'] },
      { question: 'Which features?', selected: ['Auth'], typed: 'Audit log' },
    ];
    const [answered, waiting, dismissed] = steps([
      asked('succeeded', { kind: 'answers', answers }),
      asked('running'),
      asked('not-run'),
    ]);
    expect(answered!.label).toBe('Asked: Which database? → Postgres +1');
    expect(waiting!.label).toBe('Asked: Which database?');
    expect([dismissed!.label, dismissed!.outcome]).toEqual(['Asked: Which database?', 'not-run']);
    const typed = steps([asked('succeeded', { kind: 'answers', answers: [{ question: 'Which database?', selected: [], typed: 'MySQL' }] })]);
    expect(typed[0]!.label).toBe('Asked: Which database? → MySQL');
  });

  it('says whether a plan was approved or rejected', () => {
    const plan = (status: ToolStatus, approved?: boolean) => ({
      ...tool('ExitPlanMode', { plan: '# Plan' }, status),
      outcome: approved === undefined ? undefined : ({ kind: 'plan-decision', approved } as const),
    });
    expect(steps([plan('running'), plan('not-run', false), plan('succeeded', true)]).map((s) => [s.label, s.outcome])).toEqual([
      ['Proposed a plan', 'running'],
      ['Plan rejected', 'denied'],
      ['Plan approved', 'succeeded'],
    ]);
  });

  it('marks a command the host denied', () => {
    const denied = { ...tool('Bash', { description: 'Delete the build folder' }, 'not-run'), permission: 'denied' as const };
    const allowed = { ...tool('Bash', { description: 'Run the test suite' }), permission: 'allowed' as const };
    expect(steps([denied, allowed]).map((s) => [s.label, s.outcome])).toEqual([
      ['Delete the build folder', 'denied'],
      ['Ran the test suite', 'succeeded'],
    ]);
  });

  it('waits for the host instead of a permission while a question or plan is open', () => {
    const notice = { kind: 'notice', id: 'n', text: 'Claude needs your permission', outputSeq: 9 } as const;
    expect(liveLine(turn([asked('running'), notice]), new Set(), 'Claude')).toBe("Waiting for the host's answer: Which database?");
    expect(liveLine(turn([tool('ExitPlanMode', { plan: 'x' }, 'running'), notice]), new Set(), 'Claude')).toBe(
      'Waiting for the host to approve the plan',
    );
    expect(liveLine(turn([tool('Bash', { description: 'Run it' }, 'running'), notice]), new Set(), 'Claude')).toBe(
      'Waiting for the host: Claude needs your permission',
    );
  });
});
