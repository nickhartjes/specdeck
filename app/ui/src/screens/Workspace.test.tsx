import { createRef, useState } from 'react';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { SharedFileInfo, TimelineEvent } from '../api/types';
import type { SharedFiles } from '../components/SharedFiles';
import { demoTimeline } from '../api/demo';
import { dispatchMenu } from '../menu/menu';
import { adjust, choose, type Layout } from '../model/layout';
import { terminal, TerminalViewStub } from '../test/terminal-stub';
import { Workspace, type WorkspaceHandle } from './Workspace';

vi.mock('../terminal/TerminalView', () => ({ TerminalView: TerminalViewStub }));

type Props = Partial<Parameters<typeof Workspace>[0]> & { ref?: React.Ref<WorkspaceHandle> };

/** The workspace with its layout kept here, as the screens keep it. */
function Harness({ layout: initial, onLayout, ...props }: Props) {
  const [layout, setLayout] = useState<Layout>(initial ?? choose('developer'));
  return (
    <Workspace
      mode="viewer"
      agentName="Claude Code"
      timelineSupported
      timeline={demoTimeline}
      {...props}
      layout={layout}
      onLayout={(next) => {
        setLayout(next);
        onLayout?.(next);
      }}
    />
  );
}

const renderWorkspace = (props: Props = {}) => render(<Harness {...props} />);

const at = (n: number) => ({ outputSeq: n, at: n });

/** A plan with a proposal and a design, the design changed by a second prompt. */
function planTimeline(): TimelineEvent[] {
  const dir = 'openspec/changes/demo';
  const write = (n: number, prompt: string, name: string, file: string, text: string, created: boolean): TimelineEvent[] => [
    { ...at(n), kind: 'tool-call', promptId: prompt, toolUseId: name, toolName: created ? 'Write' : 'Edit', input: { file_path: `${dir}/${file}` } },
    { ...at(n), kind: 'tool-result', promptId: prompt, toolUseId: name, status: 'succeeded' },
    { ...at(n), kind: 'file-change', promptId: prompt, toolUseId: name, path: `${dir}/${file}`, change: created ? 'created' : 'edited', hunks: [] },
    { ...at(n), kind: 'document', promptId: prompt, toolUseId: name, path: `${dir}/${file}`, text },
  ];
  return [
    { ...at(1), kind: 'prompt', promptId: 'p1', text: '/opsx:propose demo' },
    ...write(2, 'p1', 'w1', 'proposal.md', '# Demo plan', true),
    ...write(3, 'p1', 'w2', 'design.md', '# Design\n\nFirst.', true),
    { ...at(4), kind: 'prompt', promptId: 'p2', text: 'change the design' },
    ...write(5, 'p2', 'e1', 'design.md', '# Design\n\nSecond.', false),
    {
      ...at(9),
      kind: 'plan',
      change: 'demo',
      documents: [
        { id: 'proposal', name: 'Proposal', paths: [`${dir}/proposal.md`], exists: true },
        { id: 'design', name: 'Design', paths: [`${dir}/design.md`], exists: true },
      ],
      tasks: { done: 0, total: 0 },
    },
  ];
}

/** A side panel, also while it is hidden. */
const pane = (name: string) => document.querySelector<HTMLElement>(`.pane[aria-label="${name}"]`)!;

const tabNames = () => within(screen.getByRole('tablist', { name: 'Open' })).getAllByRole('tab').map((tab) => tab.textContent);

beforeEach(() => vi.clearAllMocks());
afterEach(() => vi.unstubAllGlobals());

describe('three panels', () => {
  it('shows files, the tabs and the timeline in Developer, the timeline last', () => {
    renderWorkspace();
    const panes = screen.getByRole('main').querySelector('.panes')!;
    const shown = [...panes.children].filter((el) => !el.hasAttribute('hidden') && !el.classList.contains('divider'));
    expect(shown.map((el) => el.getAttribute('aria-label'))).toEqual(['Files', 'Open', 'Timeline']);
    expect(tabNames()).toEqual(['Terminal, read-only']);
  });

  it('keeps the timeline on the right in every preset', () => {
    for (const preset of ['overview', 'developer'] as const) {
      const { unmount } = renderWorkspace({ layout: choose(preset) });
      const panes = screen.getByRole('main').querySelector('.panes')!;
      const shown = [...panes.children].filter((el) => !el.hasAttribute('hidden') && !el.classList.contains('divider'));
      expect(shown.at(-1)).toHaveAttribute('aria-label', 'Timeline');
      unmount();
    }
  });

  it('shows only the middle in Present', () => {
    renderWorkspace({ layout: choose('present') });
    expect(pane('Files')).not.toBeVisible();
    expect(pane('Timeline')).not.toBeVisible();
  });

  it('has an empty row for the host’s requests', () => {
    renderWorkspace();
    expect(screen.getByRole('main').querySelector('.requests')).toBeEmptyDOMElement();
  });

  it('closes a panel with its close button and from the menu, and opens it again', () => {
    const onLayout = vi.fn();
    renderWorkspace({ onLayout });
    fireEvent.click(screen.getByRole('button', { name: 'Hide files' }));
    expect(onLayout).toHaveBeenLastCalledWith(expect.objectContaining({ files: false }));
    expect(pane('Files')).not.toBeVisible();
    act(() => dispatchMenu({ id: 'panel:files' }));
    expect(pane('Files')).toBeVisible();
    act(() => dispatchMenu({ id: 'panel:timeline' }));
    expect(onLayout).toHaveBeenLastCalledWith(expect.objectContaining({ timeline: false }));
  });
});

describe('resizing', () => {
  it('resizes a panel with the arrow keys, and puts it back with a double click', () => {
    const onLayout = vi.fn();
    renderWorkspace({ onLayout });
    const divider = screen.getByRole('separator', { name: 'Resize the files panel' });
    expect(divider).toHaveAttribute('aria-valuenow', '230');
    fireEvent.keyDown(divider, { key: 'ArrowRight' });
    expect(divider).toHaveAttribute('aria-valuenow', '246');
    const timeline = screen.getByRole('separator', { name: 'Resize the timeline panel' });
    fireEvent.keyDown(timeline, { key: 'ArrowLeft' });
    expect(timeline).toHaveAttribute('aria-valuenow', '356');
    fireEvent.doubleClick(divider);
    expect(divider).toHaveAttribute('aria-valuenow', '230');
    expect(onLayout).toHaveBeenLastCalledWith(expect.objectContaining({ widths: expect.objectContaining({ developer: { files: 230, timeline: 356 } }) }));
  });

  it('holds the host’s terminal while a divider is dragged, and lets go at the end', () => {
    renderWorkspace({ mode: 'host' });
    const divider = screen.getByRole('separator', { name: 'Resize the files panel' });
    expect(terminal.props.hold).toBe(false);
    fireEvent.pointerDown(divider, { button: 0, clientX: 230 });
    expect(terminal.props.hold).toBe(true);
    fireEvent.pointerMove(divider, { clientX: 330 });
    fireEvent.pointerUp(divider);
    expect(terminal.props.hold).toBe(false);
  });
});

describe('step detail', () => {
  it('switches the timeline between plain words and every step, as part of the layout', () => {
    const onLayout = vi.fn();
    renderWorkspace({ onLayout });
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    expect(within(first).getByText('Write')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Plain words' }));
    expect(onLayout).toHaveBeenLastCalledWith(expect.objectContaining({ steps: 'summary' }));
    expect(within(screen.getByRole('article', { name: 'Prompt 1' })).getByText('Changed 2 files')).toBeInTheDocument();
  });
});

describe('timeline panel', () => {
  it('shows prompts with the agent text, tool calls with their outcome, and diffs', async () => {
    renderWorkspace();
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    expect(first).toHaveTextContent('Add a /health endpoint');
    expect(within(first).getByText('Write')).toBeInTheDocument();
    expect(within(first).queryByLabelText('Created src/routes/health.ts')).not.toBeInTheDocument();
    await userEvent.click(within(first).getByText('Write'));
    expect(await within(first).findByLabelText('Created src/routes/health.ts')).toHaveTextContent("+import { Router } from 'express';");
    await userEvent.click(within(first).getByText('Failed'));
    expect(await within(first).findByText(/no test files match/)).toBeInTheDocument();
    expect(screen.getByRole('article', { name: 'Prompt 2' })).toHaveTextContent('Running');
  });

  it('jumps the terminal to a prompt', async () => {
    renderWorkspace();
    await userEvent.click(screen.getByRole('button', { name: /Prompt 2/ }));
    const prompt2 = demoTimeline.find((e) => e.kind === 'prompt' && e.promptId === 'p2')!;
    expect(terminal.handle.scrollToSeq).toHaveBeenCalledWith(prompt2.outputSeq);
  });

  it('lists every prompt to jump to', async () => {
    renderWorkspace();
    await userEvent.selectOptions(screen.getByLabelText('Jump to'), 'p1');
    const prompt1 = demoTimeline.find((e) => e.kind === 'prompt' && e.promptId === 'p1')!;
    expect(terminal.handle.scrollToSeq).toHaveBeenCalledWith(prompt1.outputSeq);
  });

  it('says a terminal session has no timeline, without promising one', () => {
    renderWorkspace({ timelineSupported: false, agentName: 'Terminal', terminal: true, timeline: [] });
    expect(screen.getByText('A terminal session has no timeline')).toBeInTheDocument();
    expect(screen.queryByText(/Claude Code's events/)).not.toBeInTheDocument();
    expect(screen.getByText('No file list for a terminal session')).toBeInTheDocument();
  });

  it('says so for an agent without a timeline', () => {
    renderWorkspace({ timelineSupported: false, agentName: 'OpenCode', timeline: [] });
    expect(screen.getByText('No timeline for OpenCode')).toBeInTheDocument();
    expect(screen.getByText(/This session’s files can’t be listed/)).toBeInTheDocument();
  });

  it('shows when the timeline is paused', () => {
    renderWorkspace({ timeline: [...demoTimeline, { outputSeq: 99, at: 0, kind: 'timeline-unavailable', reason: 'No events arrived from the agent.' }] });
    expect(screen.getByText(/Timeline paused: No events arrived from the agent./)).toBeInTheDocument();
    // Without a start command there is no sandbox to set up.
    expect(screen.queryByRole('button', { name: 'Setting up a sandbox' })).not.toBeInTheDocument();
  });

  it('links the sandbox guide when a sandbox does not reach SpecDeck', async () => {
    const reason = 'Claude Code in the sandbox reaches SpecDeck without its token: the start command does not pass SPECDECK_HOOK_TOKEN into the sandbox (for example with -e SPECDECK_HOOK_TOKEN).';
    const open = vi.spyOn(window, 'open').mockReturnValue(null);
    renderWorkspace({ timeline: [...demoTimeline, { outputSeq: 99, at: 0, kind: 'timeline-unavailable', reason, help: 'sandbox' }] });
    expect(screen.getByText(/Timeline paused: Claude Code in the sandbox reaches SpecDeck without its token/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Setting up a sandbox' }));
    expect(open).toHaveBeenCalledWith('https://nickhartjes.github.io/specdeck/guide/sandbox/', '_blank', 'noopener');
    open.mockRestore();
  });
});

describe('summary steps', () => {
  it('shows what the agent did in words, each step opening to its detail', async () => {
    renderWorkspace({ layout: adjust(choose('developer'), { steps: 'summary' }) });
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    const lines = [...first.querySelectorAll('.step > summary')].map((summary) => summary.textContent);
    expect(lines).toEqual([
      'Looked at 1 fileDone',
      'Asked: What should /health return when the database is down? → 503 Service UnavailableDone',
      'Changed 2 filesDone',
      'Ran the health testsAllowed by the hostFailed',
    ]);
    await userEvent.click(within(first).getByText('Changed 2 files'));
    expect(await within(first).findByLabelText('Created src/routes/health.ts')).toBeVisible();
  });

  it('says what the agent does right now when the terminal is not shown', () => {
    renderWorkspace({ layout: choose('overview') });
    expect(screen.getByRole('status', { name: 'What Claude Code is doing' })).toHaveTextContent(/^Claude is running the health tests again · \d+ s$/);
  });

  it('has no live line while the terminal shows what happens', () => {
    renderWorkspace({ layout: adjust(choose('developer'), { steps: 'summary' }) });
    expect(screen.queryByRole('status', { name: 'What Claude Code is doing' })).not.toBeInTheDocument();
  });
});

describe('tabs', () => {
  it('replaces the preview tab with the next file, and keeps one on a double click', () => {
    renderWorkspace({ timeline: planTimeline(), layout: adjust(choose('developer'), { filesTab: 'documents' }) });
    fireEvent.click(screen.getByRole('button', { name: /proposal\.md/ }));
    fireEvent.click(screen.getByRole('button', { name: /design\.md/ }));
    expect(tabNames()).toEqual(['Terminal, read-only', 'design.md, preview']);
    fireEvent.doubleClick(screen.getByRole('tab', { name: /design\.md/ }));
    fireEvent.click(screen.getByRole('button', { name: /proposal\.md/ }));
    expect(tabNames()).toEqual(['Terminal, read-only', 'design.md', 'proposal.md, preview']);
    expect(screen.getByRole('tab', { name: /proposal\.md/ })).toHaveAttribute('title', expect.stringContaining('Preview: replaced by the next file you open. Double-click to keep it.'));
  });

  it('keeps a preview someone scrolls or selects in', () => {
    renderWorkspace({ timeline: planTimeline(), layout: adjust(choose('developer'), { filesTab: 'documents' }) });
    fireEvent.click(screen.getByRole('button', { name: /design\.md/ }));
    fireEvent.wheel(screen.getByRole('tabpanel', { name: 'design.md' }));
    expect(tabNames()).toContain('design.md');
    fireEvent.click(screen.getByRole('button', { name: /proposal\.md/ }));
    vi.spyOn(window, 'getSelection').mockReturnValue({ toString: () => 'Demo' } as Selection);
    fireEvent.mouseUp(screen.getByRole('tabpanel', { name: 'proposal.md' }));
    expect(tabNames()).toEqual(['Terminal, read-only', 'design.md', 'proposal.md']);
  });

  it('never closes the terminal tab, and closes others', () => {
    renderWorkspace({ timeline: planTimeline(), layout: adjust(choose('developer'), { filesTab: 'documents' }) });
    expect(screen.queryByRole('button', { name: /Close Terminal/ })).not.toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole('tab', { name: 'Terminal, read-only' }), { key: 'Delete' });
    expect(tabNames()).toEqual(['Terminal, read-only']);
    fireEvent.click(screen.getByRole('button', { name: /design\.md/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Close design.md' }));
    expect(tabNames()).toEqual(['Terminal, read-only']);
    expect(screen.getByRole('tab', { name: 'Terminal, read-only' })).toHaveAttribute('aria-selected', 'true');
  });

  it('moves between tabs with the arrow keys', () => {
    renderWorkspace({ timeline: planTimeline(), layout: adjust(choose('developer'), { filesTab: 'documents' }) });
    fireEvent.doubleClick(screen.getByRole('button', { name: /design\.md/ }));
    fireEvent.keyDown(screen.getByRole('tab', { name: 'design.md' }), { key: 'ArrowRight' });
    expect(screen.getByRole('tab', { name: 'Terminal, read-only' })).toHaveAttribute('aria-selected', 'true');
  });
});

describe('files the agent touched', () => {
  it('lists code and opens every diff of a changed file in a tab', () => {
    renderWorkspace();
    const list = screen.getByRole('list', { name: 'Code' });
    expect(within(list).getAllByRole('button').map((b) => b.getAttribute('aria-label'))).toEqual(['src/routes/health.ts, new', 'src/routes/index.ts, changed']);
    fireEvent.click(screen.getByRole('button', { name: 'src/routes/index.ts, changed' }));
    expect(tabNames()).toEqual(['Terminal, read-only', 'index.ts, preview']);
    const tab = screen.getByRole('tabpanel', { name: 'index.ts' });
    expect(within(tab).getByText('Change 1 of 1 · show in terminal')).toBeInTheDocument();
    // Highlighted, so the line is split into coloured spans.
    expect(tab).toHaveTextContent('routes.use(health);');
  });

  it('opens a document whole, and remembers it was read', () => {
    renderWorkspace({ timeline: planTimeline(), layout: adjust(choose('developer'), { filesTab: 'documents' }) });
    fireEvent.click(screen.getByRole('button', { name: /design\.md/ }));
    const tab = screen.getByRole('tabpanel', { name: 'design.md' });
    expect(within(tab).getByRole('heading', { name: 'Design' })).toBeInTheDocument();
    expect(within(tab).getByRole('navigation', { name: 'Plan: Demo plan' })).toHaveTextContent('ProposalDesign');
  });

  it('shows the terminal where the agent read a file it only read', () => {
    const timeline: TimelineEvent[] = [
      { ...at(1), kind: 'prompt', promptId: 'p1', text: 'look' },
      { ...at(7), kind: 'tool-call', promptId: 'p1', toolUseId: 'r1', toolName: 'Read', input: { file_path: 'Cargo.toml' } },
      { ...at(8), kind: 'file-read', promptId: 'p1', toolUseId: 'r1', path: 'Cargo.toml' },
    ];
    renderWorkspace({ timeline, layout: choose('developer') });
    fireEvent.click(screen.getByRole('button', { name: 'Cargo.toml, read' }));
    expect(terminal.handle.scrollToSeq).toHaveBeenCalledWith(7, true);
    expect(screen.getByRole('tab', { name: 'Terminal, read-only' })).toHaveAttribute('aria-selected', 'true');
  });
});

describe('whole files', () => {
  /** Two code files the agent created, with their whole text shared. */
  function codeTimeline(): TimelineEvent[] {
    const write = (n: number, id: string, path: string, text: string): TimelineEvent[] => [
      { ...at(n), kind: 'tool-call', promptId: 'p1', toolUseId: id, toolName: 'Write', input: { file_path: path } },
      { ...at(n), kind: 'tool-result', promptId: 'p1', toolUseId: id, status: 'succeeded' },
      { ...at(n), kind: 'file-change', promptId: 'p1', toolUseId: id, path, change: 'created', hunks: [{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 1, lines: [`+${text}`] }] },
      { ...at(n), kind: 'file-text', promptId: 'p1', toolUseId: id, path, text },
    ];
    return [{ ...at(1), kind: 'prompt', promptId: 'p1', text: 'write' }, ...write(2, 'w1', 'src/a.rs', 'fn a() {}'), ...write(3, 'w2', 'src/b.rs', 'fn b() {}')];
  }
  const showing = (name: string) => within(screen.getByRole('tabpanel', { name })).getByRole('button', { name: 'Whole file, changes marked' }).getAttribute('aria-pressed');

  it('opens a code file on its changes, and keeps the view chosen for it', async () => {
    renderWorkspace({ timeline: codeTimeline(), wholeFiles: true });
    fireEvent.click(screen.getByRole('button', { name: 'src/a.rs, new' }));
    expect(showing('a.rs')).toBe('false');
    await userEvent.click(within(screen.getByRole('tabpanel', { name: 'a.rs' })).getByRole('button', { name: 'Whole file, changes marked' }));
    expect(showing('a.rs')).toBe('true');
    // Switching the view keeps the preview tab.
    expect(tabNames()).toEqual(['Terminal, read-only', 'a.rs']);
    await userEvent.click(screen.getByRole('button', { name: 'Close a.rs' }));
    fireEvent.click(screen.getByRole('button', { name: 'src/a.rs, new' }));
    expect(showing('a.rs')).toBe('true');
    fireEvent.click(screen.getByRole('button', { name: 'src/b.rs, new' }));
    expect(showing('b.rs')).toBe('false');
  });

  it('shows the change of a step, and keeps the view chosen for the file', async () => {
    renderWorkspace({ timeline: codeTimeline(), wholeFiles: true });
    fireEvent.click(screen.getByRole('button', { name: 'src/a.rs, new' }));
    await userEvent.click(within(screen.getByRole('tabpanel', { name: 'a.rs' })).getByRole('button', { name: 'Whole file, changes marked' }));
    const prompt = screen.getByRole('article', { name: 'Prompt 1' });
    await userEvent.click(within(prompt).getAllByText('Write')[0]!);
    expect(showing('a.rs')).toBe('false');
    expect(screen.getByRole('tabpanel', { name: 'a.rs' }).querySelector('.flash')).not.toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Close a.rs' }));
    fireEvent.click(screen.getByRole('button', { name: 'src/a.rs, new' }));
    expect(showing('a.rs')).toBe('true');
  });
});

describe('timeline steps open tabs', () => {
  it('opens the diffs of a step that changed a file, at that change', async () => {
    renderWorkspace();
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    await userEvent.click(within(first).getByText('Write'));
    expect(screen.getByRole('tab', { name: /health\.ts/ })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('tabpanel', { name: 'health.ts' }).querySelector('.flash')).not.toBeNull();
  });

  it('shows the terminal at a step without a file, highlighted', async () => {
    renderWorkspace();
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    await userEvent.click(within(first).getByText('Failed'));
    const bash = demoTimeline.find((e) => e.kind === 'tool-call' && e.toolName === 'Bash')!;
    expect(terminal.handle.scrollToSeq).toHaveBeenCalledWith(bash.outputSeq, true);
  });

  it('opens the first document a plan step changed, with a link to the other', async () => {
    renderWorkspace({ timeline: planTimeline(), layout: adjust(choose('developer'), { steps: 'summary' }) });
    expect(within(screen.getByRole('article', { name: 'Prompt 1' })).getByText('Wrote the plan')).toBeInTheDocument();
    await userEvent.click(within(screen.getByRole('article', { name: 'Prompt 2' })).getByRole('button', { name: 'Read the changes' }));
    const tab = screen.getByRole('tabpanel', { name: 'design.md' });
    expect(tab).toHaveTextContent('Second.');
    fireEvent.click(within(tab).getByRole('button', { name: 'Proposal' }));
    expect(screen.getByRole('tab', { name: /proposal\.md/ })).toHaveAttribute('aria-selected', 'true');
  });

  it('opens a plan document from the plan card', () => {
    renderWorkspace({ timeline: planTimeline() });
    fireEvent.click(within(screen.getByRole('region', { name: 'Plan: Demo plan' })).getByRole('button', { name: /Design/ }));
    expect(screen.getByRole('tab', { name: /design\.md/ })).toHaveAttribute('aria-selected', 'true');
  });
});

describe('the terminal tab', () => {
  it('has none for a viewer in Overview, who gets a document in front, and keeps the terminal out of sight', () => {
    const ref = createRef<WorkspaceHandle>();
    renderWorkspace({ ref, timeline: planTimeline(), layout: choose('overview') });
    expect(tabNames()).toEqual(['proposal.md, preview']);
    const element = screen.getByTestId('terminal');
    expect(screen.getByLabelText('Terminal, read-only', { selector: 'section' })).toHaveAttribute('aria-hidden', 'true');
    // "hi" as an output record of the terminal stream, written while out of sight.
    act(() => ref.current!.terminalData(new Uint8Array([1, 0, 0, 0, 2, 104, 105]).buffer));
    expect(terminal.handle.write).toHaveBeenCalled();
    // Cmd+2 adds the tab, with the same terminal.
    act(() => dispatchMenu({ id: 'panel:terminal' }));
    expect(tabNames()[0]).toBe('Terminal, read-only');
    expect(screen.getByTestId('terminal')).toBe(element);
    expect(screen.getByRole('tabpanel', { name: 'Terminal, read-only' })).not.toHaveAttribute('aria-hidden');
  });

  it('is there for a session without a timeline, whatever the layout', () => {
    renderWorkspace({ timelineSupported: false, agentName: 'OpenCode', timeline: [], layout: choose('overview') });
    expect(tabNames()).toEqual(['Terminal, read-only']);
  });

  it('stays for the host in Overview', () => {
    renderWorkspace({ mode: 'host', timeline: planTimeline(), layout: choose('overview') });
    expect(tabNames()).toEqual(['Terminal', 'proposal.md, preview']);
  });

  it('holds the host’s terminal while another tab is shown', () => {
    renderWorkspace({ mode: 'host', timeline: planTimeline(), layout: adjust(choose('developer'), { filesTab: 'documents' }) });
    expect(terminal.props.hold).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: /design\.md/ }));
    expect(terminal.props.hold).toBe(true);
    fireEvent.click(screen.getByRole('tab', { name: 'Terminal' }));
    expect(terminal.props.hold).toBe(false);
  });
});

describe('opening from a notification', () => {
  const design = 'openspec/changes/demo/design.md';
  const planMode = (approved?: boolean): TimelineEvent[] => [
    ...planTimeline(),
    { ...at(10), kind: 'prompt', promptId: 'p3', text: 'plan the next step' },
    { ...at(11), kind: 'tool-call', promptId: 'p3', toolUseId: 'x1', toolName: 'ExitPlanMode', input: { plan: '# Plan: next step' } },
    ...(approved === undefined
      ? []
      : [{ ...at(12), kind: 'tool-result', promptId: 'p3', toolUseId: 'x1', status: 'succeeded', outcome: { kind: 'plan-decision', approved } } as TimelineEvent]),
  ];

  it('opens a document in the preview tab', () => {
    const ref = createRef<WorkspaceHandle>();
    renderWorkspace({ ref, timeline: planTimeline() });
    act(() => ref.current!.open({ kind: 'document', path: design }));
    expect(screen.getByRole('tab', { name: 'design.md, preview' })).toHaveAttribute('aria-selected', 'true');
  });

  it('selects a document kept open, without replacing the preview', () => {
    const ref = createRef<WorkspaceHandle>();
    renderWorkspace({ ref, timeline: planTimeline() });
    act(() => ref.current!.open({ kind: 'document', path: design }));
    fireEvent.doubleClick(screen.getByRole('tab', { name: /design\.md/ }));
    act(() => ref.current!.open({ kind: 'document', path: 'openspec/changes/demo/proposal.md' }));
    fireEvent.click(screen.getByRole('tab', { name: 'Terminal, read-only' }));
    act(() => ref.current!.open({ kind: 'document', path: design }));
    expect(tabNames()).toEqual(['Terminal, read-only', 'design.md', 'proposal.md, preview']);
    expect(screen.getByRole('tab', { name: 'design.md' })).toHaveAttribute('aria-selected', 'true');
  });

  it("opens the agent's plan in a plan tab that follows the host's decision", () => {
    const ref = createRef<WorkspaceHandle>();
    const { rerender } = renderWorkspace({ ref, timeline: planMode() });
    act(() => ref.current!.open({ kind: 'plan-mode', toolUseId: 'x1' }));
    expect(screen.getByRole('tab', { name: "Claude Code's plan, preview" })).toHaveAttribute('aria-selected', 'true');
    const panel = screen.getByRole('tabpanel', { name: "Claude Code's plan" });
    expect(within(panel).getByText('Waiting for the host')).toBeInTheDocument();
    rerender(<Harness ref={ref} timeline={planMode(true)} />);
    expect(within(screen.getByRole('tabpanel', { name: "Claude Code's plan" })).getByText('Approved')).toBeInTheDocument();
  });

  it('opens nothing for a document or plan it does not have', () => {
    const ref = createRef<WorkspaceHandle>();
    renderWorkspace({ ref, timeline: planTimeline() });
    const before = tabNames();
    act(() => ref.current!.open({ kind: 'document', path: 'gone.md' }));
    act(() => ref.current!.open({ kind: 'plan-mode', toolUseId: 'w1' }));
    expect(tabNames()).toEqual(before);
  });

  it('opens a shared file or braindump item while it is still there', () => {
    const file = (id: number, name: string): SharedFileInfo => ({ file: id, name, kind: 'image', mediaType: 'image/png', size: 10, sha256: [], by: 0, byLabel: 'Nick', atMs: 0 });
    const shared: SharedFiles = {
      files: [file(1, 'wireframe.png')],
      me: 2,
      host: false,
      canShare: true,
      hostName: 'Nick',
      agentName: 'Claude Code',
      share: vi.fn(),
      open: vi.fn(async () => new ArrayBuffer(0)),
      remove: vi.fn(),
      save: vi.fn(async () => true),
      say: vi.fn(),
      braindump: { items: [{ info: file(2, 'sketch.png'), note: false, given: null }], add: vi.fn(), write: vi.fn(), edit: vi.fn(), remove: vi.fn() } as unknown as SharedFiles['braindump'],
    };
    const ref = createRef<WorkspaceHandle>();
    renderWorkspace({ ref, timeline: planTimeline(), shared });
    const before = tabNames();
    act(() => ref.current!.open({ kind: 'shared', file: 9 }));
    expect(tabNames()).toEqual(before);
    act(() => ref.current!.open({ kind: 'shared', file: 1 }));
    expect(screen.getByRole('tab', { name: 'wireframe.png, preview' })).toHaveAttribute('aria-selected', 'true');
    act(() => ref.current!.open({ kind: 'shared', file: 2 }));
    expect(screen.getByRole('tab', { name: 'sketch.png, preview' })).toHaveAttribute('aria-selected', 'true');
  });

  it('shows the session on a narrow window', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({ matches: query.includes('max-width'), addEventListener: vi.fn(), removeEventListener: vi.fn() }));
    const ref = createRef<WorkspaceHandle>();
    renderWorkspace({ ref, timeline: planTimeline() });
    const switcher = screen.getByRole('navigation', { name: 'Panels' });
    fireEvent.click(within(switcher).getByRole('button', { name: 'Timeline' }));
    act(() => ref.current!.open({ kind: 'document', path: design }));
    expect(within(switcher).getByRole('button', { name: 'Session' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('tab', { name: /design\.md/ })).toHaveAttribute('aria-selected', 'true');
  });
});

describe('narrow window', () => {
  it('shows one panel at a time, and switches to Session to show what is opened', () => {
    vi.stubGlobal('matchMedia', (query: string) => ({ matches: query.includes('max-width'), addEventListener: vi.fn(), removeEventListener: vi.fn() }));
    renderWorkspace();
    const switcher = screen.getByRole('navigation', { name: 'Panels' });
    expect(within(switcher).getByRole('button', { name: 'Session' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.queryByRole('separator')).not.toBeInTheDocument();
    expect(pane('Timeline')).not.toBeVisible();
    fireEvent.click(within(switcher).getByRole('button', { name: 'Files' }));
    expect(pane('Files')).toBeVisible();
    expect(document.querySelector<HTMLElement>('.tab-area')!).not.toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: 'src/routes/index.ts, changed' }));
    expect(within(switcher).getByRole('button', { name: 'Session' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('tab', { name: /index\.ts/ })).toHaveAttribute('aria-selected', 'true');
  });
});

describe('search', () => {
  it('opens with the find shortcut and searches the terminal and the timeline', async () => {
    renderWorkspace();
    fireEvent.keyDown(window, { key: 'f', metaKey: true, ctrlKey: false });
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    const field = await screen.findByLabelText('Find in session');
    await userEvent.type(field, 'health');
    expect(terminal.handle.findNext).toHaveBeenLastCalledWith('health');
    act(() => (terminal.props.onSearchResults as (r: { index: number; count: number }) => void)({ index: 0, count: 4 }));
    expect(screen.getByRole('search')).toHaveTextContent('1/4 in terminal · 2 in timeline');
    await userEvent.type(field, '{Enter}');
    expect(screen.getByRole('article', { name: 'Prompt 1' })).toHaveClass('current');
    await userEvent.type(field, '{Escape}');
    expect(screen.queryByRole('search')).not.toBeInTheDocument();
    expect(terminal.handle.clearSearch).toHaveBeenCalled();
  });

  it('says when there are no matches', async () => {
    renderWorkspace();
    fireEvent.keyDown(window, { key: 'f', metaKey: true });
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });
    await userEvent.type(await screen.findByLabelText('Find in session'), 'zebra');
    expect(screen.getByRole('search')).toHaveTextContent('No matches');
  });
});

describe('F6', () => {
  it('moves between the panels and the tabs, skipping closed panels', () => {
    renderWorkspace({ layout: adjust(choose('developer'), { files: false }) });
    // No toolbar in this test: the tab area comes first, where the terminal takes the focus.
    fireEvent.keyDown(window, { key: 'F6' });
    expect(terminal.handle.focus).toHaveBeenCalled();
    screen.getByRole('tab', { name: 'Terminal, read-only' }).focus();
    fireEvent.keyDown(window, { key: 'F6' });
    expect(pane('Timeline')).toContainElement(document.activeElement as HTMLElement);
    fireEvent.keyDown(window, { key: 'F6' });
    expect(terminal.handle.focus).toHaveBeenCalledTimes(2);
  });
});

describe('following the output', () => {
  it('offers to jump back to live output after scrolling away', async () => {
    renderWorkspace();
    expect(screen.queryByRole('button', { name: /Jump to live/ })).not.toBeInTheDocument();
    act(() => (terminal.props.onFollowChange as (f: boolean) => void)(false));
    act(() => (terminal.props.onNewOutput as () => void)());
    await userEvent.click(screen.getByRole('button', { name: 'New output · jump to live' }));
    expect(terminal.handle.scrollToBottom).toHaveBeenCalled();
    act(() => (terminal.props.onFollowChange as (f: boolean) => void)(true));
    expect(screen.queryByRole('button', { name: /Jump to live/ })).not.toBeInTheDocument();
  });
});

describe('screenshot tabs', () => {
  const shot: TimelineEvent[] = [
    { ...at(1), kind: 'prompt', promptId: 'p', text: 'Check the login page' },
    { ...at(2), kind: 'tool-call', promptId: 'p', toolUseId: 'shot', toolName: 'mcp__specdeck__screenshot', input: {} },
    {
      ...at(3),
      kind: 'tool-result',
      promptId: 'p',
      toolUseId: 'shot',
      status: 'succeeded',
      outcome: { kind: 'screenshot', image: { mediaType: 'image/png', data: 'iVBORw0K', width: 1280, height: 800 }, address: 'localhost:5173/login' },
    },
  ];

  for (const preset of ['developer', 'overview'] as const) {
    it(`opens a thumbnail full size in a tab, in ${preset === 'overview' ? 'Overview, without a terminal' : 'Developer'}`, async () => {
      renderWorkspace({ timeline: shot, layout: choose(preset) });
      await userEvent.click(screen.getByRole('button', { name: 'Screenshot of localhost:5173/login' }));
      expect(tabNames()).toContain('localhost:5173/login, preview');
      const tab = screen.getByRole('tabpanel', { name: 'localhost:5173/login' });
      const picture = within(tab).getByRole('img', { name: 'Screenshot of localhost:5173/login' });
      expect(picture).toHaveAttribute('width', '1280');
    });
  }
});

