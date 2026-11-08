import { useEffect, useId, useMemo, useState, type FormEvent } from 'react';
import { errorMessage } from '../api/bridge';
import type { ReadTarget, SharedFileInfo, TimelineEvent } from '../api/types';
import { lastPromptChanges, READ_LIMITS } from '../model/asks';
import { buildDocuments } from '../model/documents';
import { buildTimeline } from '../model/timeline';
import { isDocument } from './FilesPanel';

/** Longest label of a read request, in characters; `MAX_READ_LABEL_CHARS` in crates/protocol. */
const MAX_LABEL_CHARS = 255;

interface Choice {
  key: string;
  target: ReadTarget;
  /** What everyone is asked to read, as said in a sentence and in the timeline. */
  label: string;
  /** How the dialog lists it, if not as the label. */
  name?: string;
}

const fileName = (path: string) => path.split('/').at(-1) ?? path;
const cut = (text: string) => [...text].slice(0, MAX_LABEL_CHARS).join('');

/** What the host can ask to read, grouped; a session without a timeline has shared files only. */
export function readChoices(timeline: TimelineEvent[], timelineSupported: boolean, shared: SharedFileInfo[]): { group: string; choices: Choice[] }[] {
  const groups: { group: string; choices: Choice[] }[] = [];
  if (timelineSupported) {
    const changes = lastPromptChanges(timeline);
    if (changes) groups.push({ group: 'Changes', choices: [{ key: 'changes', target: changes, label: 'the changes in the last prompt', name: 'The changes in the last prompt' }] });
    const documents = [...buildDocuments(timeline).documents.keys()];
    groups.push({ group: 'Documents', choices: documents.map((path) => ({ key: `document:${path}`, target: { document: { path } }, label: cut(fileName(path)) })) });
    const code = buildTimeline(timeline).files.filter((file) => file.changes.length > 0 && !isDocument(file.path));
    groups.push({ group: 'Code', choices: code.map((file) => ({ key: `code:${file.path}`, target: { code: { path: file.path } }, label: cut(fileName(file.path)) })) });
  }
  groups.push({ group: 'Shared', choices: shared.map((file) => ({ key: `shared:${file.file}`, target: { shared: { file: file.file } }, label: cut(file.name) })) });
  return groups.filter((group) => group.choices.length > 0);
}

interface Props {
  timeline: TimelineEvent[];
  timelineSupported: boolean;
  shared: SharedFileInfo[];
  /** Fails with a message to show. */
  onAsk: (target: ReadTarget, label: string, limitS: number) => Promise<void>;
  onClose: () => void;
}

/** *Ask to read* (`host-requests`: Ask to read, What can be read): one thing, and a time limit. */
export function AskReadDialog({ timeline, timelineSupported, shared, onAsk, onClose }: Props) {
  const groups = useMemo(() => readChoices(timeline, timelineSupported, shared), [timeline, timelineSupported, shared]);
  const [picked, setPicked] = useState<string | null>(null);
  const [limit, setLimit] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const ids = useId();
  const choice = groups.flatMap((group) => group.choices).find((c) => c.key === picked) ?? null;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!choice) return;
    setSending(true);
    try {
      await onAsk(choice.target, choice.label, limit);
      onClose();
    } catch (e) {
      setSending(false);
      setError(errorMessage(e));
    }
  };

  return (
    <div className="overlay" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog ask-dialog" role="dialog" aria-modal="true" aria-labelledby={`${ids}-title`}>
        <h2 id={`${ids}-title`}>Ask everyone to read</h2>
        <form className="ask-form" onSubmit={submit}>
          {groups.length === 0 ? (
            <p>{timelineSupported ? 'Nothing to read yet: no documents, changed files or shared files.' : 'Share a file first: in a terminal session only shared files can be read.'}</p>
          ) : (
            <div className="ask-choices">
              {groups.map((group) => (
                <fieldset key={group.group}>
                  <legend>{group.group}</legend>
                  {group.choices.map((c) => (
                    <label key={c.key} className="check" title={'document' in c.target ? c.target.document.path : 'code' in c.target ? c.target.code.path : undefined}>
                      <input type="radio" name={`${ids}-target`} checked={picked === c.key} onChange={() => setPicked(c.key)} />
                      <span>{c.name ?? c.label}</span>
                    </label>
                  ))}
                </fieldset>
              ))}
            </div>
          )}
          <label>
            Time limit
            <select className="field" value={limit} onChange={(event) => setLimit(Number(event.target.value))}>
              {READ_LIMITS.map((l) => (
                <option key={l.seconds} value={l.seconds}>
                  {l.label}
                </option>
              ))}
            </select>
          </label>
          {error && (
            <p className="error-text" role="alert">
              {error}
            </p>
          )}
          <div className="actions">
            <button type="button" className="btn" onClick={onClose}>
              Cancel
            </button>
            <button type="submit" className="btn primary" disabled={!choice || sending}>
              Ask to read
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
