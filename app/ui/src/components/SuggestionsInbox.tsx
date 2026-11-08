import { useState } from 'react';
import type { HostSuggestion } from '../api/types';
import { suggestionStateText } from './SuggestPanel';
import { PLAIN_TEXT } from './fields';

interface Props {
  agentName: string;
  /** Every suggestion in the session, oldest first. */
  suggestions: HostSuggestion[];
  /** Whether viewers may suggest now. */
  allowed: boolean;
  /** Sends `text` (the suggestion, maybe edited) to the agent, or declines it when `text` is null. */
  onAnswer: (suggestion: HostSuggestion, text: string | null) => void;
}

/** The host's Prompts tab: suggestions waiting, oldest first, to send, edit or decline. */
export function SuggestionsInbox({ agentName, suggestions, allowed, onAnswer }: Props) {
  const waiting = suggestions.filter((s) => s.state === 'Waiting');
  const answered = suggestions.filter((s) => s.state !== 'Waiting');
  return (
    <div className="suggest">
      <p className="field-hint">
        Sending types the text into {agentName}’s prompt, after anything already typed there, and submits it. Nothing reaches {agentName} until you send it.
      </p>
      {waiting.length === 0 ? (
        <p className="empty-note">
          {allowed ? 'No suggestions waiting.' : 'Viewers cannot suggest prompts. Turn it on in People.'}
        </p>
      ) : (
        <ul className="suggestions" aria-label="Suggestions waiting">
          {waiting.map((suggestion) => (
            <Waiting key={`${suggestion.viewer}-${suggestion.id}`} suggestion={suggestion} onAnswer={onAnswer} />
          ))}
        </ul>
      )}
      {answered.length > 0 && (
        <>
          <h3 className="suggest-heading">Answered</h3>
          <ul className="suggestions" aria-label="Answered suggestions">
            {[...answered].reverse().map((suggestion) => (
              <li key={`${suggestion.viewer}-${suggestion.id}`}>
                <span className="field-hint">{suggestion.from}</span>
                <span className="suggestion-text selectable">{suggestion.text}</span>
                <span className="field-hint">{suggestionStateText(suggestion.state, 'You')}</span>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

function Waiting({ suggestion, onAnswer }: { suggestion: HostSuggestion; onAnswer: Props['onAnswer'] }) {
  const [editing, setEditing] = useState<string | null>(null);
  const label = `${suggestion.from}’s suggestion`;
  return (
    <li aria-label={label}>
      <span className="field-hint">{suggestion.from}</span>
      {editing === null ? (
        <span className="suggestion-text selectable">{suggestion.text}</span>
      ) : (
        <textarea
          {...PLAIN_TEXT}
          className="field"
          rows={4}
          aria-label={`Edit ${label}`}
          value={editing}
          onChange={(event) => setEditing(event.target.value)}
        />
      )}
      <span className="suggest-actions">
        {editing === null ? (
          <>
            <button className="btn primary small" onClick={() => onAnswer(suggestion, suggestion.text)}>
              Send
            </button>
            <button className="btn small" onClick={() => setEditing(suggestion.text)}>
              Edit
            </button>
            <button className="btn ghost small" onClick={() => onAnswer(suggestion, null)}>
              Decline
            </button>
          </>
        ) : (
          <>
            <button className="btn primary small" disabled={!editing.trim()} onClick={() => onAnswer(suggestion, editing)}>
              Send
            </button>
            <button className="btn ghost small" onClick={() => setEditing(null)}>
              Cancel
            </button>
          </>
        )}
      </span>
    </li>
  );
}
