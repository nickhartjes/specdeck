// Which lines of a code file the agent changed in this session, worked out in the viewer from the
// file's whole text and its diffs (`files-documents-and-code` design decision 6).

import type { DiffHunk } from '../api/types';
import type { FileChange } from './timeline';

export type LineEdit = 'same' | 'add' | 'del';

/** Past this many differences the diff gives up and calls the whole middle changed. */
const MAX_DIFFERENCES = 4_000;

/**
 * The edits that turn `a` into `b`, line by line: Myers' O(ND) diff, after the common start and
 * end are taken off, so a large file with a few changes is cheap.
 */
export function diffLines(a: string[], b: string[]): LineEdit[] {
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let end = 0;
  while (end < a.length - start && end < b.length - start && a[a.length - 1 - end] === b[b.length - 1 - end]) end++;
  const middle = myers(a.slice(start, a.length - end), b.slice(start, b.length - end));
  return [...new Array<LineEdit>(start).fill('same'), ...middle, ...new Array<LineEdit>(end).fill('same')];
}

function myers(a: string[], b: string[]): LineEdit[] {
  const n = a.length;
  const m = b.length;
  if (n === 0) return new Array<LineEdit>(m).fill('add');
  if (m === 0) return new Array<LineEdit>(n).fill('del');
  const max = Math.min(n + m, MAX_DIFFERENCES);
  const offset = max + 1;
  const v = new Int32Array(2 * max + 3);
  // The furthest x on each diagonal after each round, for walking back.
  const trace: Int32Array[] = [];
  for (let d = 0; d <= max; d++) {
    trace.push(v.slice(offset - d - 1, offset + d + 2));
    for (let k = -d; k <= d; k += 2) {
      let x = k === -d || (k !== d && v[offset + k - 1]! < v[offset + k + 1]!) ? v[offset + k + 1]! : v[offset + k - 1]! + 1;
      let y = x - k;
      while (x < n && y < m && a[x] === b[y]) {
        x++;
        y++;
      }
      v[offset + k] = x;
      if (x >= n && y >= m) return backtrack(trace, a.length, b.length);
    }
  }
  // Too different: everything in the middle was replaced.
  return [...new Array<LineEdit>(n).fill('del'), ...new Array<LineEdit>(m).fill('add')];
}

function backtrack(trace: Int32Array[], n: number, m: number): LineEdit[] {
  const edits: LineEdit[] = [];
  let x = n;
  let y = m;
  for (let d = trace.length - 1; d >= 0; d--) {
    // `trace[d]` holds diagonals -d-1 to d+1 as they were before round d.
    const at = (k: number) => trace[d]![k + d + 1]!;
    const k = x - y;
    const down = k === -d || (k !== d && at(k - 1) < at(k + 1));
    const prevK = down ? k + 1 : k - 1;
    const prevX = d === 0 ? 0 : at(prevK);
    const prevY = prevX - prevK;
    while (x > prevX && y > prevY) {
      edits.push('same');
      x--;
      y--;
    }
    if (d > 0) {
      edits.push(down ? 'add' : 'del');
      if (down) y--;
      else x--;
    }
  }
  return edits.reverse();
}

/**
 * The text before `hunks` were applied to `after`, or null when they do not fit it, such as
 * hunks cut to fit in one message.
 */
export function unapply(after: string, hunks: DiffHunk[]): string | null {
  const lines = after.split('\n');
  const before: string[] = [];
  let next = 0;
  for (const hunk of hunks) {
    // A hunk of a created file starts at line 0.
    const start = Math.max(hunk.newStart - 1, 0);
    if (start < next || start > lines.length) return null;
    before.push(...lines.slice(next, start));
    next = start;
    for (const line of hunk.lines) {
      const sign = line[0];
      const text = line.slice(1);
      if (sign === '-') {
        before.push(text);
      } else if (sign === '+' || sign === ' ') {
        if (lines[next] !== text) return null;
        if (sign === ' ') before.push(text);
        next++;
      } else if (sign !== '\\') {
        return null;
      }
    }
  }
  before.push(...lines.slice(next));
  return before.join('\n');
}

export type LineMark = 'added' | 'changed' | null;

export type WholeFile =
  | {
      lines: string[];
      /** Per line of `lines`. */
      marks: LineMark[];
      /** Lines removed just before each line of `lines`; the last entry is for the end. */
      removed: number[];
      /** The first change whose lines are marked: changes before it went without the text. */
      markedFrom: number;
    }
  | { unavailable: 'no-text' | 'too-large' | 'deleted' };

/**
 * A file's latest text with the lines this session's changes added or changed marked.
 * `changes` are the file's changes in order, and `texts` their whole texts by tool call: a string,
 * null when it was too large, absent when none was sent.
 */
export function wholeFile(changes: FileChange[], texts: Map<string, string | null>): WholeFile {
  const last = changes.at(-1);
  if (!last || last.change === 'deleted') return { unavailable: last ? 'deleted' : 'no-text' };
  const latest = texts.get(last.toolUseId);
  if (latest === undefined) return { unavailable: 'no-text' };
  if (latest === null) return { unavailable: 'too-large' };

  // The base: the text before the earliest change since which every text is known.
  let first = changes.length - 1;
  while (first > 0 && typeof texts.get(changes[first - 1]!.toolUseId) === 'string' && changes[first - 1]!.change !== 'deleted') first--;
  const change = changes[first]!;
  const after = texts.get(change.toolUseId) as string;
  let base: string;
  let markedFrom = first;
  if (change.change === 'created') {
    base = '';
  } else {
    const before = change.truncated ? null : unapply(after, change.hunks);
    if (before === null) {
      // Its own lines can't be told apart; mark the changes after it.
      base = after;
      markedFrom = first + 1;
    } else {
      base = before;
    }
  }

  const lines = latest.split('\n');
  const edits = base === '' ? new Array<LineEdit>(lines.length).fill('add') : diffLines(base.split('\n'), lines);
  const marks: LineMark[] = new Array<LineMark>(lines.length).fill(null);
  const removed: number[] = new Array<number>(lines.length + 1).fill(0);
  let line = 0;
  let i = 0;
  while (i < edits.length) {
    if (edits[i] === 'same') {
      line++;
      i++;
      continue;
    }
    // A run of removals and additions: as many additions as removals are changes.
    let dels = 0;
    let adds = 0;
    while (i < edits.length && edits[i] !== 'same') {
      if (edits[i] === 'del') dels++;
      else adds++;
      i++;
    }
    for (let j = 0; j < adds; j++) marks[line + j] = j < dels ? 'changed' : 'added';
    line += adds;
    if (dels > adds) removed[line] = removed[line]! + dels - adds;
  }
  return { lines, marks, removed, markedFrom };
}
