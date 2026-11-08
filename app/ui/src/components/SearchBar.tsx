import { useEffect, useRef } from 'react';
import { ChevronDown, ChevronUp, X } from 'lucide-react';
import { PLAIN_TEXT } from './fields';

interface Props {
  term: string;
  onTerm: (term: string) => void;
  onNext: () => void;
  onPrevious: () => void;
  onClose: () => void;
  /** Matches in the terminal output, and in the timeline. */
  terminal: { index: number; count: number };
  timeline: number;
}

/** Find in the session: terminal output and timeline. Enter steps forward, Shift+Enter back. */
export function SearchBar({ term, onTerm, onNext, onPrevious, onClose, terminal, timeline }: Props) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);
  const none = term && terminal.count === 0 && timeline === 0;
  const summary = !term
    ? ''
    : none
      ? 'No matches'
      : `${terminal.count > 0 ? `${terminal.index + 1}/${terminal.count}` : '0'} in terminal${timeline ? ` · ${timeline} in timeline` : ''}`;
  return (
    <div className="search" role="search">
      <input
        {...PLAIN_TEXT}
        ref={input}
        className="field"
        placeholder="Find in session"
        aria-label="Find in session"
        value={term}
        onChange={(event) => onTerm(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter') (event.shiftKey ? onPrevious : onNext)();
          if (event.key === 'Escape') onClose();
        }}
      />
      <span className="count" aria-live="polite">
        {summary}
      </span>
      <button className="btn ghost icon" onClick={onPrevious} aria-label="Previous match" disabled={!term}>
        <ChevronUp aria-hidden />
      </button>
      <button className="btn ghost icon" onClick={onNext} aria-label="Next match" disabled={!term}>
        <ChevronDown aria-hidden />
      </button>
      <button className="btn ghost icon" onClick={onClose} aria-label="Close search">
        <X aria-hidden />
      </button>
    </div>
  );
}
