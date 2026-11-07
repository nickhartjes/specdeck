import { memo, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { AlertTriangle, Ban, Check, CircleSlash, Hand, History, Loader, X } from 'lucide-react';
import type { TimelineModel, ToolItem, Turn } from '../model/timeline';
import { SHOWN_IN_PART, toolSummary } from '../model/timeline';
import { liveLine, summarize, type Step, type SummaryEntry } from '../model/steps';
import type { Steps } from '../api/types';
import { DiffView } from './DiffView';
import { formatTokens } from '../model/statusbar';
import { Markdown } from './Markdown';
import { asksTheHost, interactionBody, PermissionBadge } from './HostInteractions';

interface Props {
  model: TimelineModel;
  timelineSupported: boolean;
  agentName: string;
  /** Steps in plain words, or every tool call in detail. */
  steps?: Steps;
  /** The terminal is hidden, so the panel says what the agent does right now. */
  live?: boolean;
  /** Opens the reader on the files of an OpenSpec change that a step wrote or changed. */
  onReadPlan?: (change: string, paths: string[]) => void;
  /** A terminal session, which never has a timeline. */
  terminal?: boolean;
  /** Turns matching the current search, by prompt id. */
  matches: Set<string>;
  /** The turn the search stepped to. */
  current: string | null;
  /** Scrolls the terminal to where a prompt happened, if it is shown. */
  onJump: (outputSeq: number) => void;
  /** Shows the terminal at a step, with its line highlighted. */
  onStep?: (outputSeq: number) => void;
  /** Opens the diffs of a file a step changed, at that step's change. */
  onOpenChange?: (path: string, toolUseId: string) => void;
}

/** What selecting a step does: its first changed file in a tab, else the terminal at its line. */
type Select = (item: ToolItem) => void;

const TIME = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });

export function TimelinePanel({ model, timelineSupported, agentName, steps = 'details', live = false, onReadPlan, terminal = false, matches, current, onJump, onStep, onOpenChange }: Props) {
  const select = useCallback<Select>(
    (item) => {
      const change = item.changes[0];
      if (change && onOpenChange) onOpenChange(change.path, item.id);
      else (onStep ?? onJump)(item.outputSeq);
    },
    [onJump, onStep, onOpenChange],
  );
  // Summaries per turn, in order: whether a plan was written or updated depends on earlier turns.
  const summaries = useMemo(() => {
    const seen = new Set<string>();
    return model.turns.map((turn) => summarize(turn, seen));
  }, [model]);
  // What a turn shows changes with its own events, and in summary mode also with earlier turns' plans.
  const revisions = useMemo(() => {
    const result: number[] = [];
    for (const turn of model.turns) result.push(turn.revision + (steps === 'summary' ? (result.at(-1) ?? 0) : 0));
    return result;
  }, [model, steps]);
  const lastTurn = model.turns.at(-1);
  const plansBeforeLast = useMemo(() => {
    const seen = new Set<string>();
    model.turns.slice(0, -1).forEach((turn) => summarize(turn, seen));
    return seen;
  }, [model]);
  const list = useRef<HTMLDivElement>(null);
  const atBottom = useRef(true);

  // Follow new entries while the reader is at the end of the list.
  useEffect(() => {
    const element = list.current?.parentElement;
    if (!element) return;
    const onScroll = () => {
      atBottom.current = element.scrollHeight - element.scrollTop - element.clientHeight < 40;
    };
    element.addEventListener('scroll', onScroll);
    return () => element.removeEventListener('scroll', onScroll);
  }, []);
  useEffect(() => {
    const element = list.current?.parentElement;
    if (element && atBottom.current) element.scrollTo({ top: element.scrollHeight });
  }, [model]);

  useEffect(() => {
    if (current) document.getElementById(`turn-${current}`)?.scrollIntoView({ block: 'center' });
  }, [current]);

  if (!timelineSupported && terminal) {
    return (
      <div className="empty">
        <History aria-hidden />
        <strong>A terminal session has no timeline</strong>
        <span>The terminal shows everything.</span>
      </div>
    );
  }
  if (!timelineSupported) {
    return (
      <div className="empty">
        <History aria-hidden />
        <strong>No timeline for {agentName}</strong>
        <span>
          SpecDeck builds the timeline of prompts, tool calls and file changes from Claude Code's events. For {agentName}, the
          terminal shows everything.
        </span>
      </div>
    );
  }

  const jumpTo = (promptId: string) => {
    const turn = model.turns.find((t) => t.promptId === promptId);
    if (!turn) return;
    document.getElementById(`turn-${promptId}`)?.scrollIntoView({ block: 'start' });
    onJump(turn.outputSeq);
  };

  return (
    <div ref={list}>
      {model.turns.length > 1 && (
        <div className="prompt-list">
          <label htmlFor="jump-prompt" className="field-hint">
            Jump to
          </label>
          <select id="jump-prompt" value="" onChange={(event) => jumpTo(event.target.value)}>
            <option value="" disabled>
              Prompt…
            </option>
            {model.turns.map((turn, i) => (
              <option key={turn.promptId} value={turn.promptId}>
                {i + 1}. {turn.prompt.slice(0, 60) || 'Earlier prompt'}
              </option>
            ))}
          </select>
        </div>
      )}
      {model.unavailable && (
        <div className="banner warning" role="status">
          <AlertTriangle aria-hidden />
          <span>Timeline paused: {model.unavailable} The terminal keeps working.</span>
        </div>
      )}
      {model.turns.length === 0 ? (
        <div className="empty">
          <History aria-hidden />
          <strong>No prompts yet</strong>
          <span>Prompts, tool calls and file changes appear here as they happen.</span>
        </div>
      ) : (
        <div className="timeline">
          {model.turns.map((turn, i) => (
            <TurnView
              key={turn.promptId}
              turn={turn}
              summary={steps === 'summary' ? summaries[i]! : null}
              revision={revisions[i]!}
              latest={turn === lastTurn}
              onReadPlan={onReadPlan}
              number={i + 1}
              match={matches.has(turn.promptId)}
              current={current === turn.promptId}
              onJump={onJump}
              onSelect={select}
            />
          ))}
          {live && lastTurn && <LiveLine turn={lastTurn} plansBefore={plansBeforeLast} agentName={agentName} />}
        </div>
      )}
    </div>
  );
}

/** What the agent does right now and for how long, for viewers without the terminal. */
function LiveLine({ turn, plansBefore, agentName }: { turn: Turn; plansBefore: Set<string>; agentName: string }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  const name = agentName === 'Claude Code' ? 'Claude' : agentName;
  const text = liveLine(turn, plansBefore, name);
  if (!text) return null;
  const running = turn.items.findLast((item): item is ToolItem => item.kind === 'tool' && item.status === 'running');
  const waiting = turn.items.at(-1)?.kind === 'notice';
  const seconds = running && !waiting ? Math.max(0, Math.round((now - running.at) / 1000)) : null;
  return (
    <p className={`live-line${waiting ? ' waiting' : ''}`} role="status" aria-label={`What ${agentName} is doing`}>
      <span className="dot" aria-hidden />
      {text}
      {seconds !== null && ` · ${seconds} s`}
    </p>
  );
}

const OUTCOME_ICON = { running: Loader, succeeded: Check, failed: X, 'not-run': CircleSlash, denied: Ban } as const;

/** One summary line; opens to the tool calls it stands for. A plan step also offers to read it. */
function StepView({ step, onSelect, onReadPlan }: { step: Step; onSelect: Select; onReadPlan?: (change: string, paths: string[]) => void }) {
  const Icon = OUTCOME_ICON[step.outcome];
  const plan = step.plan;
  const details = (
    <Disclosure
      className={`step ${step.outcome}`}
      open={step.tools.some(asksTheHost)}
      summary={
        <summary onClick={() => onSelect(step.tools.find((item) => item.changes.length > 0) ?? step.tools[0]!)}>
          <Icon className="status-icon" aria-hidden />
          <span className="what">{step.label}</span>
          {step.outcome !== 'denied' && step.tools.some((item) => item.permission === 'allowed') && <PermissionBadge permission="allowed" />}
          <span className="state">{STATUS_TEXT[step.outcome]}</span>
        </summary>
      }
    >
      {() => (
        <div className="step-body">
          {step.tools.map((item) => (
            <ToolView key={item.id} item={item} showChanges onSelect={onSelect} />
          ))}
        </div>
      )}
    </Disclosure>
  );
  if (!plan || !onReadPlan) return details;
  const paths = [...new Set(step.tools.flatMap((item) => item.changes.map((change) => change.path)))];
  return (
    <div className="step-row">
      {details}
      <button className="btn small" onClick={() => onReadPlan(plan, paths)}>
        {step.label === 'Wrote the plan' ? 'Read the plan' : 'Read the changes'}
      </button>
    </div>
  );
}

interface TurnProps {
  turn: Turn;
  summary: SummaryEntry[] | null;
  /** Changes whenever what the turn shows changes; see `revisions`. */
  revision: number;
  /** The newest turn, which shows its file changes opened. */
  latest: boolean;
  onReadPlan?: (change: string, paths: string[]) => void;
  number: number;
  match: boolean;
  current: boolean;
  onJump: (seq: number) => void;
  onSelect: Select;
}

// Long sessions have hundreds of turns: render only the ones that changed. The model is rebuilt
// on every event, so a turn is compared by its revision instead of by identity.
const TurnView = memo(
  TurnBody,
  (prev, next) =>
    prev.turn.promptId === next.turn.promptId &&
    (prev.summary === null) === (next.summary === null) &&
    (Object.keys(next) as (keyof TurnProps)[]).every((key) => key === 'turn' || key === 'summary' || prev[key] === next[key]),
);

function TurnBody({ turn, summary, latest, onReadPlan, number, match, current, onJump, onSelect }: TurnProps) {
  const texts = turn.items.filter((item) => item.kind === 'text');
  const showResponse = turn.response && !texts.some((item) => item.text.trim() === turn.response?.trim());
  return (
    <article id={`turn-${turn.promptId}`} className={`turn${match ? ' match' : ''}${current ? ' current' : ''}`} aria-label={`Prompt ${number}`}>
      <button className="prompt" onClick={() => onJump(turn.outputSeq)} title="Show in the terminal">
        <span className="meta">
          <span>Prompt {number}</span>
          <span>
            {turn.tokens ? <span title={`${turn.tokens.toLocaleString()} tokens used for this prompt`}>{formatTokens(turn.tokens)} tokens · </span> : null}
            {turn.at ? TIME.format(turn.at) : ''}
          </span>
        </span>
        <span className="text selectable">{turn.prompt || 'Earlier prompt'}</span>
      </button>
      {(summary ?? turn.items).map((item) =>
        item.kind === 'step' ? (
          <StepView key={item.id} step={item} onSelect={onSelect} onReadPlan={onReadPlan} />
        ) : item.kind === 'text' ? (
          <Markdown key={item.id} text={item.text} />
        ) : item.kind === 'notice' ? (
          <p key={item.id} className="notice">
            <Hand aria-hidden /> {item.text}
          </p>
        ) : (
          <ToolView key={item.id} item={item} showChanges={latest} onSelect={onSelect} />
        ),
      )}
      {showResponse && <Markdown text={turn.response ?? ''} />}
    </article>
  );
}

const STATUS_TEXT = { running: 'Running', succeeded: 'Done', failed: 'Failed', 'not-run': 'Did not run', denied: 'Denied by the host' } as const;

/** `showChanges`: opens a call that changed files, which costs a diff each, so only where they are wanted. */
function ToolView({ item, showChanges, onSelect }: { item: ToolItem; showChanges: boolean; onSelect: Select }) {
  const Icon = { running: Loader, succeeded: Check, failed: X, 'not-run': CircleSlash }[item.status];
  return (
    <Disclosure
      className={`tool ${item.status}`}
      open={(showChanges && item.changes.length > 0) || asksTheHost(item)}
      summary={
        <summary onClick={() => onSelect(item)}>
          <Icon className="status-icon" aria-hidden />
          <span className="name">{item.toolName}</span>
          <span className="what" title={toolSummary(item)}>
            {toolSummary(item)}
          </span>
          <PermissionBadge permission={item.permission} />
          <span className="state">{STATUS_TEXT[item.status]}</span>
        </summary>
      }
    >
      {() => (
        <div className="tool-body">
          {item.error && <div className="error-text selectable">{item.error}</div>}
          {item.truncated && <p className="hint">{SHOWN_IN_PART}</p>}
          {item.changes.length > 0
            ? item.changes.map((change, i) => <DiffView key={i} change={change} />)
            : (interactionBody(item) ?? <pre className="tool-input selectable">{JSON.stringify(item.input, null, 2)}</pre>)}
        </div>
      )}
    </Disclosure>
  );
}

/**
 * A `<details>` whose body is rendered only while it is open, so closed entries of a long session
 * cost nothing. `open` sets how it starts, and opens or closes it again whenever it changes.
 */
function Disclosure({ open: wanted, className, summary, children }: { open: boolean; className: string; summary: ReactNode; children: () => ReactNode }) {
  const [open, setOpen] = useState(wanted);
  const [lastWanted, setLastWanted] = useState(wanted);
  if (wanted !== lastWanted) {
    setLastWanted(wanted);
    setOpen(wanted);
  }
  return (
    <details className={className} open={open} onToggle={(event) => setOpen(event.currentTarget.open)}>
      {summary}
      {open && children()}
    </details>
  );
}

