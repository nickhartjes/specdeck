// What a viewer is notified about in a session they watch (`viewer-notifications` design
// decisions 2 and 3): at most one notification per turn of the agent, about the plans and
// documents it wrote and that it finished, and a plan-mode plan at once. Only turns the viewer
// saw start count, so the history sent on joining never notifies. Notifications name changes and
// files, never their text. Each says what a click on it opens: the thing its title names
// (`notification-open` design decision 1).

import type { AgentStatus, NoticeOpen, PlanDocument, TimelineEvent, ViewerNotificationKind, ViewerNotifications } from '../api/types';

/** What a viewer gets until they change it: plans, documents and the host's asks, not every turn's end or what others share. */
export const DEFAULT_VIEWER_NOTIFICATIONS: ViewerNotifications = {
  plans: true,
  documents: true,
  finished: false,
  reads: true,
  estimates: true,
  files: false,
  links: false,
  braindump: false,
};

/** The viewer's switches, in the order shown. The host's asks notify in `hostAskNotices.ts`, what others share in `sharedNotices.ts`. */
export const VIEWER_NOTIFICATION_KINDS: { kind: ViewerNotificationKind; label: string }[] = [
  { kind: 'plans', label: 'A plan is ready for review' },
  { kind: 'documents', label: 'A document changed' },
  { kind: 'finished', label: 'The agent finished' },
  { kind: 'reads', label: 'The host asks you to read' },
  { kind: 'estimates', label: 'The host asks for an estimate' },
  { kind: 'files', label: 'Someone shares a file' },
  { kind: 'links', label: 'The host shares a link' },
  { kind: 'braindump', label: 'Someone adds to the braindump' },
];

/** Every kind on: what the bell's list records, whatever pops up (`notification-bell` design decision 1). */
const ALL_KINDS_ON: ViewerNotifications = {
  plans: true,
  documents: true,
  finished: true,
  reads: true,
  estimates: true,
  files: true,
  links: true,
  braindump: true,
};

/** How long the agent waits before its turn counts as ended: status and timeline arrive on separate channels. */
export const SETTLE_MS = 1500;

/** Tools that wait for the host's answer, so the agent waiting on them has not finished. */
const ASKING_TOOLS = new Set(['ExitPlanMode', 'AskUserQuestion']);

export interface ViewerNotice {
  kind: ViewerNotificationKind;
  title: string;
  body: string;
  /** What a click opens; none when the notice only calls the viewer back to the session. */
  open?: NoticeOpen;
}

export interface ViewerNoticesOptions {
  /** The viewer's switches until `setSwitches` says otherwise. */
  switches: ViewerNotifications;
  notify: (notice: ViewerNotice) => void;
  /** Every notice for the bell's list, built with every kind on, whether it pops up or not. */
  record?: (notice: ViewerNotice) => void;
  /** Runs `run` after `ms`; returns a function that cancels it. */
  schedule?: (run: () => void, ms: number) => () => void;
}

export interface ViewerNotices {
  /** The viewer joined, or joined again: what comes next until the first status is history. */
  joined(agentName: string): void;
  status(status: AgentStatus): void;
  timeline(event: TimelineEvent): void;
  /** The viewer changed their switches; applies to the turn in progress. */
  setSwitches(switches: ViewerNotifications): void;
  /** Stops a pending end of turn, when the screen goes away. */
  dispose(): void;
}

interface PlanInTurn {
  /** Whether a plan state for this change arrived before the turn. */
  known: boolean;
  documents: PlanDocument[];
}

interface Turn {
  /** Documents written in the turn, each once, in order. */
  documents: string[];
  /** Paths whose latest change in the turn deleted them. */
  deleted: Set<string>;
  plans: Map<string, PlanInTurn>;
  /** A permission or other request for the host since the agent last worked. */
  attention: boolean;
  /** Questions and plans the host has not answered yet, by tool call. */
  asking: Set<string>;
  planModeShown: boolean;
}

function defaultSchedule(run: () => void, ms: number): () => void {
  const timer = setTimeout(run, ms);
  return () => clearTimeout(timer);
}

export function createViewerNotices({ switches: initial, notify, record = () => {}, schedule = defaultSchedule }: ViewerNoticesOptions): ViewerNotices {
  let switches = initial;
  let agentName = 'The agent';
  // No status since joining yet: the first one is where the viewer came in.
  let baseline = false;
  let status: AgentStatus = 'unknown';
  let turn: Turn | null = null;
  let cancelEnd: (() => void) | null = null;
  // Every change a plan state arrived for, history included.
  const knownPlans = new Set<string>();

  const stopEnd = () => {
    cancelEnd?.();
    cancelEnd = null;
  };

  const blocked = (current: Turn) => current.attention || current.asking.size > 0;

  const endTurn = () => {
    stopEnd();
    const ended = turn;
    turn = null;
    if (ended) {
      const exited = status === 'exited';
      const entry = turnNotice(ended, agentName, ALL_KINDS_ON, exited);
      if (entry) record(entry);
      const notice = turnNotice(ended, agentName, switches, exited);
      if (notice) notify(notice);
    }
  };

  const maybeEnd = () => {
    if (!turn || blocked(turn) || status !== 'waiting' || cancelEnd) return;
    cancelEnd = schedule(endTurn, SETTLE_MS);
  };

  return {
    joined(name) {
      stopEnd();
      agentName = name;
      baseline = false;
      turn = null;
    },

    status(next) {
      const previous = status;
      status = next;
      if (!baseline) {
        // A turn already running when the viewer came in is not theirs to be called in for.
        baseline = true;
        return;
      }
      if (next !== 'waiting') stopEnd();
      if (next === 'working') {
        if (turn) turn.attention = false;
        else if (previous !== 'working') turn = { documents: [], deleted: new Set(), plans: new Map(), attention: false, asking: new Set(), planModeShown: false };
      } else if (next === 'exited') {
        endTurn();
      } else if (next === 'waiting') {
        maybeEnd();
      }
    },

    timeline(event) {
      if (event.kind === 'plan') {
        if (turn) {
          const known = turn.plans.get(event.change)?.known ?? knownPlans.has(event.change);
          turn.plans.set(event.change, { known, documents: event.documents });
        }
        knownPlans.add(event.change);
        return;
      }
      if (!turn) return;
      switch (event.kind) {
        case 'attention':
          turn.attention = true;
          stopEnd();
          break;
        case 'tool-call':
          if (!ASKING_TOOLS.has(event.toolName)) break;
          turn.asking.add(event.toolUseId);
          stopEnd();
          if (event.toolName === 'ExitPlanMode' && !turn.planModeShown) {
            turn.planModeShown = true;
            const notice = planModeNotice(agentName, event.toolUseId);
            record(notice);
            if (switches.plans) notify(notice);
          }
          break;
        case 'tool-result':
          // A question dismissed while the agent already waits ends the turn as well.
          if (turn.asking.delete(event.toolUseId)) maybeEnd();
          break;
        case 'file-change':
          if (event.change === 'deleted') turn.deleted.add(event.path);
          else turn.deleted.delete(event.path);
          break;
        case 'document':
          if (!turn.documents.includes(event.path)) turn.documents.push(event.path);
          break;
      }
    },

    setSwitches(next) {
      switches = next;
    },

    dispose() {
      stopEnd();
    },
  };
}

function planModeNotice(agentName: string, toolUseId: string): ViewerNotice {
  return {
    kind: 'plans',
    title: `${agentName} has a plan for you to review`,
    body: 'Open the session to read it.',
    open: { kind: 'plan-mode', toolUseId },
  };
}

interface PlanOutcome {
  change: string;
  isNew: boolean;
  /** Names of the plan's documents written in the turn, in the schema's order. */
  written: string[];
  /** Paths of the plan's documents, to leave out of the other documents. */
  paths: Set<string>;
  /** The document a click opens: the first the turn wrote, in the schema's order, else the first. */
  opens: string | undefined;
}

/**
 * The notification for a turn that ended, or null when there is nothing the viewer wants to hear.
 * `exited` says the agent's process ended, so it does not wait for a prompt.
 */
function turnNotice(turn: Turn, agentName: string, switches: ViewerNotifications, exited = false): ViewerNotice | null {
  const documents = turn.documents.filter((path) => !turn.deleted.has(path));
  const changed = new Set(documents);

  const plans: PlanOutcome[] = [];
  // A plan's tasks file when ticking tasks was all the turn did to it: progress, not news.
  const progress = new Set<string>();
  for (const [change, plan] of turn.plans) {
    const written = plan.documents.filter((doc) => doc.paths.some((path) => changed.has(path)));
    if (plan.known && written.every((doc) => doc.id === 'tasks')) {
      written.forEach((doc) => doc.paths.forEach((path) => progress.add(path)));
      continue;
    }
    const opens = written[0]?.paths.find((path) => changed.has(path)) ?? plan.documents[0]?.paths[0];
    plans.push({ change, isNew: !plan.known, written: written.map((doc) => doc.name), paths: new Set(plan.documents.flatMap((doc) => doc.paths)), opens });
  }

  const shownPlans = switches.plans ? plans : [];
  const inPlans = new Set(shownPlans.flatMap((plan) => [...plan.paths]));
  const others = documents.filter((path) => !inPlans.has(path) && !progress.has(path));

  const lines: ViewerNotice[] = [];
  const [first, ...rest] = shownPlans;
  if (first) {
    const title = first.isNew ? `Plan ${first.change} is ready for review` : `Plan ${first.change} was updated`;
    const what = first.written.length ? `${agentName} ${first.isNew ? 'wrote' : 'changed'} ${list(first.written)}.` : '';
    const [second] = rest;
    const more = !second ? '' : rest.length === 1 ? `Plan ${second.change} changed too.` : `${rest.length} other plans changed too.`;
    lines.push({ kind: 'plans', title, body: [what, more].filter(Boolean).join(' '), ...opening(first.opens) });
  }
  if (switches.documents && others.length) {
    const [first, ...rest] = others.map(fileName);
    const title = rest.length ? `${first} and ${rest.length} other ${rest.length === 1 ? 'document' : 'documents'} changed` : `${first} changed`;
    lines.push({ kind: 'documents', title, body: `${agentName} changed ${rest.length ? 'them' : 'it'} in this turn.`, ...opening(others[0]) });
  }
  if (switches.finished && !exited) {
    lines.push({ kind: 'finished', title: `${agentName} finished`, body: "It waits for the host's next prompt." });
  }
  const [head, ...tail] = lines;
  if (!head) return null;
  // The first line is the notification; the others are named in its body.
  const body = [head.body, ...tail.map((line) => (line.kind === 'finished' ? `${line.title}.` : `${line.title} too.`))].filter(Boolean).join(' ');
  return { kind: head.kind, title: head.title, body, ...(head.open && { open: head.open }) };
}

/** A document for a notice to open, when there is one. */
function opening(path: string | undefined): Pick<ViewerNotice, 'open'> {
  return path ? { open: { kind: 'document', path } } : {};
}

function fileName(path: string): string {
  return path.split('/').at(-1) ?? path;
}

/** "a", "a and b", "a, b and c". */
function list(items: string[]): string {
  return items.length < 2 ? (items[0] ?? '') : `${items.slice(0, -1).join(', ')} and ${items.at(-1)}`;
}
