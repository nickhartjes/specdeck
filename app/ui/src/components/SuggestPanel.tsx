import { useState, type FormEvent } from 'react';
import { MAX_SUGGESTION_BYTES, type RequestOutcome, type Suggestion, type SuggestionState } from '../api/types';

interface Props {
  /** The host's name, who decides on every suggestion. */
  host: string;
  agentName: string;
  /** Whether the host lets viewers suggest now. */
  allowed: boolean;
  /** This viewer's suggestions and what became of each. */
  mine: Suggestion[];
  /** The id of the request just sent, and the host's reply to it once it came. */
  sent: { id: number; outcome: RequestOutcome | null } | null;
  /** Fails for text the host would refuse. */
  onSuggest: (text: string) => Promise<void>;
}

/** Bytes of UTF-8, as the host counts them. */
export function suggestionBytes(text: string): number {
  return new TextEncoder().encode(text.replace(/\r\n/g, '\n')).length;
}

/** What became of a suggestion, in words. */
export function suggestionStateText(state: SuggestionState, host: string): string {
  if (state === 'Waiting') return `Waiting for ${host}`;
  if (state === 'Declined') return `${host} declined it`;
  return state.Sent.edited ? `${host} edited and sent it` : `${host} sent it`;
}

/** A viewer's Prompts tab: suggest a prompt to the host, and see what became of each. */
export function SuggestPanel({ host, agentName, allowed, mine, sent, onSuggest }: Props) {
  const [text, setText] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const bytes = suggestionBytes(text);
  const tooLong = bytes > MAX_SUGGESTION_BYTES;
  const outcome = sent?.outcome ?? null;

  // The text stays until the host has it; refused, it stays to send again. Adjusted while
  // rendering, when the reply to what was just sent comes in.
  const replied = sent && outcome ? `${sent.id}:${outcome}` : null;
  const [handled, setHandled] = useState(replied);
  if (sending && replied !== null && replied !== handled) {
    setHandled(replied);
    setSending(false);
    if (outcome === 'Received') setText('');
  }

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!text.trim() || tooLong) return;
    setError(null);
    setSending(true);
    try {
      await onSuggest(text);
    } catch (e) {
      setSending(false);
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const notice =
    error ??
    (outcome === 'Off'
      ? `${host} has turned suggestions off. Your suggestion was not sent.`
      : outcome === 'RateLimited'
        ? 'Wait a moment before suggesting again.'
        : null);

  return (
    <div className="suggest">
      <p className="field-hint">
        A suggestion goes to {host}, not to {agentName}. Only {host} sees it, and decides whether to send it.
      </p>
      {allowed ? (
        <form onSubmit={submit} className="suggest-form">
          <textarea
            className="field"
            aria-label="Prompt to suggest"
            rows={4}
            value={text}
            placeholder={`What should ${agentName} do next?`}
            onChange={(event) => setText(event.target.value)}
            aria-invalid={tooLong || undefined}
            aria-describedby="suggestion-size"
          />
          <div className="suggest-row">
            <span id="suggestion-size" className={tooLong ? 'error-text' : 'field-hint'}>
              {tooLong ? `Too long: ${bytes} of ${MAX_SUGGESTION_BYTES} bytes` : `${bytes} of ${MAX_SUGGESTION_BYTES} bytes`}
            </span>
            <button className="btn primary small" type="submit" disabled={!text.trim() || tooLong || sending}>
              {sending ? 'Sending…' : `Suggest to ${host}`}
            </button>
          </div>
          {notice && (
            <p className="error-text" role="status">
              {notice}
            </p>
          )}
        </form>
      ) : (
        <p className="empty-note">{host} has turned prompt suggestions off.</p>
      )}
      <h3 className="suggest-heading">Your suggestions</h3>
      {mine.length === 0 ? (
        <p className="empty-note">You have not suggested anything yet.</p>
      ) : (
        <ul className="suggestions" aria-label="Your suggestions">
          {[...mine].reverse().map((suggestion) => (
            <li key={suggestion.id}>
              <span className="suggestion-text selectable">{suggestion.text}</span>
              <span className="field-hint">{suggestionStateText(suggestion.state, host)}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
