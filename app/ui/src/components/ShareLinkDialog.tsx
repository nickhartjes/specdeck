import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { errorMessage } from '../api/bridge';
import { MAX_LINK_TITLE_CHARS, checkLink, hasFragment, refusalText, validTitle } from '../model/links';

interface Props {
  /** The host shares with everyone at once; a viewer sends it to the host. */
  host: boolean;
  /** The host's name, for a viewer. */
  hostName?: string;
  /** Gets the normalized address; fails with a message to show. */
  onShare: (url: string, title: string | null, sessionOnly: boolean) => Promise<void>;
  onClose: () => void;
}

/** Share a link with everyone in the session (`shared-links`), or, for a viewer, send it to the host. */
export function ShareLinkDialog({ host, hostName = 'the host', onShare, onClose }: Props) {
  const [address, setAddress] = useState('');
  const [title, setTitle] = useState('');
  // Follows the address's # part until the person sets it themselves.
  const [sessionOnly, setSessionOnly] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const ids = useId();
  const marked = sessionOnly ?? hasFragment(address.trim());

  useEffect(() => {
    input.current?.focus();
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const checked = checkLink(address);
    if (!('url' in checked)) return setError(refusalText(checked.refusal, host));
    if (!validTitle(title)) return setError(`A title is one line of at most ${MAX_LINK_TITLE_CHARS} characters.`);
    setError(null);
    setSending(true);
    try {
      await onShare(checked.url, title.trim() || null, marked);
      onClose();
    } catch (e) {
      setSending(false);
      setError(errorMessage(e));
    }
  };

  return (
    <div className="overlay" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby={`${ids}-title`}>
        <h2 id={`${ids}-title`}>{host ? 'Share a link' : `Send a link to ${hostName}`}</h2>
        <form className="share-link-form" onSubmit={submit}>
          <p className="field-hint">
            {host
              ? 'Everyone in the session sees it and opens it in their own browser, with their own logins. SpecDeck only carries the address.'
              : `${hostName} sees it first, and decides whether to share it with everyone. It opens in each person’s own browser.`}
          </p>
          <label>
            Address
            <input
              ref={input}
              className="field"
              type="url"
              inputMode="url"
              autoComplete="off"
              spellCheck={false}
              placeholder="https://"
              value={address}
              onChange={(event) => setAddress(event.target.value)}
              aria-invalid={error !== null || undefined}
              aria-describedby={error ? `${ids}-error` : undefined}
            />
          </label>
          <label>
            Title (optional)
            <input className="field" maxLength={MAX_LINK_TITLE_CHARS} value={title} onChange={(event) => setTitle(event.target.value)} />
          </label>
          <label className="check">
            <input type="checkbox" checked={marked} onChange={(event) => setSessionOnly(event.target.checked)} />
            <span>
              For this session only
              <span className="field-hint">
                {' '}
                · hides the part after #, which can hold a key, as in an Excalidraw room; left out of recaps and exports
              </span>
            </span>
          </label>
          {error && (
            <p className="error-text" id={`${ids}-error`} role="alert">
              {error}
            </p>
          )}
          <div className="actions">
            <button type="button" className="btn" onClick={onClose}>
              Cancel
            </button>
            <button type="submit" className="btn primary" disabled={!address.trim() || sending}>
              {host ? 'Share with everyone' : `Send to ${hostName}`}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
