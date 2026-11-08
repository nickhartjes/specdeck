// Catching up after joining late (`catch-up` design): which prompts a late joiner sees folded,
// the outcome line of each, and the "so far" overview of what happened before they joined.

import type { TimelineEvent } from '../api/types';
import { buildDocuments, type Plan } from './documents';
import { planChangeOf } from './steps';
import type { Turn } from './timeline';

/** A prompt that ended this shortly before joining counts as seen, as clocks differ a little. */
export const SEEN_GRACE_MS = 60_000;

/** When turn `i` ended: its reply, else the start of the next prompt; null while it runs. */
export function turnEnd(turns: Turn[], i: number): number | null {
  return turns[i]?.respondedAt ?? turns[i + 1]?.at ?? null;
}

/**
 * Prompt ids of the turns that ended before the viewer joined. The latest prompt when they joined
 * never folds, also once a newer one starts, so a prompt they are reading stays open.
 */
export function missedTurns(turns: Turn[], joinedAt: number | null): Set<string> {
  const missed = new Set<string>();
  if (joinedAt === null) return missed;
  for (let i = 0; i < turns.length - 1; i++) {
    const end = turnEnd(turns, i);
    if (end !== null && end < joinedAt - SEEN_GRACE_MS && turns[i + 1]!.at <= joinedAt) missed.add(turns[i]!.promptId);
  }
  return missed;
}

/** How many events happened before the viewer joined; events come in order. */
export function eventsBefore(events: TimelineEvent[], joinedAt: number): number {
  let n = events.length;
  while (n > 0 && events[n - 1]!.at > joinedAt) n--;
  return n;
}

/** What a prompt did, for its outcome line. */
export interface TurnFacts {
  /** OpenSpec changes whose files the prompt changed, in order; `wrote` when no earlier prompt did. */
  plans: { change: string; wrote: boolean }[];
  /** Changed files outside any plan. */
  files: number;
  /** The plan whose tasks done changed during the prompt, at its last count. */
  tasks: { done: number; total: number } | null;
  /** Calls the host denied. */
  denied: number;
  /** The prompt's last call failed. */
  endedFailed: boolean;
}

/** The facts of every prompt in `events`, by prompt id. */
export function turnFacts(events: TimelineEvent[]): Map<string, TurnFacts> {
  const facts = new Map<string, TurnFacts>();
  const files = new Map<string, Set<string>>();
  const lastCall = new Map<string, string>();
  const statuses = new Map<string, string>();
  const seenPlans = new Set<string>();
  const tasksDone = new Map<string, number>();
  let current: TurnFacts | null = null;
  const of = (promptId: string): TurnFacts => {
    let f = facts.get(promptId);
    if (!f) {
      f = { plans: [], files: 0, tasks: null, denied: 0, endedFailed: false };
      facts.set(promptId, f);
      files.set(promptId, new Set());
    }
    return f;
  };
  for (const event of events) {
    switch (event.kind) {
      case 'prompt':
        current = of(event.promptId);
        break;
      case 'tool-call':
        of(event.promptId);
        lastCall.set(event.promptId, event.toolUseId);
        break;
      case 'tool-result': {
        const f = of(event.promptId);
        statuses.set(event.toolUseId, event.status);
        if (event.permission === 'denied') f.denied++;
        break;
      }
      case 'file-change': {
        const f = of(event.promptId);
        const plan = planChangeOf(event.path);
        if (plan === null) files.get(event.promptId)!.add(event.path);
        else if (!f.plans.some((p) => p.change === plan)) {
          f.plans.push({ change: plan, wrote: !seenPlans.has(plan) });
          seenPlans.add(plan);
        }
        break;
      }
      case 'plan': {
        // Plan states name no prompt: they belong to the prompt in progress.
        const before = tasksDone.get(event.change) ?? 0;
        tasksDone.set(event.change, event.tasks.done);
        if (current && event.tasks.total > 0 && event.tasks.done !== before) current.tasks = { ...event.tasks };
        break;
      }
      default:
        break;
    }
  }
  for (const [promptId, f] of facts) {
    f.files = files.get(promptId)!.size;
    const last = lastCall.get(promptId);
    f.endedFailed = last !== undefined && statuses.get(last) === 'failed';
  }
  return facts;
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
const MAX_SENTENCE = 160;

/** The first sentence of a reply, as plain text, cut to one line. */
export function firstSentence(markdown: string): string | null {
  const plain = markdown
    .replace(/```[\s\S]*?(```|$)/g, ' ')
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}(#{1,6}\s+|>\s?|[-*+]\s+|\d+[.)]\s+)/gm, '')
    .replace(/[`*_~]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
  if (!plain) return null;
  const sentence = /^.+?[.!?](?=\s|$)/.exec(plain)?.[0] ?? plain;
  return sentence.length > MAX_SENTENCE ? `${sentence.slice(0, MAX_SENTENCE - 1).trimEnd()}…` : sentence;
}

/** "Wrote the plan: viewer-heartbeat · 4 files changed · 3 of 6 tasks done", else the reply's first sentence. */
export function outcomeOf(turn: Turn, facts: TurnFacts | undefined): string | null {
  const parts: string[] = [];
  if (facts) {
    const [plan, ...more] = facts.plans;
    if (plan) parts.push(`${plan.wrote ? 'Wrote' : 'Updated'} the plan: ${plan.change}${more.length ? ` and ${plural(more.length, 'more plan')}` : ''}`);
    if (facts.files) parts.push(`${plural(facts.files, 'file')} changed`);
    if (facts.tasks) parts.push(`${facts.tasks.done} of ${facts.tasks.total} tasks done`);
    if (facts.denied) parts.push(facts.denied === 1 ? 'the host denied a step' : `the host denied ${facts.denied} steps`);
    if (facts.endedFailed) parts.push('ended on a failed step');
  }
  if (parts.length) {
    const line = parts.join(' · ');
    return line.charAt(0).toUpperCase() + line.slice(1);
  }
  return turn.response ? firstSentence(turn.response) : null;
}

/** What happened before the viewer joined. */
export interface SoFar {
  /** How long the session had run when the viewer joined. */
  ranMs: number;
  prompts: number;
  /** Each plan as it was at the join, the most recently changed first. */
  plans: Plan[];
  /** Documents written, in the order they were first written. */
  documents: string[];
  /** Files shared, by id, in order. */
  shared: { file: number; name: string }[];
  /** Links shared, by the host's id, in order. */
  links: number[];
  /** Changed files that are not documents. */
  codeFiles: number;
}

/** The overview of `events`, all of which happened before the viewer joined at `joinedAt`. */
export function soFar(events: TimelineEvent[], joinedAt: number): SoFar {
  const { documents, plans } = buildDocuments(events);
  const prompts = new Set<string>();
  const changed = new Set<string>();
  const shared: SoFar['shared'] = [];
  const links: number[] = [];
  for (const event of events) {
    if (event.kind === 'prompt') prompts.add(event.promptId);
    else if (event.kind === 'file-change') changed.add(event.path);
    else if (event.kind === 'file-shared') shared.push({ file: event.file, name: event.name });
    else if (event.kind === 'link') links.push(event.id);
  }
  const codeFiles = [...changed].filter((path) => !documents.has(path)).length;
  return { ranMs: Math.max(0, joinedAt - (events[0]?.at ?? joinedAt)), prompts: prompts.size, plans, documents: [...documents.keys()], shared, links, codeFiles };
}

/** "40 minutes", "1 hour 5 minutes"; under a minute, null. */
export function ranFor(ms: number): string | null {
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 1) return null;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (!hours) return plural(minutes, 'minute');
  return rest ? `${plural(hours, 'hour')} ${plural(rest, 'minute')}` : plural(hours, 'hour');
}
