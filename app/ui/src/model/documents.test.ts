import { describe, expect, it } from 'vitest';
import type { TimelineEvent } from '../api/types';
import { blocksWithLines, buildDocuments, compareBlocks, sourceMarks, splitBlocks } from './documents';

describe('document blocks', () => {
  it('splits Markdown into headings, paragraphs, list items and whole code blocks', () => {
    const text = '# Title\n\nFirst paragraph\nstill first.\n\n- one\n- two\n  continued\n\n```\na\n\nb\n```\n## Next\nText';
    expect(splitBlocks(text)).toEqual(['# Title', 'First paragraph\nstill first.', '- one', '- two\n  continued', '```\na\n\nb\n```', '## Next', 'Text']);
  });

  it('marks blocks that are new or changed since the version read', () => {
    const read = '# Design\n\n## Shell\n\nzsh, bash, sh.\n\n## Texts\n\nA confirmation step.';
    const now = '# Design\n\n## Shell\n\nzsh, bash, sh.\n\n## Texts\n\nNo extra step.\n\n## Risks\n\nNone.';
    expect(compareBlocks(read, now).map(({ mark }) => mark)).toEqual([null, null, null, null, 'changed', 'new', 'new']);
  });

  it('knows the lines of each block', () => {
    const text = '# Title\n\nFirst\nstill first.\n\n```\na\n\nb\n```\n- item';
    expect(blocksWithLines(text).map(({ first, last }) => [first, last])).toEqual([
      [0, 0],
      [2, 3],
      [5, 9],
      [10, 10],
    ]);
  });

  it('marks the source lines of the blocks the formatted document marks', () => {
    const read = '# Design\n\nA confirmation step.';
    const now = '# Design\n\nNo extra step,\nnone at all.\n\n## Risks';
    expect(sourceMarks(read, now)).toEqual([null, null, 'changed', 'changed', null, 'new']);
    expect(sourceMarks(null, now).every((mark) => mark === null)).toBe(true);
  });

  it('marks nothing for a first read', () => {
    expect(compareBlocks(null, '# A\n\nB').every(({ mark }) => mark === null)).toBe(true);
  });
});

describe('documents and plans from the timeline', () => {
  const at = (n: number) => ({ outputSeq: n, at: n });
  const events: TimelineEvent[] = [
    { ...at(1), kind: 'document', promptId: 'p1', toolUseId: 't1', path: 'openspec/changes/a/proposal.md', text: '# A' },
    {
      ...at(2),
      kind: 'plan',
      change: 'a',
      documents: [{ id: 'proposal', name: 'Proposal', paths: ['openspec/changes/a/proposal.md'], exists: true }],
      tasks: { done: 0, total: 3 },
    },
    { ...at(3), kind: 'document', promptId: 'p2', toolUseId: 't2', path: 'openspec/changes/a/proposal.md', text: '# A, revised' },
    { ...at(4), kind: 'plan', change: 'b', documents: [], tasks: { done: 0, total: 0 } },
    { ...at(5), kind: 'plan', change: 'a', documents: [], tasks: { done: 1, total: 3 } },
    { ...at(6), kind: 'document', promptId: 'p2', toolUseId: 't3', path: 'big.md', text: null },
  ];

  it('keeps every version of a document, and the latest plan per change, most recent first', () => {
    const { documents, plans } = buildDocuments(events);
    expect(documents.get('openspec/changes/a/proposal.md')!.map((v) => v.text)).toEqual(['# A', '# A, revised']);
    expect(documents.get('big.md')![0]!.text).toBeNull();
    expect(plans.map((plan) => [plan.change, plan.tasks.done])).toEqual([
      ['a', 1],
      ['b', 0],
    ]);
  });

  it('keeps the whole text of code files by tool call, apart from documents', () => {
    const { documents, codeTexts } = buildDocuments([
      ...events,
      { ...at(7), kind: 'file-text', promptId: 'p3', toolUseId: 't4', path: 'src/a.rs', text: 'fn a() {}' },
      { ...at(8), kind: 'file-text', promptId: 'p3', toolUseId: 't5', path: 'src/a.rs', text: null },
    ]);
    expect([...codeTexts.get('src/a.rs')!]).toEqual([
      ['t4', 'fn a() {}'],
      ['t5', null],
    ]);
    expect(documents.has('src/a.rs')).toBe(false);
    expect(documents.size).toBe(2);
  });
});
