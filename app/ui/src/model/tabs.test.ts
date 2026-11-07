import { describe, expect, it } from 'vitest';
import { closeTab, keepTab, noTabs, openTab, selectTab, TERMINAL, withTerminal } from './tabs';

const ids = (tabs: { list: { id: string }[] }) => tabs.list.map((tab) => tab.id);

describe('tabs', () => {
  it('replaces the preview tab with the next file opened', () => {
    let tabs = noTabs(true);
    for (const path of ['proposal.md', 'design.md', 'tasks.md']) tabs = openTab(tabs, 'document', path);
    expect(ids(tabs)).toEqual([TERMINAL, 'document:tasks.md']);
    expect(tabs.active).toBe('document:tasks.md');
    expect(tabs.list[1]!.preview).toBe(true);
  });

  it('keeps a tab that was kept, and opens the next file in a new preview', () => {
    let tabs = openTab(noTabs(true), 'document', 'design.md');
    tabs = keepTab(tabs, 'document:design.md');
    tabs = openTab(tabs, 'document', 'tasks.md');
    expect(ids(tabs)).toEqual([TERMINAL, 'document:design.md', 'document:tasks.md']);
    expect(tabs.list.map((tab) => tab.preview)).toEqual([false, false, true]);
  });

  it('opens a file as an ordinary tab when asked to keep it', () => {
    const tabs = openTab(openTab(noTabs(true), 'diff', 'a.rs', { keep: true }), 'diff', 'b.rs');
    expect(ids(tabs)).toEqual([TERMINAL, 'diff:a.rs', 'diff:b.rs']);
  });

  it('shows an open file again instead of opening it twice, and keeps it on a double click', () => {
    let tabs = openTab(noTabs(true), 'document', 'design.md');
    tabs = selectTab(tabs, TERMINAL);
    tabs = openTab(tabs, 'document', 'design.md', { keep: true });
    expect(ids(tabs)).toEqual([TERMINAL, 'document:design.md']);
    expect(tabs.list[1]!.preview).toBe(false);
    expect(tabs.active).toBe('document:design.md');
  });

  it('scrolls to a change again when the same step is chosen twice', () => {
    let tabs = openTab(noTabs(true), 'diff', 'a.rs', { focus: 't1' });
    tabs = openTab(tabs, 'diff', 'a.rs', { focus: 't1' });
    expect(tabs.list[1]!.focus).toEqual({ toolUseId: 't1', n: 2 });
  });

  it('never closes the terminal tab, and shows the neighbour of a closed tab', () => {
    let tabs = openTab(openTab(noTabs(true), 'diff', 'a.rs', { keep: true }), 'diff', 'b.rs', { keep: true });
    expect(closeTab(tabs, TERMINAL)).toBe(tabs);
    tabs = selectTab(tabs, 'diff:a.rs');
    tabs = closeTab(tabs, 'diff:a.rs');
    expect(ids(tabs)).toEqual([TERMINAL, 'diff:b.rs']);
    expect(tabs.active).toBe('diff:b.rs');
  });

  it('adds the terminal tab first, and takes it away', () => {
    let tabs = openTab(noTabs(false), 'document', 'design.md');
    expect(ids(tabs)).toEqual(['document:design.md']);
    tabs = withTerminal(tabs, true);
    expect(ids(tabs)).toEqual([TERMINAL, 'document:design.md']);
    tabs = selectTab(tabs, TERMINAL);
    tabs = withTerminal(tabs, false);
    expect(tabs.active).toBe('document:design.md');
  });
});

describe('screenshot tabs', () => {
  it('opens a screenshot by its tool call, named after its page, as a preview', () => {
    const tabs = openTab(noTabs(false), 'image', 'toolu_1', { title: 'localhost:5173/login' });
    expect(tabs.list).toEqual([{ id: 'image:toolu_1', kind: 'image', path: 'toolu_1', title: 'localhost:5173/login', preview: true, focus: undefined }]);
    expect(tabs.active).toBe('image:toolu_1');
  });

  it('replaces a preview document like any file, and keeps the screenshot when asked', () => {
    let tabs = openTab(noTabs(true), 'document', 'design.md');
    tabs = openTab(tabs, 'image', 'toolu_1', { title: 'Screenshot' });
    expect(ids(tabs)).toEqual([TERMINAL, 'image:toolu_1']);
    tabs = openTab(tabs, 'image', 'toolu_1', { keep: true });
    expect(tabs.list[1]!.preview).toBe(false);
    expect(tabs.list[1]!.title).toBe('Screenshot');
  });
});
