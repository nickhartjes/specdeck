import { describe, expect, it } from 'vitest';
import type { DiffHunk } from '../api/types';
import { diffLines, unapply, wholeFile, type LineEdit } from './lineDiff';
import type { FileChange } from './timeline';

/** Applies `edits` to `a` with the lines of `b`, to check an edit script. */
function apply(a: string[], b: string[], edits: LineEdit[]): string[] {
  const out: string[] = [];
  let i = 0;
  let j = 0;
  for (const edit of edits) {
    if (edit === 'same') {
      expect(a[i]).toBe(b[j]);
      out.push(b[j]!);
      i++;
      j++;
    } else if (edit === 'add') out.push(b[j++]!);
    else i++;
  }
  expect(i).toBe(a.length);
  return out;
}

function change(toolUseId: string, kind: FileChange['change'], hunks: DiffHunk[], truncated = false): FileChange {
  return { path: 'a.rs', change: kind, hunks, promptId: 'p', toolUseId, at: 0, outputSeq: 0, truncated };
}

describe('diffLines', () => {
  it.each([
    [[], []],
    [['a'], []],
    [[], ['a']],
    [['a', 'b', 'c'], ['a', 'x', 'c']],
    [['a', 'b', 'c', 'd'], ['b', 'c', 'e', 'd', 'f']],
    [['x', 'a', 'b'], ['a', 'b', 'y']],
    [['a', 'a', 'b', 'a'], ['b', 'a', 'a', 'a', 'b']],
  ])('turns %j into %j with as few edits as needed', (a, b) => {
    const edits = diffLines(a, b);
    expect(apply(a, b, edits)).toEqual(b);
  });

  it('keeps the shortest script', () => {
    expect(diffLines(['a', 'b', 'c'], ['a', 'x', 'c'])).toEqual(['same', 'del', 'add', 'same']);
  });

  it('is quick for a large file with a few changes', () => {
    const a = Array.from({ length: 10_000 }, (_, i) => `line ${i}`);
    const b = [...a];
    b[100] = 'changed';
    b.splice(5_000, 0, 'inserted');
    b.splice(9_000, 2);
    const started = performance.now();
    const edits = diffLines(a, b);
    expect(performance.now() - started).toBeLessThan(50);
    expect(apply(a, b, edits)).toEqual(b);
  });
});

describe('unapply', () => {
  it('gives the text before an edit back', () => {
    const hunks: DiffHunk[] = [{ oldStart: 1, oldLines: 3, newStart: 1, newLines: 3, lines: [' a', '-b', '+B', ' c'] }];
    expect(unapply('a\nB\nc\nd', hunks)).toBe('a\nb\nc\nd');
  });

  it('refuses hunks that do not fit the text', () => {
    const hunks: DiffHunk[] = [{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, lines: ['-b', '+X'] }];
    expect(unapply('a\nB', hunks)).toBeNull();
  });
});

describe('wholeFile', () => {
  const v1 = 'fn a() {}\nfn b() {}\nfn c() {}';
  const v2 = 'fn a() {}\nfn B() {}\nfn c() {}';
  const v3 = 'fn a() {}\nfn B() {}\nfn c() {}\nfn d() {}';
  const edit1: DiffHunk[] = [{ oldStart: 1, oldLines: 3, newStart: 1, newLines: 3, lines: [' fn a() {}', '-fn b() {}', '+fn B() {}', ' fn c() {}'] }];
  const edit2: DiffHunk[] = [{ oldStart: 3, oldLines: 1, newStart: 3, newLines: 2, lines: [' fn c() {}', '+fn d() {}'] }];

  it('marks every line of a created file as added', () => {
    const created = change('t1', 'created', [{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 3, lines: v1.split('\n').map((l) => `+${l}`) }]);
    const whole = wholeFile([created], new Map([['t1', v1]]));
    expect(whole).toMatchObject({ marks: ['added', 'added', 'added'], markedFrom: 0 });
  });

  it('marks the lines of every change', () => {
    const whole = wholeFile([change('t1', 'edited', edit1), change('t2', 'edited', edit2)], new Map([['t1', v2], ['t2', v3]]));
    expect(whole).toMatchObject({ lines: v3.split('\n'), marks: [null, 'changed', null, 'added'], markedFrom: 0 });
  });

  it('marks where lines were removed', () => {
    const removal: DiffHunk[] = [{ oldStart: 1, oldLines: 3, newStart: 1, newLines: 2, lines: [' fn a() {}', '-fn b() {}', ' fn c() {}'] }];
    const whole = wholeFile([change('t1', 'edited', removal)], new Map([['t1', 'fn a() {}\nfn c() {}']]));
    expect(whole).toMatchObject({ marks: [null, null], removed: [0, 1, 0] });
  });

  it('says when earlier changes went without their text', () => {
    const whole = wholeFile([change('t1', 'edited', edit1), change('t2', 'edited', edit2)], new Map([['t2', v3]]));
    expect(whole).toMatchObject({ marks: [null, null, null, 'added'], markedFrom: 1 });
  });

  it('falls back to the change after a cut one', () => {
    const whole = wholeFile([change('t1', 'edited', edit1, true), change('t2', 'edited', edit2)], new Map([['t1', v2], ['t2', v3]]));
    expect(whole).toMatchObject({ marks: [null, null, null, 'added'], markedFrom: 1 });
  });

  it('is unavailable without the latest text', () => {
    expect(wholeFile([change('t1', 'edited', edit1)], new Map())).toEqual({ unavailable: 'no-text' });
    expect(wholeFile([change('t1', 'edited', edit1)], new Map([['t1', null]]))).toEqual({ unavailable: 'too-large' });
    expect(wholeFile([change('t1', 'edited', edit1), change('t2', 'edited', edit2)], new Map([['t1', v2]]))).toEqual({ unavailable: 'no-text' });
    expect(wholeFile([change('t1', 'deleted', [])], new Map())).toEqual({ unavailable: 'deleted' });
  });
});
