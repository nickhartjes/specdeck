import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { errorMessage } from '../api/bridge';
import type { BraindumpItemInfo } from '../api/types';
import { DEFAULT_GIVE_PROMPT } from '../model/sharedFiles';

interface Props {
  /** The items not given yet, oldest first. */
  items: BraindumpItemInfo[];
  agentName: string;
  /** False in a terminal session: the files are only written, and nothing is typed. */
  promptPossible: boolean;
  /** Gives them; the paths written. Fails with why. */
  onGive: (prompt: string | null) => Promise<string[]>;
  /** Called with the paths once given. */
  onGiven: (paths: string[]) => void;
  onClose: () => void;
}

/**
 * Give the braindump to the agent (`braindump`): the host edits the prompt; the items go into
 * shared/braindump/ in the session folder, and their paths follow the prompt.
 */
export function GiveDialog({ items, agentName, promptPossible, onGive, onGiven, onClose }: Props) {
  const [prompt, setPrompt] = useState(DEFAULT_GIVE_PROMPT);
  const [error, setError] = useState<string | null>(null);
  const [giving, setGiving] = useState(false);
  const field = useRef<HTMLTextAreaElement>(null);
  const ids = useId();
  const count = items.length === 1 ? 'The item is' : `The ${items.length} items are`;

  useEffect(() => {
    field.current?.focus();
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (promptPossible && !prompt.trim()) return setError(`Write a prompt for ${agentName} first.`);
    setError(null);
    setGiving(true);
    try {
      onGiven(await onGive(promptPossible ? prompt : null));
      onClose();
    } catch (e) {
      setGiving(false);
      setError(errorMessage(e));
    }
  };

  return (
    <div className="overlay" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby={`${ids}-title`}>
        <h2 id={`${ids}-title`}>{promptPossible ? `Give the braindump to ${agentName}` : 'Copy the braindump to the session folder'}</h2>
        <form className="share-link-form give-form" onSubmit={submit}>
          {promptPossible ? (
            <>
              <p className="field-hint">
                {count} copied into shared/braindump/ in the session folder, then this prompt goes to {agentName}. Edit it first if you like.
              </p>
              <label>
                Prompt for {agentName}
                <textarea ref={field} className="field" rows={5} value={prompt} onChange={(event) => setPrompt(event.target.value)} />
              </label>
            </>
          ) : (
            <p className="field-hint">{count} copied into shared/braindump/ in the session folder. Nothing is typed into the terminal: point your tool to them yourself.</p>
          )}
          <div>
            <span className="field-hint">{promptPossible ? 'Added after the prompt, as their paths:' : 'Copied:'}</span>
            <ul className="chosen" aria-label="Items to give">
              {items.map((item) => (
                <li key={item.info.file}>shared/braindump/{item.info.name}</li>
              ))}
            </ul>
            <span className="field-hint">A name that is already taken there gets a number; nothing is replaced.</span>
          </div>
          {error && (
            <p className="error-text" role="alert">
              {error}
            </p>
          )}
          <div className="actions">
            <button type="button" className="btn" onClick={onClose}>
              Cancel
            </button>
            <button type="submit" className="btn primary" disabled={giving}>
              {promptPossible ? `Give to ${agentName}` : 'Copy to the session folder'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
