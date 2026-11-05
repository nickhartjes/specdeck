// Turns the flat list of timeline events into what the timeline panel shows: one turn per
// prompt, with the agent's text and tool calls in order, and the files it touched.

import type { DiffHunk, Permission, TimelineEvent, TokenCounts, ToolOutcome } from '../api/types';

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

export interface Turn {
  promptId: string;
  prompt: string;
  at: number;
  outputSeq: number;
  items: (TextItem | ToolItem | NoticeItem)[];
  /** The final response, once the turn is over. */
  response: string | null;
  /** Tokens the agent used for this prompt so far, once usage is known. */
  tokens: number | null;
}


export interface TouchedFile {
  path: string;
  /** Read but never changed. */
  readOnly: boolean;
  changes: FileChange[];
}

export interface TimelineModel {
  turns: Turn[];
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
  const texts = new Map<string, TextItem>();
  const files = new Map<string, TouchedFile>();
  let unavailable: string | null = null;
  // The session total when each turn started; a turn's tokens are what was added since.
  let sessionTotal = 0;
  const startTotals = new Map<Turn, number>();

  const turnFor = (promptId: string, at: number, outputSeq: number): Turn => {
    let turn = byPrompt.get(promptId);
    if (!turn) {
      // Events for a prompt that came before this viewer's history; show them under a blank prompt.
      turn = { promptId, prompt: '', at, outputSeq, items: [], response: null, tokens: null };
      startTotals.set(turn, sessionTotal);
      byPrompt.set(promptId, turn);
      turns.push(turn);
    }
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
        if (existing) {
          existing.text += text;
        } else {
          const item: TextItem = { kind: 'text', id: event.messageId, text, outputSeq: event.outputSeq };
          texts.set(event.messageId, item);
          turnFor(event.promptId, event.at, event.outputSeq).items.push(item);
        }
        break;
      }
      case 'response':
        turnFor(event.promptId, event.at, event.outputSeq).response = withNote(event.text, event.truncated);
        break;
      case 'tool-call': {
        const item: ToolItem = {
          kind: 'tool',
          id: event.toolUseId,
          toolName: event.toolName,
          input: event.input,
          status: 'running',
          changes: [],
          outputSeq: event.outputSeq,
          at: event.at,
          ...(event.truncated && { truncated: true }),
        };
        tools.set(event.toolUseId, item);
        turnFor(event.promptId, event.at, event.outputSeq).items.push(item);
        break;
      }
      case 'tool-result': {
        const item = tools.get(event.toolUseId);
        if (item) {
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
        tools.get(event.toolUseId)?.changes.push(change);
        const file = touched(event.path);
        file.readOnly = false;
        file.changes.push(change);
        break;
      }
      case 'file-read':
        touched(event.path);
        break;
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
        if (turn) turn.tokens = total - (startTotals.get(turn) ?? total);
        break;
      }
      case 'attention': {
        const turn = turns.at(-1);
        turn?.items.push({ kind: 'notice', id: `notice-${event.at}-${turn.items.length}`, text: withNote(event.message, event.truncated), outputSeq: event.outputSeq });
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
  return { turns, files: sorted, unavailable };
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
    parts.push(item.kind === 'tool' ? `${item.toolName} ${toolSummary(item)}` : item.text);
  }
  return parts.join('\n');
}
