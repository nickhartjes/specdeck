import { memo, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { AlertTriangle, Ban, Check, CircleSlash, Hand, History, Loader, Send, Share2, X } from 'lucide-react';
import { itemsFrom, type GiveStep, type ShareStep } from '../model/sharedFiles';
import type { AskItem, LinkItem, TimelineModel, ToolItem, Turn } from '../model/timeline';
import { SHOWN_IN_PART, toolSummary } from '../model/timeline';
import { elementHints, liveLine, named, screenshotOf, summarize, typedTextHidden, type Step, type SummaryEntry } from '../model/steps';
import type { Steps, TimelineEvent } from '../api/types';
import { eventsBefore, missedTurns, outcomeOf, soFar, turnFacts, type SoFar } from '../model/catchUp';
import { DiffView } from './DiffView';
import { formatTokens } from '../model/statusbar';
import { Markdown } from './Markdown';
import { asksTheHost, interactionBody, PermissionBadge } from './HostInteractions';
import { ScreenshotThumbnail } from './Screenshot';
import { LinkCard } from './LinkCard';
import { AskCard } from './AskCard';
import { CatchUpCard } from './CatchUpCard';
import { PromptPicker } from './PromptPicker';

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
  /** Opens a call's screenshot full size in a tab; kept rather than a preview with `keep`. */
  onOpenScreenshot?: OpenScreenshot;
  /** Files people shared, as steps among the prompts (`session-timeline` spec: Shared files in the timeline). */
  shares?: ShareStep[];
  /** The ids of files still shared; a step for any other is marked removed. */
  sharedNow?: Set<number>;
  onOpenShared?: (file: number, name: string) => void;
  /** The givings of the braindump, as steps among the prompts (`braindump`). */
  gives?: GiveStep[];
  /** Above the timeline: sharing links, and the links waiting on the host (`shared-links`). */
  linksBar?: ReactNode;
  /** The host's: removes a shared link. */
  onRemoveLink?: (linkId: number) => void;
  /** A viewer's: when they joined, which folds the prompts they missed (`catch-up`). */
  joinedAt?: number | null;
  /** The events the model was built from, for the overview of what happened before joining. */
  events?: TimelineEvent[];
  /** Opens a document the agent wrote. */
  onOpenDocument?: (path: string) => void;
}

/** What happened before the viewer joined, worked out once the events before the join are in. */
interface Before {
  count: number;
  joinedAt: number;
  soFar: SoFar;
  /** The outcome line of each prompt before the join, by prompt id. */
  outcomes: Map<string, string | null>;
}

/** What selecting a step does: its first changed file in a tab, else the terminal at its line. */
type Select = (item: ToolItem) => void;
type OpenScreenshot = (item: ToolItem, keep: boolean) => void;

const TIME = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });

export function TimelinePanel({ model, timelineSupported, agentName, steps = 'details', live = false, onReadPlan, terminal = false, matches, current, onJump, onStep, onOpenChange, onOpenScreenshot, shares = [], sharedNow, onOpenShared, gives = [], linksBar, onRemoveLink, joinedAt = null, events, onOpenDocument }: Props) {
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

  // Prompts the viewer missed fold until opened; search opens the one it lands on.
  const missed = useMemo(() => missedTurns(model.turns, joinedAt), [model, joinedAt]);
  const [unfolded, setUnfolded] = useState<ReadonlySet<string>>(new Set());
  const unfold = useCallback((promptId: string) => setUnfolded((open) => new Set(open).add(promptId)), []);
  if (current && missed.has(current) && !unfolded.has(current)) setUnfolded(new Set(unfolded).add(current));
  const [closed, setClosed] = useState(false);
  // Worked out again only while events from before the join arrive; later events leave it alone.
  const [before, setBefore] = useState<Before | null>(null);
  const count = joinedAt !== null && events ? eventsBefore(events, joinedAt) : 0;
  if (joinedAt !== null && events && missed.size > 0 && (before?.count !== count || before.joinedAt !== joinedAt)) {
    const earlier = events.slice(0, count);
    const facts = turnFacts(earlier);
    const outcomes = new Map(model.turns.filter((turn) => facts.has(turn.promptId)).map((turn) => [turn.promptId, outcomeOf(turn, facts.get(turn.promptId))]));
    setBefore({ count, joinedAt, soFar: soFar(earlier, joinedAt), outcomes });
  }

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

  const linkCards = (links: LinkItem[]) =>
    links.map((link) => <LinkCard key={link.id} link={link} onRemove={onRemoveLink} />);
  const askCards = (asks: AskItem[]) => asks.map((ask) => <AskCard key={ask.id} item={ask} />);

  // Without a timeline the note stays, and the session's links, read requests and estimates
  // are listed below it.
  if (!timelineSupported) {
    return (
      <div ref={list}>
        {terminal ? (
          <div className="empty">
            <History aria-hidden />
            <strong>A terminal session has no timeline</strong>
            <span>The terminal shows everything.</span>
          </div>
        ) : (
          <div className="empty">
            <History aria-hidden />
            <strong>No timeline for {agentName}</strong>
            <span>
              SpecDeck builds the timeline of prompts, tool calls and file changes from Claude Code's events. For {agentName}, the
              terminal shows everything.
            </span>
          </div>
        )}
        {linksBar}
        {model.links.length > 0 && (
          <section aria-label="Shared links" className="links-early">
            {linkCards(model.links)}
          </section>
        )}
        {model.asks.length > 0 && (
          <section aria-label="Read requests and estimates" className="links-early">
            {askCards(model.asks)}
          </section>
        )}
        {model.suggested.length > 0 && (
          <section aria-label="Sent suggestions" className="links-early">
            {model.suggested.map((item) => (
              <p key={item.id} className="field-hint">
                {TIME.format(item.at)} · Sent a prompt {suggestedBy(item)}
              </p>
            ))}
          </section>
        )}
      </div>
    );
  }

  const jumpTo = (promptId: string) => {
    const turn = model.turns.find((t) => t.promptId === promptId);
    if (!turn) return;
    document.getElementById(`turn-${promptId}`)?.scrollIntoView({ block: 'start' });
    onJump(turn.outputSeq);
  };
  // The terminal to the latest prompt, the timeline to its end, which then follows new entries again.
  const toLatest = () => {
    const last = model.turns.at(-1);
    if (last) onJump(last.outputSeq);
    const element = list.current?.parentElement;
    atBottom.current = true;
    element?.scrollTo({ top: element.scrollHeight });
  };

  return (
    <div ref={list}>
      {before && missed.size > 0 && !closed && (
        <CatchUpCard
          soFar={before.soFar}
          turns={model.turns}
          sharedNow={sharedNow}
          links={model.links}
          onJump={jumpTo}
          onLatest={toLatest}
          onClose={() => setClosed(true)}
          onReadPlan={onReadPlan}
          onOpenDocument={onOpenDocument}
          onOpenShared={onOpenShared}
        />
      )}
      {model.turns.length > 1 && (
        <div className="prompt-list">
          <PromptPicker id="jump-prompt" label="Jump to" turns={model.turns} onPick={jumpTo} />
        </div>
      )}
      {linksBar}
      {model.earlyLinks.length > 0 && (
        <section aria-label="Links shared before the first prompt" className="links-early">
          {linkCards(model.earlyLinks)}
        </section>
      )}
      {model.earlyAsks.length > 0 && (
        <section aria-label="Read requests and estimates before the first prompt" className="links-early">
          {askCards(model.earlyAsks)}
        </section>
      )}
      {model.unavailable && (
        <div className="banner warning" role="status">
          <AlertTriangle aria-hidden />
          <span>Timeline paused: {model.unavailable} The terminal keeps working.</span>
        </div>
      )}
      {model.turns.length === 0 && shares.length === 0 && gives.length === 0 ? (
        <div className="empty">
          <History aria-hidden />
          <strong>No prompts yet</strong>
          <span>Prompts, tool calls and file changes appear here as they happen.</span>
        </div>
      ) : (
        <div className="timeline">
          {model.turns.map((turn, i) => [
            ...stepsBefore(shares, model.turns, i).map((share) => (
              <ShareView key={`share-${share.file}`} share={share} removed={!sharedNow?.has(share.file)} onOpen={onOpenShared} />
            )),
            ...stepsBefore(gives, model.turns, i).map((give) => <GiveView key={`give-${give.round}`} give={give} agentName={agentName} onOpen={onOpenShared} />),
            <TurnView
              key={turn.promptId}
              turn={turn}
              folded={missed.has(turn.promptId) && !unfolded.has(turn.promptId)}
              outcome={before?.outcomes.get(turn.promptId) ?? null}
              hidden={steps === 'summary' ? summaries[i]!.filter((entry) => entry.kind === 'step').length : turn.items.filter((item) => item.kind === 'tool').length}
              onUnfold={unfold}
              summary={steps === 'summary' ? summaries[i]! : null}
              revision={revisions[i]!}
              latest={turn === lastTurn}
              onReadPlan={onReadPlan}
              number={i + 1}
              match={matches.has(turn.promptId)}
              current={current === turn.promptId}
              onJump={onJump}
              onSelect={select}
              onOpenScreenshot={onOpenScreenshot}
              onRemoveLink={onRemoveLink}
            />,
          ])}
          {stepsBefore(shares, model.turns, model.turns.length).map((share) => (
            <ShareView key={`share-${share.file}`} share={share} removed={!sharedNow?.has(share.file)} onOpen={onOpenShared} />
          ))}
          {stepsBefore(gives, model.turns, model.turns.length).map((give) => (
            <GiveView key={`give-${give.round}`} give={give} agentName={agentName} onOpen={onOpenShared} />
          ))}
          {live && lastTurn && <LiveLine turn={lastTurn} plansBefore={plansBeforeLast} agentName={agentName} />}
        </div>
      )}
    </div>
  );
}

/** The steps that happened before turn `index` started and after the one before it; all later ones for the end. */
function stepsBefore<T extends { at: number }>(steps: T[], turns: Turn[], index: number): T[] {
  const from = index === 0 ? -Infinity : turns[index - 1]!.at;
  const to = index < turns.length ? turns[index]!.at : Infinity;
  return steps.filter((step) => step.at >= from && step.at < to);
}

/** "Gave the braindump to Claude: 3 items from Sanne and Mo", which lists the items to open. */
function GiveView({ give, agentName, onOpen }: { give: GiveStep; agentName: string; onOpen?: (file: number, name: string) => void }) {
  return (
    <details className="share-step give-step">
      <summary>
        <Send aria-hidden />
        <span className="time">{TIME.format(give.at)}</span>
        <span>
          Gave the braindump to {agentName}: {itemsFrom(give.items.length, give.from)}
        </span>
      </summary>
      <ul>
        {give.items.map((item) => (
          <li key={item.file}>
            {item.removed ? (
              <span>
                {item.path} <span className="muted">(removed)</span>
              </span>
            ) : (
              <button className="link" onClick={() => onOpen?.(item.file, item.name)}>
                {item.path}
              </button>
            )}
          </li>
        ))}
      </ul>
    </details>
  );
}

/** "Sanne shared wireframe.png", which opens the file, or says it was removed. */
function ShareView({ share, removed, onOpen }: { share: ShareStep; removed: boolean; onOpen?: (file: number, name: string) => void }) {
  const who = share.byLabel || (share.by === 0 ? 'The host' : 'A viewer');
  return (
    <div className="share-step">
      <Share2 aria-hidden />
      <span className="time">{TIME.format(share.at)}</span>
      {removed ? (
        <span>
          {who} shared {share.name} <span className="muted">(removed)</span>
        </span>
      ) : (
        <button className="link" onClick={() => onOpen?.(share.file, share.name)}>
          {who} shared {share.name}
        </button>
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
function StepView({ step, onSelect, onReadPlan, onOpenScreenshot }: { step: Step; onSelect: Select; onReadPlan?: (change: string, paths: string[]) => void; onOpenScreenshot?: OpenScreenshot }) {
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
            <ToolView key={item.id} item={item} showChanges onSelect={onSelect} onOpenScreenshot={onOpenScreenshot} />
          ))}
        </div>
      )}
    </Disclosure>
  );
  const shot = step.screenshot;
  const screenshot = shot && screenshotOf(shot);
  if (shot && screenshot) {
    return (
      <div className="with-screenshot">
        {details}
        <ScreenshotThumbnail screenshot={screenshot} onOpen={onOpenScreenshot && ((keep) => onOpenScreenshot(shot, keep))} />
      </div>
    );
  }
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

/** "suggested by Sanne", and that the host edited it. */
function suggestedBy({ by, edited }: { by: string; edited: boolean }): string {
  return `suggested by ${by}${edited ? ', edited' : ''}`;
}

interface TurnProps {
  turn: Turn;
  /** Missed by a late joiner: only the prompt, its outcome and a way to unfold it. */
  folded: boolean;
  outcome: string | null;
  /** How many steps unfolding shows. */
  hidden: number;
  onUnfold: (promptId: string) => void;
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
  onOpenScreenshot?: OpenScreenshot;
  onRemoveLink?: (linkId: number) => void;
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

function TurnBody({ turn, folded, outcome, hidden, onUnfold, summary, latest, onReadPlan, number, match, current, onJump, onSelect, onOpenScreenshot, onRemoveLink }: TurnProps) {
  const prompt = (
    <button className="prompt" onClick={() => onJump(turn.outputSeq)} title="Show in the terminal">
      <span className="meta">
        <span>
          Prompt {number}
          {turn.suggested && ` · ${suggestedBy(turn.suggested)}`}
        </span>
        <span>
          {turn.tokens ? <span title={`${turn.tokens.toLocaleString()} tokens used for this prompt`}>{formatTokens(turn.tokens)} tokens · </span> : null}
          {turn.at ? TIME.format(turn.at) : ''}
        </span>
      </span>
      <span className="text selectable">{turn.prompt || 'Earlier prompt'}</span>
    </button>
  );
  if (folded) {
    return (
      <article id={`turn-${turn.promptId}`} className={`turn folded${match ? ' match' : ''}`} aria-label={`Prompt ${number}`}>
        {prompt}
        {outcome && (
          <p className="outcome">
            <Check aria-hidden /> {outcome}
          </p>
        )}
        {hidden > 0 && (
          <button className="link unfold" onClick={() => onUnfold(turn.promptId)}>
            Show the {hidden === 1 ? 'step' : `${hidden} steps`}
          </button>
        )}
      </article>
    );
  }
  const texts = turn.items.filter((item) => item.kind === 'text');
  const showResponse = turn.response && !texts.some((item) => item.text.trim() === turn.response?.trim());
  return (
    <article id={`turn-${turn.promptId}`} className={`turn${match ? ' match' : ''}${current ? ' current' : ''}`} aria-label={`Prompt ${number}`}>
      {prompt}
      {(summary ?? turn.items).map((item) =>
        item.kind === 'step' ? (
          <StepView key={item.id} step={item} onSelect={onSelect} onReadPlan={onReadPlan} onOpenScreenshot={onOpenScreenshot} />
        ) : item.kind === 'text' ? (
          <Markdown key={item.id} text={item.text} />
        ) : item.kind === 'notice' ? (
          <p key={item.id} className="notice">
            <Hand aria-hidden /> {item.text}
          </p>
        ) : item.kind === 'link' ? (
          <LinkCard key={item.id} link={item} onRemove={onRemoveLink} />
        ) : item.kind === 'ask' ? (
          <AskCard key={item.id} item={item} />
        ) : (
          <ToolView key={item.id} item={item} showChanges={latest} onSelect={onSelect} onOpenScreenshot={onOpenScreenshot} />
        ),
      )}
      {showResponse && <Markdown text={turn.response ?? ''} />}
    </article>
  );
}

const STATUS_TEXT = { running: 'Running', succeeded: 'Done', failed: 'Failed', 'not-run': 'Did not run', denied: 'Denied by the host' } as const;

/** `showChanges`: opens a call that changed files, which costs a diff each, so only where they are wanted. */
function ToolView({ item, showChanges, onSelect, onOpenScreenshot }: { item: ToolItem; showChanges: boolean; onSelect: Select; onOpenScreenshot?: OpenScreenshot }) {
  const Icon = { running: Loader, succeeded: Check, failed: X, 'not-run': CircleSlash }[item.status];
  const screenshot = screenshotOf(item);
  const details = (
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
          {item.target && <p className="hint">On {item.target.label ? `${named(item.target)}, ${item.target.kind}` : named(item.target)}</p>}
          {typedTextHidden(item) && <p className="hint">Text hidden</p>}
          {elementHints(item).map((hint) => (
            <p key={hint} className="hint">
              {hint}
            </p>
          ))}
          {item.changes.length > 0
            ? item.changes.map((change, i) => <DiffView key={i} change={change} />)
            : (interactionBody(item) ?? <pre className="tool-input selectable">{JSON.stringify(item.input, null, 2)}</pre>)}
        </div>
      )}
    </Disclosure>
  );
  if (!screenshot) return details;
  return (
    <div className="with-screenshot">
      {details}
      <ScreenshotThumbnail screenshot={screenshot} onOpen={onOpenScreenshot && ((keep) => onOpenScreenshot(item, keep))} />
    </div>
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

