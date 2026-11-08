import { useState } from 'react';
import type { HostSuggestion, SuggestionAnswer } from '../api/types';
import { suggestionStateText } from './SuggestPanel';

interface Props {
  agentName: string;
  /** Every suggestion in the session, oldest first. */
  suggestions: HostSuggestion[];
  /** Whether viewers may suggest now. */
  allowed: boolean;
  /** Sends the suggestion (maybe edited), keeps it for later, or declines it; declining a kept one removes it. */
  onAnswer: (suggestion: HostSuggestion, answer: SuggestionAnswer) => void;
}

/** The host's Prompts tab: suggestions waiting, oldest first, to send, edit, keep or decline; then those kept for later and those handled. */
export function SuggestionsInbox({ agentName, suggestions, allowed, onAnswer }: Props) {
  const waiting = suggestions.filter((s) => s.state === 'Waiting');
  const kept = suggestions.filter((s) => s.state === 'Kept');
  const handled = suggestions.filter((s) => s.state !== 'Waiting' && s.state !== 'Kept');
  return (
    <div className="suggest">
      <p className="field-hint">
        Sending types the text into {agentName}’s prompt, after anything already typed there, and submits it. Nothing reaches {agentName} until you send it.
      </p>
      <h3 className="suggest-heading">Waiting for you</h3>
      {waiting.length === 0 ? (
        <p className="empty-note">
          {allowed ? 'No suggestions waiting.' : 'Viewers cannot suggest prompts. Turn it on in People.'}
        </p>
      ) : (
        <ul className="suggestions" aria-label="Suggestions waiting">
          {waiting.map((suggestion) => (
            <Open key={`${suggestion.viewer}-${suggestion.id}`} suggestion={suggestion} onAnswer={onAnswer} />
          ))}
        </ul>
      )}
      <h3 className="suggest-heading">Kept for later</h3>
      {kept.length === 0 ? (
        <p className="empty-note">Nothing kept.</p>
      ) : (
        <ul className="suggestions" aria-label="Kept for later">
          {kept.map((suggestion) => (
            <Open key={`${suggestion.viewer}-${suggestion.id}`} suggestion={suggestion} onAnswer={onAnswer} />
          ))}
        </ul>
      )}
      {handled.length > 0 && (
        <>
          <h3 className="suggest-heading">Handled</h3>
          <ul className="suggestions" aria-label="Handled suggestions">
            {[...handled].reverse().map((suggestion) => (
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

/** A waiting or kept suggestion, with what the host can do with it. */
function Open({ suggestion, onAnswer }: { suggestion: HostSuggestion; onAnswer: Props['onAnswer'] }) {
  const [editing, setEditing] = useState<string | null>(null);
  const kept = suggestion.state === 'Kept';
  const label = `${suggestion.from}’s suggestion`;
  const send = (text: string) => onAnswer(suggestion, { type: 'send', text });
  return (
    <li aria-label={label}>
      <span className="field-hint">{suggestion.from}</span>
      {editing === null ? (
        <span className="suggestion-text selectable">{suggestion.text}</span>
      ) : (
        <textarea
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
            <button className="btn primary small" onClick={() => send(suggestion.text)}>
              Send
            </button>
            <button className="btn small" onClick={() => setEditing(suggestion.text)}>
              Edit
            </button>
            {!kept && (
              <button className="btn small" onClick={() => onAnswer(suggestion, { type: 'keep' })}>
                Keep for later
              </button>
            )}
            <button className="btn ghost small" onClick={() => onAnswer(suggestion, { type: 'decline' })}>
              {kept ? 'Remove' : 'Decline'}
            </button>
          </>
        ) : (
          <>
            <button className="btn primary small" disabled={!editing.trim()} onClick={() => send(editing)}>
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
