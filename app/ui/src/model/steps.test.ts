import { describe, expect, it } from 'vitest';
import type { Screenshot, Target } from '../api/types';
import { liveLine, planChangeOf, screenshotOf, summarize, type Step } from './steps';
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
  return { promptId: 'p', prompt: 'do it', at: 0, outputSeq: 0, items, response: null, respondedAt: null, tokens: null, revision: 0 };
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

  describe('names what Claude clicked and typed into', () => {
    const on = (name: string, input: Record<string, unknown>, target?: Target, status?: ToolStatus): ToolItem => ({
      ...browser(name, input, status),
      ...(target && { target }),
    });
    const button: Target = { kind: 'button', label: 'Sign in', sensitive: false };
    const email: Target = { kind: 'email field', label: 'Email', sensitive: false };
    const password: Target = { kind: 'password field', label: 'Password', sensitive: true };
    const one = (item: ToolItem) => steps([item])[0]!;

    it('by label, by kind, or not at all', () => {
      expect(one(on('click', { element: 3 }, button)).label).toBe('Clicked “Sign in” in its browser');
      expect(one(on('click', { element: 3 }, { kind: 'button', sensitive: false })).label).toBe('Clicked a button in its browser');
      expect(one(on('click', { element: 3 })).label).toBe('Clicked on the page in its browser');
      expect(one(on('type', { element: 1, text: 'anna@example.com' }, email)).label).toBe('Typed into “Email” in its browser');
      expect(one(on('type', { element: 1, text: 'x' }, { kind: 'email field', sensitive: false })).label).toBe(
        'Typed into an email field in its browser',
      );
      expect(one(on('type', { element: 2 }, password)).label).toBe('Filled in “Password” in its browser');
      expect(one(on('type', { element: 2 }, { kind: 'password field', sensitive: true })).label).toBe(
        'Filled in a password field in its browser',
      );
      // A field the session didn't know: its text was hidden.
      expect(one(on('type', { element: 9 })).label).toBe('Filled in a field in its browser');
      // An older host sends no element, and the text.
      expect(one(on('type', { element: 9, text: 'x' })).label).toBe('Typed on the page in its browser');
    });

    it('while it runs', () => {
      expect(one(on('click', { element: 3 }, button, 'running')).live).toBe('clicking “Sign in” in its browser');
      expect(one(on('type', { element: 2 }, password, 'running')).live).toBe('filling in “Password” in its browser');
    });

    it('in a run, with long labels cut', () => {
      const [step] = steps([
        browser('navigate', { url: 'http://localhost:5173/login' }),
        on('type', { element: 1, text: 'anna@example.com' }, email),
        on('type', { element: 2 }, password),
        on('click', { element: 3 }, button),
        on('click', { element: 4 }, { kind: 'link', label: 'Read the terms and conditions of this shop before you order', sensitive: false }),
      ]);
      expect(step!.label).toBe(
        'Used its browser: opened localhost:5173/login, typed into “Email”, filled in “Password”, clicked “Sign in”, clicked “Read the terms and conditions of this s…”',
      );
    });
  });
});

describe('other browsers', () => {
  const text = (label: string): Target => ({ kind: 'text field', label, sensitive: false });
  const email: Target = { kind: 'email field', label: 'Email', sensitive: false };
  const password: Target = { kind: 'password field', label: 'Password', sensitive: true };
  const signIn: Target = { kind: 'button', label: 'Sign in', sensitive: false };
  const on = (toolName: string, input: Record<string, unknown>, acts: Pick<ToolItem, 'target' | 'targets'> = {}, status?: ToolStatus): ToolItem => ({
    ...tool(toolName, input, status),
    ...acts,
  });
  const playwright = (name: string, input: Record<string, unknown>, acts?: Pick<ToolItem, 'target' | 'targets'>) =>
    on(`mcp__playwright__browser_${name}`, input, acts);
  const chrome = (name: string, input: Record<string, unknown>, acts?: Pick<ToolItem, 'target' | 'targets'>) =>
    on(`mcp__claude-in-chrome__${name}`, input, acts);

  it('makes one step of a Playwright run, its typed text hidden', () => {
    // The calls of `playwright-steps`, as the host sends them.
    const [step] = steps([
      playwright('navigate', { url: 'http://127.0.0.1:5179/' }),
      playwright('snapshot', {}),
      playwright('type', { element: 'Email field', target: 'e5' }, { target: text('Email') }),
      playwright('type', { element: 'Password field', target: 'e7' }, { target: text('Password') }),
      playwright('type', { element: 'Code field', target: 'e9' }, { target: text('Code') }),
      playwright(
        'fill_form',
        { fields: [{ name: 'Card number', target: 'e10', type: 'textbox' }, { name: 'Notes', target: 'e12', type: 'textbox' }] },
        { targets: [text('Card number'), text('Notes')] },
      ),
      playwright('click', { element: 'Unlabeled ✕ button', target: 'e14' }, { target: { kind: 'button', sensitive: false } }),
      playwright('click', { element: 'Sign in button', target: 'e13' }, { target: signIn }),
    ]);
    expect(step!.label).toBe(
      'Used its browser: opened 127.0.0.1:5179/, read the page, filled in “Email”, filled in “Password”, filled in “Code”, filled in “Card number” and “Notes”, clicked a button, clicked “Sign in”',
    );
  });

  it("names an element by Claude's own words without a read", () => {
    expect(steps([playwright('click', { element: 'Sign in button', target: 'e13' })])[0]!.label).toBe('Clicked Sign in button in its browser');
    expect(steps([chrome('computer', { action: 'left_click', coordinate: [1, 2], action_summary: 'Submits the sign-in form' })])[0]!.label).toBe(
      'Submits the sign-in form in its browser',
    );
    expect(steps([chrome('computer', { action: 'left_click', coordinate: [1, 2] })])[0]!.label).toBe('Clicked on the page in its browser');
  });

  it('shows typing into a known ordinary field, and fills in the rest', () => {
    const [step] = steps([
      chrome('computer', { action: 'left_click', ref: 'ref_4' }, { target: email }),
      chrome('computer', { action: 'type', text: 'test@example.com' }, { target: email }),
      chrome('computer', { action: 'left_click', ref: 'ref_6' }, { target: password }),
      chrome('computer', { action: 'type' }, { target: password }),
      chrome('computer', { action: 'type' }),
      chrome('form_input', { ref: 'ref_8' }, { target: text('Code') }),
      chrome('form_input', { ref: 'ref_20', value: true }, { target: { kind: 'checkbox', label: 'Remember me', sensitive: false } }),
    ]);
    expect(step!.label).toBe(
      'Used its browser: clicked “Email”, typed into “Email”, clicked “Password”, filled in “Password”, filled in a field, filled in “Code”, set “Remember me”',
    );
  });

  it('lists each action of a Claude in Chrome batch', () => {
    // The login batch of `chrome-batch`, as the host sends it.
    const batch = chrome(
      'browser_batch',
      {
        actions: [
          { name: 'computer', input: { action: 'left_click', ref: 'ref_1', action_summary: 'Focuses the Email field' } },
          { name: 'computer', input: { action: 'type', text: 'test@example.com', action_summary: 'Types the test email address' } },
          { name: 'computer', input: { action: 'left_click', ref: 'ref_2', action_summary: 'Focuses the Password field' } },
          { name: 'computer', input: { action: 'type', action_summary: 'Types the test password' } },
          { name: 'form_input', input: { ref: 'ref_3', value: '123456', action_summary: 'Sets the Code field' } },
          { name: 'computer', input: { action: 'left_click', ref: 'ref_6', action_summary: 'Submits the sign-in form' } },
        ],
      },
      { targets: [email, email, password, password, text('Code'), signIn] },
    );
    const [step] = steps([chrome('tabs_create_mcp', {}), batch]);
    expect(step!.label).toBe(
      'Used its browser: opened a tab, clicked “Email”, typed into “Email”, clicked “Password”, filled in “Password”, set “Code”, clicked “Sign in”',
    );
    expect(steps([{ ...batch, status: 'running' }])[0]!.live).toBe('using its browser: clicking “Sign in”');
  });

  it('names the other tools in plain words', () => {
    const labels = [
      playwright('navigate_back', {}),
      playwright('press_key', { key: 'Enter' }),
      playwright('take_screenshot', {}),
      playwright('hover', { element: 'Menu', target: 'e3' }),
      playwright('select_option', { element: 'Size', target: 'e4', values: ['L'] }),
      playwright('click', { element: 'Row', target: 'e5', doubleClick: true }),
      playwright('tabs', { action: 'new' }),
      playwright('evaluate', { function: '() => 1' }),
      playwright('network_requests', {}),
      chrome('read_page', { tabId: 1 }),
      chrome('tabs_context_mcp', {}),
      chrome('computer', { action: 'key', text: 'Tab' }),
      chrome('computer', { action: 'scroll' }),
      chrome('gif_creator', {}),
    ].map((call) => steps([call])[0]!.label);
    expect(labels).toEqual([
      'Went back a page in its browser',
      'Pressed Enter in its browser',
      'Took a screenshot in its browser',
      'Hovered over Menu in its browser',
      'Chose an option in Size in its browser',
      'Double-clicked Row in its browser',
      'Opened a tab in its browser',
      'Ran a script on the page in its browser',
      'Used browser_network_requests in its browser',
      'Read the page in its browser',
      'Looked at its tabs in its browser',
      'Pressed Tab in its browser',
      'Scrolled the page in its browser',
      'Used gif_creator in its browser',
    ]);
    // A server with another tool name is no browser.
    expect(steps([tool('mcp__playwright__something_else', {})])[0]!.label).toBe('Used mcp__playwright__something_else');
  });

  it("says typed into for an older host's call that still has its text", () => {
    expect(steps([playwright('type', { element: 'Email field', target: 'e5', text: 'a@b.c' })])[0]!.label).toBe('Typed into Email field in its browser');
    expect(steps([chrome('computer', { action: 'type', text: 'a@b.c' })])[0]!.label).toBe('Typed into the page in its browser');
  });

  it("joins SpecDeck's own browser calls in one step", () => {
    const [step] = steps([tool('mcp__specdeck__navigate', { url: 'http://localhost:5173/' }), playwright('snapshot', {})]);
    expect(step!.label).toBe('Used its browser: opened localhost:5173/, read the page');
  });
});

describe('screenshots', () => {
  const shot = (address: string): Screenshot => ({ kind: 'screenshot', image: { mediaType: 'image/jpeg', data: 'x', width: 8, height: 5 }, address });
  const browser = (name: string, outcome?: Screenshot) => ({ ...tool(`mcp__specdeck__${name}`, {}), ...(outcome && { outcome }) });

  it('gives a browser run its latest screenshot, and each call keeps its own', () => {
    const first = browser('screenshot', shot('localhost:5173/login'));
    const second = browser('screenshot', shot('localhost:5173/home'));
    const [step] = steps([browser('navigate'), first, browser('click'), second]);
    expect(step!.screenshot).toBe(second);
    expect(step!.tools.map((t) => screenshotOf(t)?.address ?? null)).toEqual([null, 'localhost:5173/login', null, 'localhost:5173/home']);
  });

  it('gives a single call of another MCP server its screenshot', () => {
    const playwright = { ...tool('mcp__playwright__browser_take_screenshot', {}), outcome: shot('') };
    expect(steps([playwright])[0]!.screenshot).toBe(playwright);
  });

  it('keeps a screenshot that was left out, with its reason', () => {
    const left: Screenshot = { kind: 'screenshot', leftOut: 'turned-off' };
    const [step] = steps([browser('screenshot', left)]);
    expect(screenshotOf(step!.screenshot!)).toEqual(left);
  });

  it('gives steps without a screenshot none', () => {
    expect(steps([browser('navigate'), browser('click')])[0]!.screenshot).toBeUndefined();
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
