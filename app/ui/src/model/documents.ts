// Documents the agent wrote, every version of each, and the OpenSpec plans it worked on
// (viewer-views design decisions 1 and 7).

import type { PlanDocument, TimelineEvent } from '../api/types';

export interface DocumentVersion {
  /** Position among this document's versions, from 0. */
  version: number;
  /** null when the document was too large to send. */
  text: string | null;
  promptId: string;
  toolUseId: string;
}

export interface Plan {
  change: string;
  documents: PlanDocument[];
  tasks: { done: number; total: number };
}

export interface Documents {
  /** Every version of each document, by path, oldest first. */
  documents: Map<string, DocumentVersion[]>;
  /**
   * The whole text of code files after each change the host shared it for, by path and then by
   * tool call; null when it was too large to send.
   */
  codeTexts: Map<string, Map<string, string | null>>;
  /** The latest state of each plan, the most recently changed first. */
  plans: Plan[];
}

export function buildDocuments(events: TimelineEvent[]): Documents {
  const documents = new Map<string, DocumentVersion[]>();
  const plans = new Map<string, Plan>();
  const codeTexts = new Map<string, Map<string, string | null>>();
  for (const event of events) {
    if (event.kind === 'file-text') {
      const texts = codeTexts.get(event.path) ?? new Map<string, string | null>();
      texts.set(event.toolUseId, event.text);
      codeTexts.set(event.path, texts);
    } else if (event.kind === 'document') {
      const versions = documents.get(event.path) ?? [];
      versions.push({ version: versions.length, text: event.text, promptId: event.promptId, toolUseId: event.toolUseId });
      documents.set(event.path, versions);
    } else if (event.kind === 'plan') {
      // Re-inserted, so the most recently changed plan ends up last.
      plans.delete(event.change);
      plans.set(event.change, { change: event.change, documents: event.documents, tasks: event.tasks });
    }
  }
  return { documents, codeTexts, plans: [...plans.values()].reverse() };
}

const BLOCK_START = /^(#{1,6}\s|[-*+]\s|\d+[.)]\s|>|```|~~~|\|)/;

/** A top-level Markdown block, and the lines it takes in the text, counted from 0. */
export interface Block {
  text: string;
  first: number;
  last: number;
}

/** Top-level Markdown blocks: headings, paragraphs, list items, quotes, tables and code blocks. */
export function splitBlocks(text: string): string[] {
  return blocksWithLines(text).map((block) => block.text);
}

/** As {@link splitBlocks}, with the lines of each block, for marking them in the source. */
export function blocksWithLines(text: string): Block[] {
  const blocks: Block[] = [];
  let current: string[] = [];
  let first = 0;
  let fence: string | null = null;
  let index = -1;
  const flush = () => {
    if (current.length) blocks.push({ text: current.join('\n'), first, last: first + current.length - 1 });
    current = [];
  };
  const add = (line: string) => {
    if (!current.length) first = index;
    current.push(line);
  };
  for (const line of text.replace(/\r\n/g, '\n').split('\n')) {
    index++;
    if (fence) {
      add(line);
      if (line.trimStart().startsWith(fence)) {
        fence = null;
        flush();
      }
      continue;
    }
    const trimmed = line.trimStart();
    if (trimmed === '') {
      flush();
      continue;
    }
    const indented = line.length > trimmed.length;
    if (trimmed.startsWith('```') || trimmed.startsWith('~~~')) {
      flush();
      fence = trimmed.slice(0, 3);
      add(line);
      continue;
    }
    // A new heading or list item starts a block; indented lines continue the one before.
    if (!indented && BLOCK_START.test(trimmed) && !(trimmed.startsWith('|') && current[0]?.trimStart().startsWith('|'))) flush();
    add(line);
    if (/^#{1,6}\s/.test(trimmed)) flush();
  }
  flush();
  return blocks;
}

export type Mark = 'new' | 'changed' | null;

/** The blocks of `now`, each marked when it is new or changed since `read` (null: never read). */
export function compareBlocks(read: string | null, now: string): { block: string; mark: Mark }[] {
  const after = splitBlocks(now);
  if (read === null) return after.map((block) => ({ block, mark: null }));
  const before = splitBlocks(read);
  // Longest common subsequence of identical blocks.
  const table = Array.from({ length: before.length + 1 }, () => new Array<number>(after.length + 1).fill(0));
  for (let i = before.length - 1; i >= 0; i--) {
    for (let j = after.length - 1; j >= 0; j--) {
      table[i]![j] = before[i] === after[j] ? table[i + 1]![j + 1]! + 1 : Math.max(table[i + 1]![j]!, table[i]![j + 1]!);
    }
  }
  const marks: Mark[] = new Array(after.length).fill(null);
  let i = 0;
  let j = 0;
  let removed = 0;
  while (j < after.length) {
    if (i < before.length && before[i] === after[j]) {
      i++;
      j++;
      removed = 0;
    } else if (i < before.length && table[i + 1]![j]! >= table[i]![j + 1]!) {
      i++;
      removed++;
    } else {
      // A block in place of a removed one is a change; otherwise it is new.
      marks[j] = removed > 0 ? 'changed' : 'new';
      if (removed > 0) removed--;
      j++;
    }
  }
  return after.map((block, index) => ({ block, mark: marks[index]! }));
}

/**
 * Per line of `now`, the mark of the block it belongs to, so the source marks exactly what the
 * formatted document marks.
 */
export function sourceMarks(read: string | null, now: string): Mark[] {
  const marks: Mark[] = new Array<Mark>(now.replace(/\r\n/g, '\n').split('\n').length).fill(null);
  const compared = compareBlocks(read, now);
  blocksWithLines(now).forEach((block, i) => {
    const mark = compared[i]?.mark ?? null;
    if (mark) marks.fill(mark, block.first, block.last + 1);
  });
  return marks;
}
