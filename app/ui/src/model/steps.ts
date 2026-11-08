// What the agent did for a prompt, in plain words (viewer-views design decision 6): each tool call,
// or each run of reads or file changes, becomes one step line that opens to its technical detail.

import type { Screenshot, Target } from '../api/types';
import type { AskItem, LinkItem, NoticeItem, TextItem, ToolItem, ToolStatus, Turn } from './timeline';

/** `denied`: the host did not allow it, or rejected the plan it presented. */
export type Outcome = 'running' | 'succeeded' | 'failed' | 'not-run' | 'denied';

export interface Step {
  kind: 'step';
  id: string;
  /** What happened, as a summary line reads. */
  label: string;
  /** The same while it runs, after "Claude is …"; lower case. */
  live: string;
  outcome: Outcome;
  /** The calls this line stands for, shown when it is opened. */
  tools: ToolItem[];
  /** The OpenSpec change this step wrote or updated. */
  plan?: string;
  /** The call with the step's latest screenshot. */
  screenshot?: ToolItem;
}

export type SummaryEntry = Step | TextItem | NoticeItem | LinkItem | AskItem;

const SHELL = new Set(['Bash', 'PowerShell']);
const READS = new Set(['Read', 'Glob', 'Grep', 'LS']);
const CHANGES = new Set(['Write', 'Edit', 'MultiEdit', 'NotebookEdit']);
const SUBAGENTS = new Set(['Task', 'Agent']);
/** SpecDeck's own browser (`agent-browser`): its MCP server's tools. */
const BROWSER = 'mcp__specdeck__';

/** Past tense and -ing form of verbs Claude starts command descriptions with. */
const VERBS: Record<string, [string, string]> = {
  add: ['added', 'adding'], build: ['built', 'building'], check: ['checked', 'checking'], clean: ['cleaned', 'cleaning'],
  commit: ['committed', 'committing'], compile: ['compiled', 'compiling'], copy: ['copied', 'copying'], count: ['counted', 'counting'],
  create: ['created', 'creating'], delete: ['deleted', 'deleting'], download: ['downloaded', 'downloading'], fetch: ['fetched', 'fetching'],
  find: ['found', 'finding'], format: ['formatted', 'formatting'], generate: ['generated', 'generating'], get: ['got', 'getting'],
  inspect: ['inspected', 'inspecting'], install: ['installed', 'installing'], lint: ['linted', 'linting'], list: ['listed', 'listing'],
  look: ['looked', 'looking'], make: ['made', 'making'], move: ['moved', 'moving'], open: ['opened', 'opening'], print: ['printed', 'printing'],
  pull: ['pulled', 'pulling'], push: ['pushed', 'pushing'], read: ['read', 'reading'], remove: ['removed', 'removing'],
  rename: ['renamed', 'renaming'], run: ['ran', 'running'], search: ['searched', 'searching'], show: ['showed', 'showing'],
  start: ['started', 'starting'], stop: ['stopped', 'stopping'], test: ['tested', 'testing'], update: ['updated', 'updating'],
  validate: ['validated', 'validating'], verify: ['verified', 'verifying'], view: ['viewed', 'viewing'], write: ['wrote', 'writing'],
};

/** The OpenSpec change a file belongs to, or null (archived changes too). */
export function planChangeOf(path: string): string | null {
  const match = /(?:^|\/)openspec\/changes\/([^/]+)\/./.exec(path.replaceAll('\\', '/'));
  return match && match[1] !== 'archive' ? match[1]! : null;
}

/** The screenshot a call returned, if any. */
export function screenshotOf(tool: ToolItem): Screenshot | null {
  return tool.outcome?.kind === 'screenshot' ? tool.outcome : null;
}

function outcome(tools: ToolItem[]): Outcome {
  const statuses = new Set<ToolStatus>(tools.map((t) => t.status));
  if (statuses.has('running')) return 'running';
  const rejected = (t: ToolItem) => t.outcome?.kind === 'plan-decision' && !t.outcome.approved;
  if (tools.some((t) => t.permission === 'denied' || rejected(t))) return 'denied';
  if (statuses.has('failed')) return 'failed';
  if (statuses.size === 1 && statuses.has('not-run')) return 'not-run';
  return 'succeeded';
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
const text = (tool: ToolItem, key: string) => (typeof tool.input[key] === 'string' ? (tool.input[key] as string).trim() : '');
const lowerFirst = (s: string) => s.charAt(0).toLowerCase() + s.slice(1);
const upperFirst = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** "Run the test suite" → ["Ran the test suite", "running the test suite"]. */
function tenses(description: string): [string, string] {
  const [first = '', ...rest] = description.split(' ');
  const verb = VERBS[first.toLowerCase()];
  if (!verb) return [description, lowerFirst(description)];
  const tail = rest.length ? ` ${rest.join(' ')}` : '';
  return [upperFirst(verb[0]) + tail, verb[1] + tail];
}

/** An address as people say it: without `http://`, cut when long. */
function address(url: string): string {
  const bare = url.replace(/^https?:\/\//, '');
  return bare.length > 60 ? `${bare.slice(0, 59)}…` : bare;
}

/** Most characters of an element's label in a step. */
const MAX_LABEL = 40;

/** An element as a step names it (`browser-steps` design decision 4): “Sign in”, or a button. */
export function named(target: Target): string {
  const label = target.label?.trim();
  if (label) return `“${label.length > MAX_LABEL ? `${label.slice(0, MAX_LABEL - 1).trimEnd()}…` : label}”`;
  return `${/^[aeiou]/.test(target.kind) ? 'an' : 'a'} ${target.kind}`;
}

/** Playwright MCP's tools (`browser-steps-mcp` design decision 1), under whatever name the host gave its server. */
const PLAYWRIGHT = new Set([
  'browser_click', 'browser_close', 'browser_console_messages', 'browser_drag', 'browser_drop', 'browser_emulate_media',
  'browser_evaluate', 'browser_file_upload', 'browser_fill_form', 'browser_find', 'browser_handle_dialog', 'browser_hover',
  'browser_navigate', 'browser_navigate_back', 'browser_network_request', 'browser_network_requests', 'browser_press_key',
  'browser_resize', 'browser_run_code_unsafe', 'browser_select_option', 'browser_snapshot', 'browser_tabs',
  'browser_take_screenshot', 'browser_type', 'browser_wait_for',
]);
/** Claude in Chrome, the server Claude Code itself adds with `--chrome`. */
const CHROME = 'mcp__claude-in-chrome__';

type Browser = 'own' | 'playwright' | 'chrome';

/** The browser a tool belongs to, and the tool's name without its server. */
function browserOf(toolName: string): [Browser, string] | null {
  if (toolName.startsWith(BROWSER)) return ['own', toolName.slice(BROWSER.length)];
  if (toolName.startsWith(CHROME)) return ['chrome', toolName.slice(CHROME.length)];
  const match = /^mcp__.+?__(browser_\w+)$/.exec(toolName);
  return match && PLAYWRIGHT.has(match[1]!) ? ['playwright', match[1]!] : null;
}

/** Text a typing call's input carries, by browser and tool: hidden when the session left it out. */
function typedKey(browser: Browser, tool: string, input: Record<string, unknown>): string | null {
  if (browser === 'own' && tool === 'type') return 'text';
  if (browser === 'playwright' && tool === 'browser_type') return 'text';
  if (browser === 'chrome' && tool === 'computer' && input.action === 'type') return 'text';
  if (browser === 'chrome' && tool === 'form_input') return 'value';
  return null;
}

/** Typing in a browser whose text the session left out: a sensitive field, or one it doesn't know to be ordinary. */
export function typedTextHidden(tool: ToolItem): boolean {
  const browser = browserOf(tool.toolName);
  const key = browser && typedKey(browser[0], browser[1], tool.input);
  return key !== null && !(key in tool.input);
}

/** A form field of Playwright MCP left without its value: typed text, not a choice. */
const fieldHidden = (field: Record<string, unknown>) => !('value' in field) && (field.type ?? 'textbox') === 'textbox';

const objects = (value: unknown): Record<string, unknown>[] =>
  Array.isArray(value) ? value.map((v) => (v && typeof v === 'object' ? (v as Record<string, unknown>) : {})) : [];

/** What the details view says of each element a form or batch acts on: "Field 1: “Card number”, text field, text hidden". */
export function elementHints(tool: ToolItem): string[] {
  const browser = browserOf(tool.toolName);
  if (!browser) return [];
  const targets = tool.targets ?? [];
  const hint = (word: string, n: number, target: Target | null | undefined, hidden: boolean) => {
    const parts = [target ? (target.label ? `${named(target)}, ${target.kind}` : named(target)) : '', hidden ? 'text hidden' : ''].filter(Boolean);
    return parts.length ? `${word} ${n + 1}: ${parts.join(', ')}` : '';
  };
  if (browser[1] === 'browser_fill_form') {
    return objects(tool.input.fields).map((field, n) => hint('Field', n, targets[n], fieldHidden(field))).filter(Boolean);
  }
  if (browser[0] === 'chrome' && browser[1] === 'browser_batch') {
    return objects(tool.input.actions)
      .map((action, n) => {
        const input = objects([action.input])[0]!;
        const key = typedKey('chrome', typeof action.name === 'string' ? action.name : '', input);
        return hint('Action', n, targets[n], key !== null && !(key in input));
      })
      .filter(Boolean);
  }
  return [];
}

/** What one browser call did, past and -ing. */
function browserAction(tool: ToolItem): [string, string] {
  switch (tool.toolName.slice(BROWSER.length)) {
    case 'navigate': {
      const where = address(text(tool, 'url')) || 'a page';
      return [`opened ${where}`, `opening ${where}`];
    }
    case 'read_page':
      return ['read the page', 'reading the page'];
    case 'click': {
      const what = tool.target ? named(tool.target) : 'on the page';
      return [`clicked ${what}`, `clicking ${what}`];
    }
    case 'type': {
      const { target } = tool;
      if (target?.sensitive) return [`filled in ${named(target)}`, `filling in ${named(target)}`];
      if (target) return [`typed into ${named(target)}`, `typing into ${named(target)}`];
      // The session hid the text of a field it didn't know; an older host sent it.
      if (typedTextHidden(tool)) return ['filled in a field', 'filling in a field'];
      return ['typed on the page', 'typing on the page'];
    }
    case 'press': {
      const key = text(tool, 'key') || 'a key';
      return [`pressed ${key}`, `pressing ${key}`];
    }
    case 'screenshot':
      return ['took a screenshot', 'taking a screenshot'];
    case 'back':
      return ['went back a page', 'going back a page'];
    case 'console':
      return ['read the console', 'reading the console'];
    default:
      return [`used ${tool.toolName.slice(BROWSER.length)}`, `using ${tool.toolName.slice(BROWSER.length)}`];
  }
}

/** A phrase both ways, as a past and -ing pair. */
const both = (past: string, ing: string): [string, string] => [past, ing];

/** The element of another browser's call (`browser-steps-mcp` design decision 5): by the page's read, else by Claude's words. */
function element(target: Target | null | undefined, words: unknown): string | null {
  if (target) return named(target);
  const said = typeof words === 'string' ? words.trim() : '';
  return said ? (said.length > MAX_LABEL ? `${said.slice(0, MAX_LABEL - 1).trimEnd()}…` : said) : null;
}

/** Claude in Chrome's own summary of an action, as the whole phrase: "submits the sign-in form". */
function summary(input: Record<string, unknown>): [string, string] | null {
  const said = typeof input.action_summary === 'string' ? lowerFirst(input.action_summary.trim()) : '';
  return said ? [said, said] : null;
}

const opened = (input: Record<string, unknown>) => {
  const where = address(typeof input.url === 'string' ? input.url.trim() : '') || 'a page';
  return both(`opened ${where}`, `opening ${where}`);
};

/** Typing into `what`: filled in when its text was left out, typed into when it is there. */
function typing(what: string | null, hidden: boolean): [string, string] {
  if (hidden) return both(`filled in ${what ?? 'a field'}`, `filling in ${what ?? 'a field'}`);
  return both(`typed into ${what ?? 'the page'}`, `typing into ${what ?? 'the page'}`);
}

/** One Playwright MCP call, past and -ing. */
function playwrightAction(tool: string, input: Record<string, unknown>, target?: Target, targets: (Target | null)[] = []): [string, string] {
  const what = element(target, input.element);
  switch (tool) {
    case 'browser_navigate':
      return opened(input);
    case 'browser_navigate_back':
      return both('went back a page', 'going back a page');
    case 'browser_snapshot':
      return both('read the page', 'reading the page');
    case 'browser_find':
      return both('searched the page', 'searching the page');
    case 'browser_take_screenshot':
      return both('took a screenshot', 'taking a screenshot');
    case 'browser_click':
      return input.doubleClick === true
        ? both(`double-clicked ${what ?? 'on the page'}`, `double-clicking ${what ?? 'on the page'}`)
        : both(`clicked ${what ?? 'on the page'}`, `clicking ${what ?? 'on the page'}`);
    case 'browser_type':
      return typing(what, !('text' in input));
    case 'browser_fill_form': {
      const fields = objects(input.fields);
      const names = fields.map((field, n) => element(targets[n], field.name) ?? 'a field');
      const hidden = fields.some(fieldHidden);
      const which = names.length === 1 ? names[0]! : names.length === 2 ? `${names[0]} and ${names[1]}` : plural(names.length, 'field');
      return hidden || names.length !== 1 ? both(`filled in ${which}`, `filling in ${which}`) : both(`set ${which}`, `setting ${which}`);
    }
    case 'browser_press_key': {
      const key = typeof input.key === 'string' && input.key.trim() ? input.key.trim() : 'a key';
      return both(`pressed ${key}`, `pressing ${key}`);
    }
    case 'browser_hover':
      return both(`hovered over ${what ?? 'the page'}`, `hovering over ${what ?? 'the page'}`);
    case 'browser_select_option':
      return both(`chose an option in ${what ?? 'a list'}`, `choosing an option in ${what ?? 'a list'}`);
    case 'browser_drag': {
      const from = element(targets[0], input.startElement);
      const to = element(targets[1], input.endElement);
      return from && to ? both(`dragged ${from} onto ${to}`, `dragging ${from} onto ${to}`) : both('dragged on the page', 'dragging on the page');
    }
    case 'browser_wait_for':
      return both('waited', 'waiting');
    case 'browser_tabs':
      return tabs(input.action === 'new' ? 'new' : input.action === 'close' ? 'close' : 'list');
    case 'browser_console_messages':
      return both('read the console', 'reading the console');
    case 'browser_evaluate':
    case 'browser_run_code_unsafe':
      return both('ran a script on the page', 'running a script on the page');
    default:
      return both(`used ${tool}`, `using ${tool}`);
  }
}

function tabs(action: 'new' | 'close' | 'list'): [string, string] {
  if (action === 'new') return both('opened a tab', 'opening a tab');
  if (action === 'close') return both('closed a tab', 'closing a tab');
  return both('looked at its tabs', 'looking at its tabs');
}

const CHROME_CLICKS: Record<string, [string, string]> = {
  left_click: ['clicked', 'clicking'],
  right_click: ['right-clicked', 'right-clicking'],
  double_click: ['double-clicked', 'double-clicking'],
  triple_click: ['triple-clicked', 'triple-clicking'],
};

/** One Claude in Chrome action, alone or in a batch, past and -ing. */
function chromeAction(tool: string, input: Record<string, unknown>, target?: Target | null): [string, string] {
  const what = target ? named(target) : null;
  const action = typeof input.action === 'string' ? input.action : '';
  switch (tool) {
    case 'navigate':
      return opened(input);
    case 'read_page':
    case 'get_page_text':
      return both('read the page', 'reading the page');
    case 'find':
      return both('searched the page', 'searching the page');
    case 'form_input':
      if (!what) return summary(input) ?? typing(null, true);
      return 'value' in input ? both(`set ${what}`, `setting ${what}`) : typing(what, true);
    case 'tabs_context_mcp':
      return tabs('list');
    case 'tabs_create_mcp':
      return tabs('new');
    case 'tabs_close_mcp':
      return tabs('close');
    case 'read_console_messages':
      return both('read the console', 'reading the console');
    case 'javascript_tool':
      return both('ran a script on the page', 'running a script on the page');
    case 'computer':
      break;
    default:
      return both(`used ${tool}`, `using ${tool}`);
  }
  const click = CHROME_CLICKS[action];
  if (click) return what ? both(`${click[0]} ${what}`, `${click[1]} ${what}`) : (summary(input) ?? both(`${click[0]} on the page`, `${click[1]} on the page`));
  switch (action) {
    case 'type':
      return typing(what, !('text' in input));
    case 'key': {
      const key = typeof input.text === 'string' && input.text.trim() ? input.text.trim() : 'a key';
      return both(`pressed ${key}`, `pressing ${key}`);
    }
    case 'screenshot':
    case 'zoom':
      return both('took a screenshot', 'taking a screenshot');
    case 'hover':
      return both(`hovered over ${what ?? 'the page'}`, `hovering over ${what ?? 'the page'}`);
    case 'left_click_drag':
      return summary(input) ?? both('dragged on the page', 'dragging on the page');
    case 'scroll':
    case 'scroll_to':
      return both('scrolled the page', 'scrolling the page');
    case 'wait':
      return both('waited', 'waiting');
    default:
      return both(`used ${action || tool}`, `using ${action || tool}`);
  }
}

/** What one browser call did, past and -ing; a Claude in Chrome batch, each of its actions. */
function browserActions(tool: ToolItem): [string, string][] {
  const [browser, name] = browserOf(tool.toolName)!;
  if (browser === 'own') return [browserAction(tool)];
  if (browser === 'playwright') return [playwrightAction(name, tool.input, tool.target, tool.targets)];
  if (name !== 'browser_batch') return [chromeAction(name, tool.input, tool.target)];
  const actions = objects(tool.input.actions).map((action, n) =>
    chromeAction(typeof action.name === 'string' ? action.name : '', objects([action.input])[0]!, tool.targets?.[n]),
  );
  return actions.length ? actions : [both('used its browser', 'using its browser')];
}

/** Which consecutive calls share a line: reads, changes, the files of one plan, and the browser. */
function group(tool: ToolItem): string | null {
  if (browserOf(tool.toolName)) return 'browser';
  if (READS.has(tool.toolName)) return 'reads';
  if (!CHANGES.has(tool.toolName)) return null;
  const path = text(tool, 'file_path') || tool.changes[0]?.path || '';
  const plan = planChangeOf(path);
  return plan ? `plan:${plan}` : 'changes';
}

function line(tools: ToolItem[], key: string | null, seenPlans: Set<string>): Step {
  const first = tools[0]!;
  const screenshot = tools.findLast((tool) => screenshotOf(tool) !== null);
  const base = { kind: 'step' as const, id: first.id, outcome: outcome(tools), tools, ...(screenshot && { screenshot }) };
  if (key === 'reads') {
    const files = new Set(tools.filter((t) => t.toolName === 'Read').map((t) => text(t, 'file_path'))).size;
    const searched = tools.some((t) => t.toolName !== 'Read');
    const parts = [files ? `looked at ${plural(files, 'file')}` : '', searched ? 'searched the code' : ''].filter(Boolean);
    const said = parts.join(' and ');
    return { ...base, label: upperFirst(said), live: said.replace('looked at', 'looking at').replace('searched', 'searching') };
  }
  if (key === 'changes') {
    const files = new Set(tools.map((t) => text(t, 'file_path') || t.changes[0]?.path)).size;
    return { ...base, label: `Changed ${plural(files, 'file')}`, live: `changing ${plural(files, 'file')}` };
  }
  if (key === 'browser') {
    const actions = tools.flatMap(browserActions);
    if (actions.length === 1) {
      const [past, ing] = actions[0]!;
      return { ...base, label: `${upperFirst(past)} in its browser`, live: `${ing} in its browser` };
    }
    return { ...base, label: `Used its browser: ${actions.map(([past]) => past).join(', ')}`, live: `using its browser: ${actions.at(-1)![1]}` };
  }
  if (key?.startsWith('plan:')) {
    const plan = key.slice('plan:'.length);
    const wrote = !seenPlans.has(plan);
    seenPlans.add(plan);
    return { ...base, plan, label: wrote ? 'Wrote the plan' : 'Updated the plan', live: wrote ? 'writing the plan' : 'updating the plan' };
  }
  if (SHELL.has(first.toolName)) {
    const description = text(first, 'description');
    if (!description) return { ...base, label: 'Ran a command', live: 'running a command' };
    const [past, ing] = tenses(description);
    // A command that never ran keeps the description as given, rather than saying it was done.
    const ran = base.outcome === 'succeeded' || base.outcome === 'failed';
    return { ...base, label: base.outcome === 'running' ? upperFirst(ing) : ran ? past : description, live: ing };
  }
  if (SUBAGENTS.has(first.toolName)) {
    const description = text(first, 'description') || 'a task';
    return { ...base, label: `Asked a helper: ${description}`, live: `working with a helper on: ${lowerFirst(description)}` };
  }
  if (first.toolName === 'AskUserQuestion') {
    const question = firstQuestion(first) ?? 'a question';
    const answers = first.outcome?.kind === 'answers' ? first.outcome.answers : [];
    const answer = answers[0] ? [...answers[0].selected, answers[0].typed ?? ''].filter(Boolean).join(', ') : '';
    const more = answers.length > 1 ? ` +${answers.length - 1}` : '';
    return { ...base, label: answer ? `Asked: ${question} → ${answer}${more}` : `Asked: ${question}`, live: `asking: ${question}` };
  }
  if (first.toolName === 'ExitPlanMode') {
    const decided = first.outcome?.kind === 'plan-decision' ? first.outcome : null;
    const label = decided ? (decided.approved ? 'Plan approved' : 'Plan rejected') : 'Proposed a plan';
    return { ...base, label, live: 'proposing a plan' };
  }
  if (first.toolName === 'TodoWrite') return { ...base, label: 'Updated its to-do list', live: 'updating its to-do list' };
  return { ...base, label: `Used ${first.toolName}`, live: `using ${first.toolName}` };
}

/**
 * A turn as summary lines, with the agent's own text and waiting notices kept in place.
 * `seenPlans` holds the OpenSpec changes earlier turns wrote; it is updated, so pass the same set
 * through the turns in order.
 */
export function summarize(turn: Turn, seenPlans: Set<string>): SummaryEntry[] {
  const entries: SummaryEntry[] = [];
  let run: ToolItem[] = [];
  let runKey: string | null = null;
  const flush = () => {
    if (run.length) entries.push(line(run, runKey, seenPlans));
    run = [];
    runKey = null;
  };
  for (const item of turn.items) {
    if (item.kind !== 'tool') {
      flush();
      entries.push(item);
      continue;
    }
    const key = group(item);
    if (key === null || key !== runKey) flush();
    run.push(item);
    runKey = key;
    if (key === null) flush();
  }
  flush();
  return entries;
}

/** The first question an `AskUserQuestion` call asks. */
function firstQuestion(tool: ToolItem): string | null {
  const questions = tool.input.questions;
  return Array.isArray(questions) && typeof questions[0]?.question === 'string' ? (questions[0].question as string) : null;
}

/** What the agent does right now, for viewers without the terminal; null when it is not busy. */
export function liveLine(turn: Turn, seenPlans: Set<string>, agentName: string): string | null {
  const running = summarize(turn, new Set(seenPlans))
    .filter((entry): entry is Step => entry.kind === 'step')
    .findLast((step) => step.outcome === 'running');
  // A question or a plan raises the same notice as a permission prompt; say what it waits for.
  const tool = running?.tools[0];
  if (tool?.toolName === 'AskUserQuestion') return `Waiting for the host's answer: ${firstQuestion(tool) ?? 'a question'}`;
  if (tool?.toolName === 'ExitPlanMode') return 'Waiting for the host to approve the plan';
  const last = turn.items.at(-1);
  if (last?.kind === 'notice') return `Waiting for the host: ${last.text}`;
  return running ? `${agentName} is ${running.live}` : null;
}
