import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { BookOpen, Check, Clock, MessageCircleQuestionMark } from 'lucide-react';
import type { Participant, ReadAnswer, ReadView } from '../api/types';
import { formatLeft, MAX_QUESTION_CHARS, READ_STATE_TEXT, targetKind, timeLeft } from '../model/asks';

/** The time left, counted down each second from what the host sent; null without a limit. */
export function useTimeLeft(leftMs: number | null, receivedAt: number): number | null {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (leftMs === null) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [leftMs, receivedAt]);
  // A view newer than the last tick counts from when it arrived.
  return leftMs === null ? null : timeLeft(leftMs, receivedAt, Math.max(now, receivedAt));
}

export const nameOf = (participants: Participant[], id: number, me?: number) =>
  id === me ? 'You' : (participants.find((p) => p.id === id)?.label ?? `Viewer ${id}`);

interface Props {
  view: ReadView;
  /** When the view arrived, to count down from. */
  receivedAt: number;
  participants: Participant[];
  onOpen: () => void;
  /** The host's: closes the request for everyone. Absent for a viewer. */
  onEnd?: () => void;
  /** A viewer's: answers. Absent for the host. */
  onAnswer?: (answer: ReadAnswer) => void;
  /** Why the last answer did not count, if it did not. */
  notice?: string | null;
  /** The host's: each new value shows the question texts, after a click on a question's notification. */
  showQuestions?: number;
}

/** An open read request in the request row (`host-requests`: Ask to read, Ready or a question, The host sees who read). */
export function ReadBanner({ view, receivedAt, participants, onOpen, onEnd, onAnswer, notice, showQuestions = 0 }: Props) {
  const left = useTimeLeft(view.leftMs, receivedAt);
  const host = onEnd !== undefined;
  const answering = view.people.filter(([, state]) => state !== 'older-app');
  const ready = answering.filter(([, state]) => state === 'ready').length;
  const questions = answering.filter(([, state]) => state === 'question').length;
  const counts = [`${ready} of ${answering.length} ready`, questions > 0 && `${questions} with a question`].filter(Boolean).join(', ');
  const [asking, setAsking] = useState(false);
  const [text, setText] = useState('');
  const ids = useId();
  const mine = view.mine;
  const myQuestion = typeof mine === 'object' ? mine.question.text : null;
  const banner = useRef<HTMLElement>(null);
  const questionTexts = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    if (showQuestions === 0) return;
    if (questionTexts.current) questionTexts.current.open = true;
    banner.current?.scrollIntoView?.({ block: 'nearest' });
  }, [showQuestions]);

  const sendQuestion = (event: FormEvent) => {
    event.preventDefault();
    if ([...text].length > MAX_QUESTION_CHARS) return;
    onAnswer?.({ question: { text: text.trim() || null } });
    setAsking(false);
  };

  return (
    <section ref={banner} className="ask-banner" aria-labelledby={`${ids}-title`}>
      <div className="ask-banner-head">
        <BookOpen aria-hidden />
        <strong id={`${ids}-title`}>
          {host ? 'You asked everyone to read' : 'The host asks you to read'} <span className="selectable">{view.label}</span>
        </strong>
        {!('changes' in view.target) && <span className="field-hint">{targetKind(view.target)}</span>}
        <span
          className={`ask-time${left !== null && left <= 30_000 ? ' ending' : ''}`}
          role="timer"
          aria-label={left === null ? 'No time limit' : `${formatLeft(left)} left`}
        >
          <Clock aria-hidden className="inline-icon" /> {left === null ? 'No time limit' : `${formatLeft(left)} left`}
        </span>
        <div className="spacer" />
        <button className="btn small" onClick={onOpen}>
          Open it
        </button>
        {host && (
          <button className="btn small primary" onClick={onEnd}>
            Go on
          </button>
        )}
        {!host &&
          (mine === 'ready' ? (
            <button className="btn small answered ready" aria-pressed="true" onClick={() => onAnswer?.('reading')} title="Take it back: still reading">
              <Check aria-hidden /> Ready
            </button>
          ) : (
            <button className="btn small primary" onClick={() => onAnswer?.('ready')}>
              <Check aria-hidden /> I’m ready
            </button>
          ))}
        {!host &&
          (typeof mine === 'object' ? (
            <button className="btn small answered question" aria-pressed="true" onClick={() => onAnswer?.('reading')} title="Take the question back">
              <MessageCircleQuestionMark aria-hidden /> You have a question
            </button>
          ) : (
            <button className="btn small" aria-expanded={asking} onClick={() => setAsking((open) => !open)}>
              <MessageCircleQuestionMark aria-hidden /> I have a question
            </button>
          ))}
      </div>
      <div className="read-progress">
        <span className="read-bar" aria-hidden>
          <span className="ready" style={{ flexGrow: ready }} />
          <span className="question" style={{ flexGrow: questions }} />
          <span className="reading" style={{ flexGrow: answering.length - ready - questions }} />
        </span>
        <span className="field-hint">{counts}</span>
      </div>
      {!host && myQuestion && <p className="ask-note">Your question: <span className="selectable">{myQuestion}</span></p>}
      {!host && asking && (
        <form className="ask-question" onSubmit={sendQuestion}>
          <label>
            Your question for the host (optional)
            <input
              className="field"
              autoFocus
              value={text}
              onChange={(event) => setText(event.target.value)}
              aria-invalid={[...text].length > MAX_QUESTION_CHARS || undefined}
              aria-describedby={`${ids}-count`}
            />
          </label>
          <span className="field-hint" id={`${ids}-count`}>
            {[...text].length} of {MAX_QUESTION_CHARS} · only the host sees it
          </span>
          <button className="btn small primary" type="submit" disabled={[...text].length > MAX_QUESTION_CHARS}>
            Send
          </button>
          <button className="btn small" type="button" onClick={() => setAsking(false)}>
            Cancel
          </button>
        </form>
      )}
      {host && view.people.length > 0 && (
        <ul className="ask-people" aria-label="Who read">
          {view.people.map(([id, state]) => (
            <li key={id} className={`ask-state ${state}`}>
              <span className="dot" aria-hidden />
              {nameOf(participants, id)}: {READ_STATE_TEXT[state]}
            </li>
          ))}
        </ul>
      )}
      {host && view.questions.length > 0 && (
        <details ref={questionTexts} className="ask-questions">
          <summary>{view.questions.length === 1 ? '1 question' : `${view.questions.length} questions`}</summary>
          <ul>
            {view.questions.map(([id, question]) => (
              <li key={id}>
                <strong>{nameOf(participants, id)}:</strong> <span className="selectable">{question}</span>
              </li>
            ))}
          </ul>
        </details>
      )}
      {notice && (
        <p className="error-text" role="alert">
          {notice}
        </p>
      )}
    </section>
  );
}
