import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { describe, expect, it } from 'vitest';
import type { TimelineEvent } from '../api/types';
import { buildTimeline } from '../model/timeline';
import { TimelinePanel } from './TimelinePanel';

const stamp = () => ({ outputSeq: 1, at: 1 });
const questions = [
  { question: 'Which database?', header: 'Database', options: [{ label: 'Postgres', description: 'Relational, robust' }, { label: 'SQLite' }], multiSelect: false },
  { question: 'Which features?', header: 'Features', options: [{ label: 'Auth' }, { label: 'Search' }, { label: 'Export' }], multiSelect: true },
];

function events(): TimelineEvent[] {
  return [
    { ...stamp(), kind: 'prompt', promptId: 'p', text: 'build it' },
    // An answered question, and one still waiting.
    { ...stamp(), kind: 'tool-call', promptId: 'p', toolUseId: 'q1', toolName: 'AskUserQuestion', input: { questions } },
    {
      ...stamp(),
      kind: 'tool-result',
      promptId: 'p',
      toolUseId: 'q1',
      status: 'succeeded',
      outcome: {
        kind: 'answers',
        answers: [
          { question: 'Which database?', selected: ['Postgres'], notes: 'We already run it' },
          { question: 'Which features?', selected: ['Auth', 'Export'], typed: 'Audit log' },
        ],
      },
    },
    // A rejected plan, then an approved one.
    { ...stamp(), kind: 'tool-call', promptId: 'p', toolUseId: 'x1', toolName: 'ExitPlanMode', input: { plan: '# Plan: hello.txt' } },
    { ...stamp(), kind: 'tool-result', promptId: 'p', toolUseId: 'x1', status: 'not-run', outcome: { kind: 'plan-decision', approved: false } },
    { ...stamp(), kind: 'tool-call', promptId: 'p', toolUseId: 'x2', toolName: 'ExitPlanMode', input: { plan: '# Plan: greeting.txt' } },
    { ...stamp(), kind: 'tool-result', promptId: 'p', toolUseId: 'x2', status: 'succeeded', outcome: { kind: 'plan-decision', approved: true } },
    // An allowed and a denied command.
    { ...stamp(), kind: 'tool-call', promptId: 'p', toolUseId: 'b1', toolName: 'Bash', input: { command: 'npm test', description: 'Run the test suite' } },
    { ...stamp(), kind: 'tool-result', promptId: 'p', toolUseId: 'b1', status: 'succeeded', permission: 'allowed' },
    { ...stamp(), kind: 'tool-call', promptId: 'p', toolUseId: 'b2', toolName: 'Bash', input: { command: 'rm -rf build', description: 'Delete the build folder' } },
    { ...stamp(), kind: 'tool-result', promptId: 'p', toolUseId: 'b2', status: 'not-run', permission: 'denied' },
    // A to-do list, and a helper whose report came later and was cut.
    {
      ...stamp(),
      kind: 'tool-call',
      promptId: 'p',
      toolUseId: 'td',
      toolName: 'TodoWrite',
      input: {
        todos: [
          { content: 'Count files', status: 'completed', activeForm: 'Counting files' },
          { content: 'Report back', status: 'in_progress', activeForm: 'Reporting back' },
          { content: 'Clean up', status: 'pending', activeForm: 'Cleaning up' },
        ],
      },
    },
    { ...stamp(), kind: 'tool-result', promptId: 'p', toolUseId: 'td', status: 'succeeded' },
    { ...stamp(), kind: 'tool-call', promptId: 'p', toolUseId: 'h', toolName: 'Agent', input: { description: 'Count the files here', prompt: 'Count.' } },
    { ...stamp(), kind: 'tool-result', promptId: 'p', toolUseId: 'h', status: 'succeeded' },
    { ...stamp(), kind: 'tool-result', promptId: 'p', toolUseId: 'h', status: 'succeeded', outcome: { kind: 'report', text: '- **Files:** 1', truncated: true } },
    { ...stamp(), kind: 'tool-call', promptId: 'p', toolUseId: 'q2', toolName: 'AskUserQuestion', input: { questions: [questions[0]] } },
  ];
}

function panel(steps: 'summary' | 'details') {
  // In the app the panel sits inside the main landmark.
  return render(
    <main>
      <TimelinePanel
      model={buildTimeline(events())}
      timelineSupported
      agentName="Claude Code"
      steps={steps}
      live
      matches={new Set()}
      current={null}
      onJump={() => {}}
      />
    </main>,
  );
}

async function open(name: RegExp) {
  const summary = screen.getByText(name).closest('summary')!;
  await userEvent.click(summary);
  // The body is rendered once the step has opened.
  await waitFor(() => expect(summary.nextElementSibling).not.toBeNull());
  return summary.parentElement!;
}

/** Opens the call a step stands for. */
async function openCall(step: HTMLElement) {
  const summary = step.querySelector<HTMLElement>('details.tool > summary')!;
  await userEvent.click(summary);
  await waitFor(() => expect(summary.nextElementSibling).not.toBeNull());
  return summary.parentElement!;
}

/** A step that is shown opened, without clicking it. */
function shown(name: RegExp) {
  const step = screen.getByText(name).closest('details')!;
  expect(step).toHaveAttribute('open');
  return step;
}

describe('host interactions', () => {
  it('summarises what the host decided', () => {
    panel('summary');
    const lines = [...document.querySelectorAll('details.step > summary')].map((summary) => summary.textContent);
    expect(lines).toEqual([
      'Asked: Which database? → Postgres +1Done',
      'Plan rejectedDenied by the host',
      'Plan approvedDone',
      'Ran the test suiteAllowed by the hostDone',
      'Delete the build folderDenied by the host',
      'Updated its to-do listDone',
      'Asked a helper: Count the files hereDone',
      'Asked: Which database?Running',
    ]);
    expect(screen.getByRole('status')).toHaveTextContent("Waiting for the host's answer: Which database?");
  });

  it('shows every option with the host’s picks, typed answer and notes', () => {
    panel('summary');
    const step = shown(/→ Postgres/);
    const database = within(step).getByRole('group', { name: 'Which database?' });
    expect(within(database).getAllByRole('listitem').map((li) => [li.textContent, li.getAttribute('aria-current')])).toEqual([
      ['Postgres (chosen)Relational, robust', 'true'],
      ['SQLite', null],
    ]);
    expect(database).toHaveTextContent('Notes: We already run it');
    const features = within(step).getByRole('group', { name: 'Which features?' });
    expect(within(features).getAllByRole('listitem', { current: true }).map((li) => li.textContent)).toEqual(['Auth (chosen)', 'Export (chosen)']);
    expect(features).toHaveTextContent('Typed by the host: Audit log');
  });

  it('shows an open question as waiting, with no pick', () => {
    panel('summary');
    const step = shown(/^Asked: Which database\?$/);
    expect(within(step).queryAllByRole('listitem', { current: true })).toEqual([]);
    expect(step).toHaveTextContent("Waiting for the host's answer.");
  });

  it('shows plans formatted, with the decision', () => {
    panel('summary');
    const rejected = shown(/Plan rejected/);
    expect(within(rejected).getByRole('heading', { name: 'Plan: hello.txt' })).toBeInTheDocument();
    expect(rejected).toHaveTextContent('Rejected');
    expect(shown(/Plan approved/)).toHaveTextContent('Approved');
  });

  it('shows a to-do list as a checklist and a helper report formatted', async () => {
    panel('summary');
    const todos = within(await openCall(await open(/to-do list/))).getByRole('list', { name: 'To-do list' });
    expect(within(todos).getAllByRole('listitem').map((li) => li.textContent)).toEqual([
      'Count files (Done)',
      'Report back (In progress)',
      'Clean up (To do)',
    ]);
    const helper = await openCall(await open(/Asked a helper/));
    expect(within(helper).getByText('Files:').tagName).toBe('STRONG');
    expect(helper).toHaveTextContent('The report was cut to its first 200 KB.');
  });

  it('shows questions and plans opened in details too, and folds the rest', () => {
    panel('details');
    const opened = (text: string) => screen.getAllByText(text, { selector: '.tool .name' }).map((name) => name.closest('details')!.hasAttribute('open'));
    expect(opened('AskUserQuestion')).toEqual([true, true]);
    expect(opened('ExitPlanMode')).toEqual([true, true]);
    expect(opened('TodoWrite')).toEqual([false]);
    expect(opened('Agent')).toEqual([false]);
    expect(opened('Bash')).toEqual([false, false]);
  });

  it('marks the denied tool in details too', () => {
    panel('details');
    const denied = screen.getByText('rm -rf build').closest('summary')!;
    expect(denied).toHaveTextContent('Denied by the host');
    expect(denied).toHaveTextContent('Did not run');
  });

  it('is accessible', async () => {
    panel('details');
    for (const details of document.querySelectorAll('details:not([open])')) await userEvent.click(details.querySelector('summary')!);
    const results = await axe.run(document.body, { rules: { 'color-contrast': { enabled: false } } });
    expect(results.violations.flatMap((v) => v.nodes.map((n) => v.id + ' ' + n.target.join(' ')))).toEqual([]);
  });
});
