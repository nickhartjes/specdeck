import { createRef } from 'react';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { demoParticipants, demoTimeline } from '../api/demo';
import { terminal, TerminalViewStub } from '../test/terminal-stub';
import { Workspace, type WorkspaceHandle } from './Workspace';

vi.mock('../terminal/TerminalView', () => ({ TerminalView: TerminalViewStub }));

function renderWorkspace(props: Partial<Parameters<typeof Workspace>[0]> = {}) {
  return render(
    <Workspace
      mode="viewer"
      agentName="Claude Code"
      timelineSupported
      timeline={demoTimeline}
      participants={demoParticipants}
      me={2}
      {...props}
    />,
  );
}

beforeEach(() => vi.clearAllMocks());

describe('timeline panel', () => {
  it('shows prompts with the agent text, tool calls with their outcome, and diffs', async () => {
    renderWorkspace();
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    expect(first).toHaveTextContent('Add a /health endpoint');
    expect(within(first).getByText('Write')).toBeInTheDocument();
    // A finished turn shows its diffs once a call is opened.
    expect(within(first).queryByLabelText('Created src/routes/health.ts')).not.toBeInTheDocument();
    await userEvent.click(within(first).getByText('Write'));
    expect(await within(first).findByLabelText('Created src/routes/health.ts')).toHaveTextContent("+import { Router } from 'express';");
    await userEvent.click(within(first).getByText('Failed'));
    expect(await within(first).findByText(/no test files match/)).toBeInTheDocument();
    expect(screen.getByRole('article', { name: 'Prompt 2' })).toHaveTextContent('Running');
  });

  it('jumps the terminal to where an entry happened', async () => {
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
    fireEvent.click(screen.getByRole('tab', { name: /Timeline/ }));
    expect(screen.getByText('A terminal session has no timeline')).toBeInTheDocument();
    expect(screen.queryByText(/Claude Code's events/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: /Files/ }));
    expect(screen.getByText('No file list for a terminal session')).toBeInTheDocument();
  });

  it('says so for an agent without a timeline', () => {
    renderWorkspace({ timelineSupported: false, agentName: 'OpenCode', timeline: [] });
    // Opens on People, the only tab with content; the timeline explains itself.
    expect(screen.getByRole('tab', { name: /People/ })).toHaveAttribute('aria-selected', 'true');
    fireEvent.click(screen.getByRole('tab', { name: /Timeline/ }));
    expect(screen.getByText('No timeline for OpenCode')).toBeInTheDocument();
  });

  it('shows when the timeline is paused', () => {
    renderWorkspace({
      timeline: [...demoTimeline, { outputSeq: 99, at: 0, kind: 'timeline-unavailable', reason: 'No events arrived from the agent.' }],
    });
    expect(screen.getByText(/Timeline paused: No events arrived from the agent./)).toBeInTheDocument();
  });
});

describe('summary steps', () => {
  it('shows what the agent did in words, each step opening to its detail', async () => {
    renderWorkspace({ steps: 'summary' });
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    const lines = [...first.querySelectorAll('.step > summary')].map((summary) => summary.textContent);
    expect(lines).toEqual([
      'Looked at 1 fileDone',
      'Asked: What should /health return when the database is down? → 503 Service UnavailableDone',
      'Changed 2 filesDone',
      'Ran the health testsAllowed by the hostFailed',
    ]);
    expect(within(first).queryByLabelText('Created src/routes/health.ts')).not.toBeInTheDocument();
    await userEvent.click(within(first).getByText('Changed 2 files'));
    expect(await within(first).findByLabelText('Created src/routes/health.ts')).toBeVisible();
  });

  it('says what the agent does right now when the terminal is hidden', () => {
    const { rerender } = renderWorkspace({ steps: 'summary', terminalHidden: true });
    expect(screen.getByRole('status', { name: 'What Claude Code is doing' })).toHaveTextContent(/^Claude is running the health tests again · \d+ s$/);
    rerender(
      <Workspace
        mode="viewer"
        agentName="Claude Code"
        timelineSupported
        timeline={[...demoTimeline, { outputSeq: 99, at: 99, kind: 'attention', message: 'Claude needs your permission to use Bash' }]}
        participants={demoParticipants}
        steps="summary"
        terminalHidden
      />,
    );
    expect(screen.getByRole('status', { name: 'What Claude Code is doing' })).toHaveTextContent('Waiting for the host: Claude needs your permission to use Bash');
  });

  it('has no live line while the terminal shows what happens', () => {
    renderWorkspace({ steps: 'summary' });
    expect(screen.queryByRole('status', { name: 'What Claude Code is doing' })).not.toBeInTheDocument();
  });
});

describe('documents', () => {
  it('opens a changed document from the Files tab, and remembers it was read', async () => {
    const at = (n: number) => ({ outputSeq: n, at: n });
    const timeline = [
      ...demoTimeline,
      { ...at(5), kind: 'tool-call' as const, promptId: 'p2', toolUseId: 'd1', toolName: 'Write', input: { file_path: 'README.md' } },
      { ...at(5), kind: 'tool-result' as const, promptId: 'p2', toolUseId: 'd1', status: 'succeeded' as const },
      { ...at(5), kind: 'file-change' as const, promptId: 'p2', toolUseId: 'd1', path: 'README.md', change: 'created' as const, hunks: [] },
      { ...at(5), kind: 'document' as const, promptId: 'p2', toolUseId: 'd1', path: 'README.md', text: '# Webshop\n\nRun it with npm start.' },
    ];
    renderWorkspace({ timeline });
    await userEvent.click(screen.getByRole('tab', { name: /Files/ }));
    await userEvent.click(screen.getByRole('button', { name: /README\.md/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Read the document' }));
    const reader = screen.getByRole('dialog', { name: 'README.md' });
    expect(within(reader).getByRole('heading', { name: 'Webshop' })).toBeInTheDocument();
    await userEvent.click(within(reader).getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });
});

describe('plan steps', () => {
  it('open the reader on the documents a step changed', async () => {
    const at = (n: number) => ({ outputSeq: n, at: n });
    const dir = 'openspec/changes/demo';
    const write = (n: number, prompt: string, name: string, file: string, text: string, created: boolean) => [
      { ...at(n), kind: 'tool-call' as const, promptId: prompt, toolUseId: name, toolName: created ? 'Write' : 'Edit', input: { file_path: `${dir}/${file}` } },
      { ...at(n), kind: 'tool-result' as const, promptId: prompt, toolUseId: name, status: 'succeeded' as const },
      { ...at(n), kind: 'file-change' as const, promptId: prompt, toolUseId: name, path: `${dir}/${file}`, change: created ? ('created' as const) : ('edited' as const), hunks: [] },
      { ...at(n), kind: 'document' as const, promptId: prompt, toolUseId: name, path: `${dir}/${file}`, text },
    ];
    const plan = {
      ...at(9),
      kind: 'plan' as const,
      change: 'demo',
      documents: [
        { id: 'proposal', name: 'Proposal', paths: [`${dir}/proposal.md`], exists: true },
        { id: 'design', name: 'Design', paths: [`${dir}/design.md`], exists: true },
      ],
      tasks: { done: 0, total: 0 },
    };
    const timeline = [
      { ...at(1), kind: 'prompt' as const, promptId: 'p1', text: '/opsx:propose demo' },
      ...write(2, 'p1', 'w1', 'proposal.md', '# Demo plan', true),
      ...write(3, 'p1', 'w2', 'design.md', '# Design\n\nFirst.', true),
      { ...at(4), kind: 'prompt' as const, promptId: 'p2', text: 'change the design' },
      ...write(5, 'p2', 'e1', 'design.md', '# Design\n\nSecond.', false),
      plan,
    ];
    renderWorkspace({ timeline, steps: 'summary' });
    const first = screen.getByRole('article', { name: 'Prompt 1' });
    expect(within(first).getByText('Wrote the plan')).toBeInTheDocument();
    await userEvent.click(within(screen.getByRole('article', { name: 'Prompt 2' })).getByRole('button', { name: 'Read the changes' }));
    const reader = screen.getByRole('dialog', { name: 'Design' });
    expect(reader).toHaveTextContent('Plan · Demo plan');
    expect(within(reader).getByRole('navigation', { name: 'Documents' })).toHaveTextContent('ProposalDesign');
    expect(reader).toHaveTextContent('Second.');
  });
});

describe('hidden terminal', () => {
  it('keeps receiving output while hidden and comes back without a remount', () => {
    const ref = createRef<WorkspaceHandle>();
    const { rerender } = render(
      <Workspace ref={ref} mode="viewer" agentName="Claude Code" timelineSupported timeline={demoTimeline} participants={demoParticipants} terminalHidden />,
    );
    const element = screen.getByTestId('terminal');
    expect(screen.getByLabelText('Terminal, read-only', { selector: 'section' })).toHaveAttribute('aria-hidden', 'true');
    // "hi" as an output record of the terminal stream, written while hidden.
    act(() => ref.current!.terminalData(new Uint8Array([1, 0, 0, 0, 2, 104, 105]).buffer));
    expect(terminal.handle.write).toHaveBeenCalled();
    rerender(<Workspace ref={ref} mode="viewer" agentName="Claude Code" timelineSupported timeline={demoTimeline} participants={demoParticipants} />);
    expect(screen.getByTestId('terminal')).toBe(element);
    expect(screen.getByRole('region', { name: 'Terminal, read-only' })).not.toHaveAttribute('aria-hidden');
  });
});

describe('files panel', () => {
  it('lists touched files and opens every diff of a changed file', async () => {
    renderWorkspace();
    await userEvent.click(screen.getByRole('tab', { name: /Files/ }));
    const list = screen.getByRole('list', { name: 'Touched files' });
    expect(within(list).getAllByRole('button').map((b) => b.getAttribute('aria-label'))).toEqual([
      'src/routes/health.ts, 1 change',
      'src/routes/index.ts, 1 change',
    ]);
    await userEvent.click(screen.getByRole('button', { name: 'src/routes/index.ts, 1 change' }));
    expect(screen.getByText('Change 1 of 1 · show in terminal')).toBeInTheDocument();
    expect(screen.getByText('routes.use(health);')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /All files/ }));
    expect(screen.getByRole('list', { name: 'Touched files' })).toBeInTheDocument();
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
    // Both prompts mention it; stepping highlights them in turn.
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
