// Turns the flat list of timeline events into what the timeline panel shows: one turn per
// prompt, with the agent's text and tool calls in order, and the files it touched.

import type { DiffHunk, Permission, Target, TimelineEvent, TokenCounts, ToolOutcome } from '../api/types';

export type ToolStatus = 'running' | 'succeeded' | 'failed' | 'not-run';

export interface FileChange {
  path: string;
  change: 'created' | 'edited' | 'deleted';
  hunks: DiffHunk[];
  promptId: string;
  toolUseId: string;
  at: number;
  outputSeq: number;
  /** Cut by the host to fit in one message: only the start of the diff is here. */
  truncated?: boolean;
}

/** Said wherever the host cut an entry to fit in one message. */
export const SHOWN_IN_PART = 'Shown in part: too large to send in full.';

/** `text`, with the note when it was cut. */
function withNote(text: string, truncated: boolean | undefined): string {
  return truncated ? `${text}\n\n${SHOWN_IN_PART}` : text;
}

export interface TextItem {
  kind: 'text';
  id: string;
  text: string;
  outputSeq: number;
}

export interface ToolItem {
  kind: 'tool';
  id: string;
  toolName: string;
  input: Record<string, unknown>;
  /** The element a click or typing in Claude's browser acts on. */
  target?: Target;
  /** The elements of a call that acts on several, by position: a form's fields, a batch's actions. */
  targets?: (Target | null)[];
  status: ToolStatus;
  error?: string;
  /** What the host decided or a helper reported, once known. */
  outcome?: ToolOutcome;
  /** Whether the host allowed the call, when they were asked. */
  permission?: Permission;
  changes: FileChange[];
  outputSeq: number;
  /** When the call started, in milliseconds since the epoch. */
  at: number;
  /** The host cut the call's input to fit in one message. */
  truncated?: boolean;
}

/** The agent waited for the host here, for example to allow a tool. */
export interface NoticeItem {
  kind: 'notice';
  id: string;
  text: string;
  outputSeq: number;
}

/** A link shared with everyone, at the moment it was shared (`shared-links`). */
export interface LinkItem {
  kind: 'link';
  id: string;
  /** The host's number for it. */
  linkId: number;
  /** Null once the host removed it. */
  url: string | null;
  title?: string;
  /** Who shared it, by name. */
  by: string;
  sessionOnly: boolean;
  at: number;
  outputSeq: number;
}

/** A read request that closed or an estimate the host kept (`read-and-estimate`), at that moment. */
export interface AskItem {
  kind: 'ask';
  id: string;
  entry: Extract<TimelineEvent, { kind: 'read-done' | 'estimated' }>;
  at: number;
  outputSeq: number;
}

export interface Turn {
  promptId: string;
  prompt: string;
  at: number;
  outputSeq: number;
  items: (TextItem | ToolItem | NoticeItem | LinkItem | AskItem)[];
  /** The final response, once the turn is over. */
  response: string | null;
  /** When the final response came, in milliseconds since the epoch. */
  respondedAt: number | null;
  /** Tokens the agent used for this prompt so far, once usage is known. */
  tokens: number | null;
  /** Counts the events that changed this turn, so the panel renders only turns that changed. */
  revision: number;
}


export interface TouchedFile {
  path: string;
  /** Read but never changed. */
  readOnly: boolean;
  changes: FileChange[];
  /** Where in the terminal the agent first read it, for a file it only read. */
  readSeq?: number;
}

export interface TimelineModel {
  turns: Turn[];
  /** Every shared link, in order; also in the turns they were shared in. */
  links: LinkItem[];
  /** Links shared before the first prompt, which belong to no turn. */
  earlyLinks: LinkItem[];
  /** Every closed read request and kept estimate, in order; also in the turns they happened in. */
  asks: AskItem[];
  /** Those before the first prompt. */
  earlyAsks: AskItem[];
  files: TouchedFile[];
  /** Why the timeline is unavailable right now, or null. */
  unavailable: string | null;
}

export function totalTokens(tokens: TokenCounts): number {
  return tokens.input + tokens.output + tokens.cacheRead + tokens.cacheWrite;
}

export function buildTimeline(events: TimelineEvent[]): TimelineModel {
  const turns: Turn[] = [];
  const byPrompt = new Map<string, Turn>();
  const tools = new Map<string, ToolItem>();
  const toolTurns = new Map<string, Turn>();
  const texts = new Map<string, TextItem>();
  const files = new Map<string, TouchedFile>();
  const links: LinkItem[] = [];
  const earlyLinks: LinkItem[] = [];
  const linkTurns = new Map<number, Turn>();
  const asks: AskItem[] = [];
  const earlyAsks: AskItem[] = [];
  let unavailable: string | null = null;
  // The session total when each turn started; a turn's tokens are what was added since.
  let sessionTotal = 0;
  const startTotals = new Map<Turn, number>();

  const turnFor = (promptId: string, at: number, outputSeq: number): Turn => {
    let turn = byPrompt.get(promptId);
    if (!turn) {
      // Events for a prompt that came before this viewer's history; show them under a blank prompt.
      turn = { promptId, prompt: '', at, outputSeq, items: [], response: null, respondedAt: null, tokens: null, revision: 0 };
      startTotals.set(turn, sessionTotal);
      byPrompt.set(promptId, turn);
      turns.push(turn);
    }
    turn.revision++;
    return turn;
  };
  const touched = (path: string): TouchedFile => {
    let file = files.get(path);
    if (!file) {
      file = { path, readOnly: true, changes: [] };
      files.set(path, file);
    }
    return file;
  };

  for (const event of events) {
    switch (event.kind) {
      case 'prompt': {
        const turn = turnFor(event.promptId, event.at, event.outputSeq);
        turn.prompt = withNote(event.text, event.truncated);
        turn.at = event.at;
        turn.outputSeq = event.outputSeq;
        break;
      }
      case 'message': {
        // Live text arrives in pieces per message.
        const existing = texts.get(event.messageId);
        const text = withNote(event.text, event.truncated);
        const turn = turnFor(event.promptId, event.at, event.outputSeq);
        if (existing) {
          existing.text += text;
        } else {
          const item: TextItem = { kind: 'text', id: event.messageId, text, outputSeq: event.outputSeq };
          texts.set(event.messageId, item);
          turn.items.push(item);
        }
        break;
      }
      case 'response': {
        const turn = turnFor(event.promptId, event.at, event.outputSeq);
        turn.response = withNote(event.text, event.truncated);
        turn.respondedAt = event.at;
        break;
      }
      case 'tool-call': {
        const item: ToolItem = {
          kind: 'tool',
          id: event.toolUseId,
          toolName: event.toolName,
          input: event.input,
          ...(event.target && { target: event.target }),
          ...(event.targets && { targets: event.targets }),
          status: 'running',
          changes: [],
          outputSeq: event.outputSeq,
          at: event.at,
          ...(event.truncated && { truncated: true }),
        };
        const turn = turnFor(event.promptId, event.at, event.outputSeq);
        tools.set(event.toolUseId, item);
        toolTurns.set(event.toolUseId, turn);
        turn.items.push(item);
        break;
      }
      case 'tool-result': {
        const item = tools.get(event.toolUseId);
        if (item) {
          toolTurns.get(event.toolUseId)!.revision++;
          item.status = event.status;
          if (event.error) item.error = event.error;
          // A call can get a second result that adds an outcome, such as a helper's report.
          if (event.outcome) item.outcome = event.outcome;
          if (event.permission) item.permission = event.permission;
        }
        break;
      }
      case 'file-change': {
        const change: FileChange = {
          path: event.path,
          change: event.change,
          hunks: event.hunks,
          promptId: event.promptId,
          toolUseId: event.toolUseId,
          at: event.at,
          outputSeq: event.outputSeq,
          ...(event.truncated && { truncated: true }),
        };
        const item = tools.get(event.toolUseId);
        if (item) {
          item.changes.push(change);
          toolTurns.get(event.toolUseId)!.revision++;
        }
        const file = touched(event.path);
        file.readOnly = false;
        file.changes.push(change);
        break;
      }
      case 'file-read': {
        const file = touched(event.path);
        file.readSeq ??= tools.get(event.toolUseId)?.outputSeq ?? event.outputSeq;
        break;
      }
      case 'timeline-unavailable':
        unavailable = event.reason;
        break;
      case 'timeline-available':
        unavailable = null;
        break;
      case 'usage': {
        const total = totalTokens(event.tokens);
        sessionTotal = total;
        const turn = turns.at(-1);
        if (turn) {
          turn.tokens = total - (startTotals.get(turn) ?? total);
          turn.revision++;
        }
        break;
      }
      case 'attention': {
        const turn = turns.at(-1);
        if (turn) turn.revision++;
        turn?.items.push({ kind: 'notice', id: `notice-${event.at}-${turn.items.length}`, text: withNote(event.message, event.truncated), outputSeq: event.outputSeq });
        break;
      }
      case 'link': {
        const link: LinkItem = {
          kind: 'link',
          id: `link-${event.id}`,
          linkId: event.id,
          url: event.url,
          title: event.title,
          by: event.by,
          sessionOnly: event.sessionOnly ?? false,
          at: event.at,
          outputSeq: event.outputSeq,
        };
        links.push(link);
        // Under the prompt it was shared during; before any prompt, on its own.
        const turn = turns.at(-1);
        if (turn) {
          turn.items.push(link);
          turn.revision++;
          linkTurns.set(event.id, turn);
        } else {
          earlyLinks.push(link);
        }
        break;
      }
      case 'link-removed': {
        const link = links.find((l) => l.linkId === event.id);
        if (link) link.url = null;
        const turn = linkTurns.get(event.id);
        if (turn) turn.revision++;
        break;
      }
      case 'read-done':
      case 'estimated': {
        const ask: AskItem = { kind: 'ask', id: `ask-${event.kind}-${event.id}`, entry: event, at: event.at, outputSeq: event.outputSeq };
        asks.push(ask);
        // Under the prompt it happened during, as links are.
        const turn = turns.at(-1);
        if (turn) {
          turn.items.push(ask);
          turn.revision++;
        } else {
          earlyAsks.push(ask);
        }
        break;
      }
      case 'usage-unavailable':
      case 'status-line':
      case 'git':
        break;
      case 'session-started':
      case 'session-ended':
        break;
    }
  }

  // Changed files first, then files that were only read; each group by name.
  const sorted = [...files.values()].sort(
    (a, b) => Number(a.readOnly) - Number(b.readOnly) || a.path.localeCompare(b.path),
  );
  return { turns, links, earlyLinks, asks, earlyAsks, files: sorted, unavailable };
}

/** A short line describing what a tool call does, for its timeline entry. */
export function toolSummary(item: ToolItem): string {
  const input = item.input;
  const text = (key: string) => (typeof input[key] === 'string' ? (input[key] as string) : null);
  switch (item.toolName) {
    case 'Bash':
      return text('command') ?? 'Run a command';
    case 'Read':
    case 'Write':
    case 'Edit':
    case 'MultiEdit':
    case 'NotebookEdit':
      return text('file_path') ?? item.toolName;
    case 'Grep':
    case 'Glob':
      return text('pattern') ?? item.toolName;
    case 'WebFetch':
      return text('url') ?? 'Fetch a page';
    case 'WebSearch':
      return text('query') ?? 'Search the web';
    case 'Task':
      return text('description') ?? 'Start a subagent';
    default: {
      const first = Object.values(input).find((value) => typeof value === 'string');
      return typeof first === 'string' ? first : item.toolName;
    }
  }
}

/** The text a search looks through for a turn. */
export function turnText(turn: Turn): string {
  const parts = [turn.prompt, turn.response ?? ''];
  for (const item of turn.items) {
    if (item.kind === 'tool') parts.push(`${item.toolName} ${toolSummary(item)}`);
    // A link's key after # is not searched, as it is not shown.
    else if (item.kind === 'link') parts.push(`${item.title ?? ''} ${item.url && !item.sessionOnly ? item.url : ''}`);
    else if (item.kind === 'ask') parts.push(item.entry.kind === 'estimated' ? item.entry.task : item.entry.label);
    else parts.push(item.text);
  }
  return parts.join('\n');
}
