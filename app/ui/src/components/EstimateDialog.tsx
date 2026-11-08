import { useEffect, useId, useMemo, useRef, useState, type FormEvent } from 'react';
import { errorMessage } from '../api/bridge';
import type { CardSet, TimelineEvent } from '../api/types';
import { CARD_SETS, cardsOf, cardLabel, lastCardSet, lastPrompt, MAX_TASK_CHARS, openTasks, rememberCardSet } from '../model/asks';
import { buildDocuments } from '../model/documents';

/** The open tasks of the current plan's `tasks.md`, newest text; none without a plan. */
export function planTasks(timeline: TimelineEvent[]): string[] {
  const written = buildDocuments(timeline);
  const plan = written.plans[0];
  if (!plan) return [];
  const path = plan.documents.flatMap((doc) => doc.paths).find((p) => /(^|\/)tasks\.md$/.test(p));
  const text = path ? written.documents.get(path)?.at(-1)?.text : null;
  return text ? openTasks(text) : [];
}

interface Props {
  timeline: TimelineEvent[];
  /** Fails with a message to show. */
  onStart: (task: string, cards: CardSet) => Promise<void>;
  onClose: () => void;
}

/** *Estimate* (`host-requests`: Estimate a task, Card sets): the task and the cards to vote with. */
export function EstimateDialog({ timeline, onStart, onClose }: Props) {
  const tasks = useMemo(() => planTasks(timeline), [timeline]);
  const prompt = useMemo(() => lastPrompt(timeline), [timeline]);
  const [task, setTask] = useState('');
  const [cards, setCards] = useState<CardSet>(lastCardSet);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const input = useRef<HTMLTextAreaElement>(null);
  const ids = useId();
  const length = [...task.trim()].length;

  useEffect(() => {
    input.current?.focus();
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (length === 0 || length > MAX_TASK_CHARS) return setError(`A task is 1 to ${MAX_TASK_CHARS} characters.`);
    setSending(true);
    try {
      await onStart(task.trim(), cards);
      rememberCardSet(cards);
      onClose();
    } catch (e) {
      setSending(false);
      setError(errorMessage(e));
    }
  };

  return (
    <div className="overlay" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog ask-dialog" role="dialog" aria-modal="true" aria-labelledby={`${ids}-title`}>
        <h2 id={`${ids}-title`}>Estimate a task</h2>
        <form className="ask-form" onSubmit={submit}>
          {(tasks.length > 0 || prompt) && (
            <label>
              Fill from
              <select
                className="field"
                value=""
                onChange={(event) => {
                  if (event.target.value) setTask(event.target.value);
                  setError(null);
                }}
              >
                <option value="">Choose a task or the last prompt…</option>
                {tasks.length > 0 && (
                  <optgroup label="Open tasks in the plan">
                    {tasks.map((t, i) => (
                      <option key={i} value={t}>
                        {t}
                      </option>
                    ))}
                  </optgroup>
                )}
                {prompt && (
                  <optgroup label="The last prompt">
                    <option value={prompt}>{prompt}</option>
                  </optgroup>
                )}
              </select>
            </label>
          )}
          <label>
            Task
            <textarea
              ref={input}
              className="field"
              rows={3}
              value={task}
              onChange={(event) => {
                setTask(event.target.value);
                setError(null);
              }}
              aria-invalid={length > MAX_TASK_CHARS || undefined}
              aria-describedby={`${ids}-count`}
            />
          </label>
          <span className={`field-hint${length > MAX_TASK_CHARS ? ' error-text' : ''}`} id={`${ids}-count`}>
            {length} of {MAX_TASK_CHARS} characters
          </span>
          <fieldset>
            <legend>Cards</legend>
            {(Object.keys(CARD_SETS) as CardSet[]).map((set) => (
              <label key={set} className="check">
                <input type="radio" name={`${ids}-cards`} checked={cards === set} onChange={() => setCards(set)} />
                <span>
                  {CARD_SETS[set].name}{' '}
                  <span className="field-hint">{cardsOf(set).map((card) => cardLabel(card, set)).join(' ')}</span>
                </span>
              </label>
            ))}
          </fieldset>
          {error && (
            <p className="error-text" role="alert">
              {error}
            </p>
          )}
          <div className="actions">
            <button type="button" className="btn" onClick={onClose}>
              Cancel
            </button>
            <button type="submit" className="btn primary" disabled={length === 0 || length > MAX_TASK_CHARS || sending}>
              Start the vote
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
