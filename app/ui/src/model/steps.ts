// What the agent did for a prompt, in plain words (viewer-views design decision 6): each tool call,
// or each run of reads or file changes, becomes one step line that opens to its technical detail.

import type { NoticeItem, TextItem, ToolItem, ToolStatus, Turn } from './timeline';

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
}

export type SummaryEntry = Step | TextItem | NoticeItem;

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

/** What one browser call did, past and -ing. */
function browserAction(tool: ToolItem): [string, string] {
  switch (tool.toolName.slice(BROWSER.length)) {
    case 'navigate': {
      const where = address(text(tool, 'url')) || 'a page';
      return [`opened ${where}`, `opening ${where}`];
    }
    case 'read_page':
      return ['read the page', 'reading the page'];
    case 'click':
      return ['clicked on the page', 'clicking on the page'];
    case 'type':
      return ['typed on the page', 'typing on the page'];
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

/** Which consecutive calls share a line: reads, changes, the files of one plan, and the browser. */
function group(tool: ToolItem): string | null {
  if (tool.toolName.startsWith(BROWSER)) return 'browser';
  if (READS.has(tool.toolName)) return 'reads';
  if (!CHANGES.has(tool.toolName)) return null;
  const path = text(tool, 'file_path') || tool.changes[0]?.path || '';
  const plan = planChangeOf(path);
  return plan ? `plan:${plan}` : 'changes';
}

function line(tools: ToolItem[], key: string | null, seenPlans: Set<string>): Step {
  const first = tools[0]!;
  const base = { kind: 'step' as const, id: first.id, outcome: outcome(tools), tools };
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
    const actions = tools.map(browserAction);
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
