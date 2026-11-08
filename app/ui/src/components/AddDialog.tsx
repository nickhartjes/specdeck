import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { Upload } from 'lucide-react';
import { errorMessage } from '../api/bridge';
import { MAX_NOTE_BYTES } from '../api/types';
import { formatSize, noteBytes, SHAREABLE } from '../model/sharedFiles';
import type { SharedFiles } from './SharedFiles';

type Destination = 'braindump' | 'everyone';

interface Props {
  shared: SharedFiles;
  /** Files already chosen, such as ones dropped on the Shared tab. */
  files?: File[];
  onClose: () => void;
}

/**
 * Add files or a note (`braindump`): to the braindump for the agent, or shared with everyone.
 * A note goes only to the braindump. Each file is checked and refused on its own, as when sharing.
 */
export function AddDialog({ shared, files: dropped = [], onClose }: Props) {
  const braindump = shared.braindump;
  const [destination, setDestination] = useState<Destination>(braindump ? 'braindump' : 'everyone');
  const [files, setFiles] = useState<File[]>(dropped);
  const [note, setNote] = useState('');
  const [over, setOver] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const ids = useId();
  const toBraindump = destination === 'braindump' && braindump !== undefined;
  const bytes = noteBytes(note);
  const agent = shared.agentName;
  // A terminal session has no agent to give to: the host copies the items into the folder.
  const forAgent = braindump?.promptPossible !== false;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const writing = toBraindump && note.trim() !== '';
    if (files.length === 0 && !writing) return setError(toBraindump ? 'Choose a file or write a note first.' : 'Choose a file to share.');
    if (writing && bytes > MAX_NOTE_BYTES) return setError('Notes can be up to 32 KB.');
    setError(null);
    setSending(true);
    if (writing) {
      try {
        await braindump!.note(note);
      } catch (e) {
        setSending(false);
        return setError(errorMessage(e));
      }
    }
    // Files report back one by one, as they go.
    if (files.length > 0) (toBraindump ? braindump!.add : shared.share)(files);
    onClose();
  };

  const help = toBraindump
    ? forAgent
      ? `Everyone adds to one place. When it is complete, ${shared.host ? 'you give' : `${shared.hostName} gives`} it all to ${agent} at once.`
      : `Everyone adds to one place. When it is complete, ${shared.host ? 'you copy' : `${shared.hostName} copies`} it all into the session folder at once.`
    : `For people to look at. ${agent} does not see it unless ${shared.host ? 'you put' : `${shared.hostName} puts`} it in the session folder.`;

  return (
    <div className="overlay" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby={`${ids}-title`}>
        <h2 id={`${ids}-title`}>{braindump ? 'Add files or a note' : 'Share files'}</h2>
        <form className="share-link-form add-form" onSubmit={submit}>
          {braindump && (
            <label>
              Where it goes
              <select className="field" value={destination} onChange={(event) => setDestination(event.target.value as Destination)}>
                <option value="braindump">{forAgent ? `To the braindump for ${agent}` : 'To the braindump'}</option>
                <option value="everyone">Share with everyone</option>
              </select>
            </label>
          )}
          <p className="field-hint">{help}</p>
          <div
            className={`drop${over ? ' drop-over' : ''}`}
            onDragOver={(event) => {
              if (!event.dataTransfer.types.includes('Files')) return;
              event.preventDefault();
              setOver(true);
            }}
            onDragLeave={() => setOver(false)}
            onDrop={(event) => {
              setOver(false);
              event.preventDefault();
              setFiles([...files, ...event.dataTransfer.files]);
            }}
          >
            <input
              ref={input}
              type="file"
              multiple
              hidden
              aria-label="Files to add"
              onChange={(event) => {
                setFiles([...files, ...(event.target.files ?? [])]);
                event.target.value = '';
              }}
            />
            <button type="button" className="btn small" onClick={() => input.current?.click()}>
              <Upload aria-hidden /> Choose files
            </button>
            <span className="field-hint">{SHAREABLE}. Or drop them here.</span>
            {files.length > 0 && (
              <ul className="chosen" aria-label="Chosen files">
                {files.map((file, i) => (
                  <li key={`${file.name}-${i}`}>
                    {file.name} <span className="field-hint">{formatSize(file.size)}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
          {toBraindump && (
            <label>
              Or write a note
              <textarea
                className="field"
                rows={5}
                value={note}
                placeholder={forAgent ? `Ideas, requirements, edge cases: what ${agent} should know.` : 'Ideas, requirements, edge cases.'}
                onChange={(event) => setNote(event.target.value)}
                aria-invalid={bytes > MAX_NOTE_BYTES || undefined}
                aria-describedby={`${ids}-count`}
              />
              <span id={`${ids}-count`} className={`field-hint${bytes > MAX_NOTE_BYTES ? ' error-text' : ''}`}>
                {formatSize(bytes)} of 32 KB
              </span>
            </label>
          )}
          {error && (
            <p className="error-text" role="alert">
              {error}
            </p>
          )}
          <div className="actions">
            <button type="button" className="btn" onClick={onClose}>
              Cancel
            </button>
            <button type="submit" className="btn primary" disabled={sending}>
              {toBraindump ? 'Add' : 'Share with everyone'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
