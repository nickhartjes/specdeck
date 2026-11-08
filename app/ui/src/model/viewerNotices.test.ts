import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AgentStatus, PlanDocument, TimelineEvent, ViewerNotifications } from '../api/types';
import { createViewerNotices, SETTLE_MS, type ViewerNotice } from './viewerNotices';

const DEFAULTS: ViewerNotifications = { plans: true, documents: true, finished: false };

let seq = 0;
function event(entry: Record<string, unknown>): TimelineEvent {
  seq += 1;
  return { outputSeq: seq, at: seq, ...entry } as unknown as TimelineEvent;
}

function documentWritten(path: string, change: 'created' | 'edited' | 'deleted' = 'edited'): TimelineEvent[] {
  const ids = { promptId: 'p', toolUseId: `t${seq}` };
  const written = [event({ kind: 'file-change', ...ids, path, change, hunks: [] })];
  if (change !== 'deleted') written.push(event({ kind: 'document', ...ids, path, text: '# text' }));
  return written;
}

const SPEC_DRIVEN = ['proposal', 'specs', 'design', 'tasks'];
function planState(change: string, tasks = { done: 0, total: 8 }): TimelineEvent {
  const documents: PlanDocument[] = SPEC_DRIVEN.map((id) => ({
    id,
    name: id,
    paths: [id === 'specs' ? `openspec/changes/${change}/specs/login/spec.md` : `openspec/changes/${change}/${id}.md`],
    exists: true,
  }));
  return event({ kind: 'plan', change, documents, tasks });
}

function planWritten(change: string, ids: string[], tasks?: { done: number; total: number }): TimelineEvent[] {
  return [
    ...ids.flatMap((id) => documentWritten(id === 'specs' ? `openspec/changes/${change}/specs/login/spec.md` : `openspec/changes/${change}/${id}.md`)),
    planState(change, tasks),
  ];
}

function setup(switches: Partial<ViewerNotifications> = {}) {
  const sent: ViewerNotice[] = [];
  const current = { ...DEFAULTS, ...switches };
  const notices = createViewerNotices({ switches: current, notify: (notice) => sent.push(notice) });
  notices.joined('Claude Code');
  const status = (s: AgentStatus) => notices.status(s);
  const timeline = (events: TimelineEvent[]) => events.forEach((e) => notices.timeline(e));
  /** A turn from working to waiting, with `events` in between, settled. */
  const turn = (events: TimelineEvent[]) => {
    status('working');
    timeline(events);
    status('waiting');
    vi.advanceTimersByTime(SETTLE_MS);
  };
  return { notices, sent, status, timeline, turn, current };
}

describe('viewer notices', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  describe('turns', () => {
    it('never notifies about history received on joining', () => {
      const { sent, status, timeline } = setup({ finished: true });
      timeline([...planWritten('add-login', SPEC_DRIVEN), ...documentWritten('README.md')]);
      status('waiting');
      timeline(documentWritten('notes.md'));
      vi.advanceTimersByTime(SETTLE_MS * 4);
      expect(sent).toEqual([]);
    });

    it('does not follow a turn already running on joining, and follows the next', () => {
      const { sent, status, timeline, turn } = setup();
      status('working');
      timeline(documentWritten('README.md'));
      status('waiting');
      vi.advanceTimersByTime(SETTLE_MS);
      expect(sent).toEqual([]);
      turn(documentWritten('notes.md'));
      expect(sent).toEqual([{ kind: 'documents', title: 'notes.md changed', body: 'Claude Code changed it in this turn.' }]);
    });

    it('waits through a permission and notifies once at the end of the turn', () => {
      const { sent, status, timeline } = setup();
      status('waiting');
      status('working');
      timeline(documentWritten('README.md'));
      timeline([event({ kind: 'attention', message: 'Claude needs your permission to use Bash' })]);
      status('waiting');
      vi.advanceTimersByTime(SETTLE_MS * 10);
      expect(sent).toEqual([]);
      status('working');
      status('waiting');
      vi.advanceTimersByTime(SETTLE_MS);
      expect(sent).toHaveLength(1);
      expect(sent[0]?.title).toBe('README.md changed');
    });

    it('keeps the turn open when the request arrives just after the status', () => {
      const { sent, status, timeline } = setup();
      status('waiting');
      status('working');
      status('waiting');
      vi.advanceTimersByTime(SETTLE_MS / 2);
      timeline([event({ kind: 'attention', message: 'Claude needs your permission to use Bash' })]);
      timeline(documentWritten('README.md'));
      vi.advanceTimersByTime(SETTLE_MS * 4);
      expect(sent).toEqual([]);
    });

    it('counts a document that arrives just after the agent waits', () => {
      const { sent, status, timeline } = setup();
      status('waiting');
      status('working');
      status('waiting');
      vi.advanceTimersByTime(1000);
      timeline(documentWritten('README.md'));
      vi.advanceTimersByTime(SETTLE_MS);
      expect(sent.map((n) => n.title)).toEqual(['README.md changed']);
    });

    it('gives one notification for several documents', () => {
      const { sent, status, turn } = setup();
      status('waiting');
      turn(['a.md', 'b.md', 'c.md', 'd.md'].flatMap((path) => documentWritten(path)));
      expect(sent).toEqual([{ kind: 'documents', title: 'a.md and 3 other documents changed', body: 'Claude Code changed them in this turn.' }]);
    });

    it('notifies when the agent exits, without saying it waits', () => {
      const { sent, status, timeline } = setup({ finished: true });
      status('waiting');
      status('working');
      timeline(documentWritten('README.md'));
      status('exited');
      expect(sent).toEqual([{ kind: 'documents', title: 'README.md changed', body: 'Claude Code changed it in this turn.' }]);
    });

    it('keeps the turn across a reconnect, with what was missed', () => {
      const { notices, sent, status, timeline } = setup();
      status('waiting');
      status('working');
      timeline(documentWritten('README.md'));
      // Reconnected: the host sends the status again and what the viewer missed.
      status('working');
      timeline(documentWritten('notes.md'));
      status('waiting');
      vi.advanceTimersByTime(SETTLE_MS);
      expect(sent.map((n) => n.title)).toEqual(['README.md and 1 other document changed']);
      notices.dispose();
    });

    it('starts over when the viewer joins again', () => {
      const { notices, sent, status, timeline } = setup();
      status('waiting');
      status('working');
      timeline(documentWritten('README.md'));
      notices.joined('Claude Code');
      status('working');
      status('waiting');
      vi.advanceTimersByTime(SETTLE_MS);
      expect(sent).toEqual([]);
    });

    it('never notifies when the activity is unknown', () => {
      const { sent, status, timeline } = setup({ finished: true });
      status('unknown');
      timeline(documentWritten('README.md'));
      status('unknown');
      vi.advanceTimersByTime(SETTLE_MS * 4);
      expect(sent).toEqual([]);
    });

    it('ends the turn when a question is dismissed while the agent waits', () => {
      const { sent, status, timeline } = setup({ finished: true });
      status('waiting');
      status('working');
      timeline([event({ kind: 'tool-call', promptId: 'p', toolUseId: 'q', toolName: 'AskUserQuestion', input: {} })]);
      status('waiting');
      vi.advanceTimersByTime(SETTLE_MS * 4);
      expect(sent).toEqual([]);
      timeline([event({ kind: 'tool-result', promptId: 'p', toolUseId: 'q', status: 'error' })]);
      vi.advanceTimersByTime(SETTLE_MS);
      expect(sent.map((n) => n.title)).toEqual(['Claude Code finished']);
    });
  });

  describe('what a turn says', () => {
    it('says a proposed change is ready for review', () => {
      const { sent, status, turn } = setup();
      status('waiting');
      turn(planWritten('add-login', SPEC_DRIVEN));
      expect(sent).toEqual([
        { kind: 'plans', title: 'Plan add-login is ready for review', body: 'Claude Code wrote proposal, specs, design and tasks.' },
      ]);
    });

    it('says a revised change was updated', () => {
      const { sent, status, timeline, turn } = setup();
      timeline([planState('add-login')]);
      status('waiting');
      turn(planWritten('add-login', ['design']));
      expect(sent).toEqual([{ kind: 'plans', title: 'Plan add-login was updated', body: 'Claude Code changed design.' }]);
    });

    it('stays quiet while tasks are ticked', () => {
      const { sent, status, timeline, turn } = setup();
      timeline([planState('add-login')]);
      status('waiting');
      turn([...planWritten('add-login', ['tasks'], { done: 1, total: 8 }), ...planWritten('add-login', ['tasks'], { done: 3, total: 8 })]);
      expect(sent).toEqual([]);
    });

    it('stays quiet about code', () => {
      const { sent, status, turn } = setup();
      status('waiting');
      turn([event({ kind: 'file-change', promptId: 'p', toolUseId: 't', path: 'server.ts', change: 'edited', hunks: [] })]);
      expect(sent).toEqual([]);
    });

    it('leaves out deleted documents', () => {
      const { sent, status, turn } = setup();
      status('waiting');
      turn([...documentWritten('old.md'), ...documentWritten('old.md', 'deleted')]);
      expect(sent).toEqual([]);
    });

    it('names a README next to a plan', () => {
      const { sent, status, turn } = setup();
      status('waiting');
      turn([...planWritten('add-login', SPEC_DRIVEN), ...documentWritten('README.md')]);
      expect(sent).toEqual([
        {
          kind: 'plans',
          title: 'Plan add-login is ready for review',
          body: 'Claude Code wrote proposal, specs, design and tasks. README.md changed too.',
        },
      ]);
    });

    it('lists a plan as documents when plans are off', () => {
      const { sent, status, turn } = setup({ plans: false });
      status('waiting');
      turn(planWritten('add-login', SPEC_DRIVEN));
      expect(sent).toEqual([{ kind: 'documents', title: 'proposal.md and 3 other documents changed', body: 'Claude Code changed them in this turn.' }]);
    });

    it('stays quiet when the kinds are off', () => {
      const { sent, status, turn } = setup({ documents: false });
      status('waiting');
      turn(documentWritten('README.md'));
      expect(sent).toEqual([]);
    });

    it('says the agent finished, when that is on', () => {
      const { sent, status, turn } = setup({ finished: true });
      status('waiting');
      turn([]);
      expect(sent).toEqual([{ kind: 'finished', title: 'Claude Code finished', body: "It waits for the host's next prompt." }]);
    });

    it('adds that the agent finished to a plan', () => {
      const { sent, status, turn } = setup({ finished: true });
      status('waiting');
      turn(planWritten('add-login', ['proposal']));
      expect(sent[0]?.body).toBe('Claude Code wrote proposal. Claude Code finished.');
    });

    it('notifies a plan-mode plan at once, and not again at the end', () => {
      const { sent, status, timeline } = setup({ finished: true });
      status('waiting');
      status('working');
      timeline([event({ kind: 'tool-call', promptId: 'p', toolUseId: 'plan', toolName: 'ExitPlanMode', input: { plan: '# Secret plan' } })]);
      expect(sent).toEqual([{ kind: 'plans', title: 'Claude Code has a plan for you to review', body: 'Open the session to read it.' }]);
      timeline([event({ kind: 'attention', message: 'Claude has a plan for you to review' })]);
      status('waiting');
      vi.advanceTimersByTime(SETTLE_MS * 4);
      timeline([event({ kind: 'tool-result', promptId: 'p', toolUseId: 'plan', status: 'ok' })]);
      status('working');
      status('waiting');
      vi.advanceTimersByTime(SETTLE_MS);
      expect(sent.map((n) => n.title)).toEqual(['Claude Code has a plan for you to review', 'Claude Code finished']);
    });

    it('never puts a document or plan text in a notification', () => {
      const { sent, status, turn } = setup({ finished: true });
      status('waiting');
      turn([...planWritten('add-login', SPEC_DRIVEN), ...documentWritten('README.md')]);
      expect(JSON.stringify(sent)).not.toContain('# text');
    });
  });
});
