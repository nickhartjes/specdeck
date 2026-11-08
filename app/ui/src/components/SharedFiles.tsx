import { useEffect, useMemo, useRef, useState } from 'react';
import { Check, Download, FileText, FolderInput, Image, NotebookPen, Pencil, Send, Share2, Trash2, Upload } from 'lucide-react';
import { errorMessage } from '../api/bridge';
import { MAX_NOTE_BYTES, type BraindumpItemInfo, type SharedFileInfo } from '../api/types';
import { languageOf, tokenizeLines } from '../model/highlight';
import { dataUrl, formatSize, noteBytes, refusal, SHAREABLE, sharedBy, textOf } from '../model/sharedFiles';
import { AddDialog } from './AddDialog';
import { CodeLines } from './CodeLines';
import { GiveDialog } from './GiveDialog';
import { Markdown } from './Markdown';

/** Text longer than this is shown without highlighting, to keep the window responsive. */
const MAX_HIGHLIGHTED_LINES = 50_000;

/** Shared files and what this person may do with them (`shared-files`). */
export interface SharedFiles {
  files: SharedFileInfo[];
  /** This person's participant id: 0 for the host. */
  me: number | undefined;
  host: boolean;
  /** Whether this person may share files now. */
  canShare: boolean;
  /** The host's name, for "the host has not turned on sharing files". */
  hostName: string;
  agentName: string;
  /** Shares files; refusals are reported by the caller, file by file. */
  share: (files: File[]) => void;
  open: (file: number) => Promise<ArrayBuffer>;
  remove: (file: number) => void;
  /** Puts a file into shared/ in the session folder; the host only. */
  put?: (file: number) => Promise<string>;
  save: (name: string, bytes: ArrayBuffer) => Promise<boolean>;
  /** Tells this person something, such as a refusal. */
  say: (message: string) => void;
  /** The braindump; absent where the session has none. */
  braindump?: Braindump;
}

/** The braindump and what this person may do with it (`braindump`). */
export interface Braindump {
  items: BraindumpItemInfo[];
  /** Adds files; refusals are reported by the caller, file by file. */
  add: (files: File[]) => void;
  /** Adds a note, or replaces this person's note `file`; fails with why. */
  note: (text: string, file?: number) => Promise<void>;
  /** The host's: gives what was not given yet, with a prompt unless `promptPossible` is false; the paths written. */
  give?: (prompt: string | null) => Promise<string[]>;
  /** False in terminal sessions, where giving only writes the files. */
  promptPossible: boolean;
}

/**
 * Checks `files` and shares those that may go, one by one; says why for each one that may not.
 * The host decides again from the content.
 */
export async function shareEach(
  files: File[],
  send: (name: string, bytes: ArrayBuffer) => Promise<unknown>,
  say: (message: string) => void,
  done = (name: string) => `${name} is shared with everyone.`,
) {
  for (const file of files) {
    const refused = refusal(file);
    if (refused) {
      say(refused);
      continue;
    }
    try {
      await send(file.name, await file.arrayBuffer());
      say(done(file.name));
    } catch (error) {
      say(`${file.name}: ${errorMessage(error)}`);
    }
  }
}

const iconOf = (file: SharedFileInfo, note = false) => (note ? NotebookPen : file.kind === 'image' ? Image : FileText);
const TIME = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });

/** The Shared tab of the files panel: the braindump and Shared with everyone (`session-window` spec: Shared tab). */
export function SharedTab({ shared, active, onOpen }: { shared: SharedFiles; active: number | null; onOpen: (file: SharedFileInfo, keep: boolean) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const [over, setOver] = useState(false);
  // Open with the files chosen so far, or closed.
  const [adding, setAdding] = useState<File[] | null>(null);
  const [giving, setGiving] = useState(false);
  const newestFirst = useMemo(() => [...shared.files].reverse(), [shared.files]);
  const braindump = shared.braindump;
  const take = (files: FileList | null) => {
    if (!files || files.length === 0) return;
    // With a braindump, the dialog asks where they go.
    if (braindump) setAdding([...files]);
    else shared.share([...files]);
  };
  return (
    <div
      className={`shared-tab${over ? ' drop-over' : ''}`}
      onDragOver={(event) => {
        if (!shared.canShare || !event.dataTransfer.types.includes('Files')) return;
        event.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(event) => {
        setOver(false);
        if (!shared.canShare) return;
        event.preventDefault();
        take(event.dataTransfer.files);
      }}
    >
      {braindump && (
        <BraindumpGroup
          shared={shared}
          braindump={braindump}
          active={active}
          onOpen={onOpen}
          onGive={() => setGiving(true)}
        />
      )}
      <section aria-labelledby="shared-everyone">
        <h3 id="shared-everyone" className="group-title">
          Shared with everyone
        </h3>
        <p className="field-hint group-help">
          To look at together. {shared.agentName} does not see these unless {shared.host ? 'you put' : `${shared.hostName} puts`} one in the session folder.
        </p>
        {newestFirst.length === 0 ? (
          <p className="field-hint group-help">Nothing shared yet. {shared.canShare ? 'Share a file, or drop one here.' : ''}</p>
        ) : (
          <ul className="files" aria-label="Shared with everyone">
            {newestFirst.map((file) => {
              const Icon = iconOf(file);
              const who = sharedBy(file, shared.me);
              return (
                <li key={file.file}>
                  <button
                    aria-current={active === file.file || undefined}
                    aria-label={`${file.name}, shared by ${who}, ${formatSize(file.size)}`}
                    title={file.name}
                    onClick={() => onOpen(file, false)}
                    onDoubleClick={() => onOpen(file, true)}
                  >
                    <Icon aria-hidden />
                    <span className="name">{file.name}</span>
                    <span className="folder">
                      {who}, {TIME.format(file.atMs)}, {formatSize(file.size)}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </section>
      <div className="shared-foot">
        {shared.canShare ? (
          <>
            <input
              ref={input}
              type="file"
              multiple
              hidden
              aria-label="Files to share"
              onChange={(event) => {
                take(event.target.files);
                event.target.value = '';
              }}
            />
            {braindump ? (
              <button className="btn small" onClick={() => setAdding([])} title={`${SHAREABLE}, or a note. Or drop files here.`}>
                <Upload aria-hidden /> Add files or a note
              </button>
            ) : (
              <button className="btn small" onClick={() => input.current?.click()} title={`${SHAREABLE}. Or drop them here.`}>
                <Upload aria-hidden /> Share files
              </button>
            )}
          </>
        ) : (
          <span className="field-hint">{shared.hostName} has not turned on {braindump ? 'adding files and notes' : 'sharing files'}.</span>
        )}
      </div>
      {adding && <AddDialog shared={shared} files={adding} onClose={() => setAdding(null)} />}
      {giving && braindump?.give && (
        <GiveDialog
          items={braindump.items.filter((item) => item.given === null)}
          agentName={shared.agentName}
          promptPossible={braindump.promptPossible}
          onGive={braindump.give}
          onGiven={(paths) => shared.say(`In shared/braindump/ now: ${paths.map((path) => path.split('/').at(-1)).join(', ')}.`)}
          onClose={() => setGiving(false)}
        />
      )}
    </div>
  );
}

/** The braindump for the agent: everyone's notes and files, and the host's Give N to the agent. */
function BraindumpGroup({
  shared,
  braindump,
  active,
  onOpen,
  onGive,
}: {
  shared: SharedFiles;
  braindump: Braindump;
  active: number | null;
  onOpen: (file: SharedFileInfo, keep: boolean) => void;
  onGive: () => void;
}) {
  const fresh = braindump.items.filter((item) => item.given === null).length;
  const agent = shared.agentName;
  // A terminal session has no agent to give to: the host copies the items into the folder.
  const title = braindump.promptPossible ? `Braindump for ${agent}` : 'Braindump';
  const done = braindump.promptPossible ? `Given to ${agent}` : 'Copied to the session folder';
  return (
    <section aria-labelledby="shared-braindump" className="braindump">
      <div className="group-head">
        <h3 id="shared-braindump" className="group-title">
          {title}
        </h3>
        {shared.host && braindump.give && fresh > 0 && (
          <button className="btn small primary" onClick={onGive}>
            <Send aria-hidden /> {braindump.promptPossible ? `Give ${fresh} to ${agent}` : `Copy ${fresh} to the session folder`}
          </button>
        )}
      </div>
      <p className="field-hint group-help">
        Notes and files from everyone.{' '}
        {braindump.promptPossible
          ? `${shared.host ? 'You give' : `${shared.hostName} gives`} them to ${agent} together, as one folder.`
          : `${shared.host ? 'You copy' : `${shared.hostName} copies`} them into the session folder together.`}
      </p>
      {braindump.items.length === 0 ? (
        <p className="field-hint group-help">Nothing yet. {shared.canShare ? 'Add a note or a file.' : ''}</p>
      ) : (
        <ul className="files" aria-label={title}>
          {braindump.items.map(({ info, note, given }) => {
            const Icon = iconOf(info, note);
            const who = sharedBy(info, shared.me);
            return (
              <li key={info.file}>
                <button
                  aria-current={active === info.file || undefined}
                  aria-label={`${info.name}, added by ${who}, ${formatSize(info.size)}${given !== null ? (braindump.promptPossible ? `, given to ${agent}` : ', copied to the session folder') : ''}`}
                  title={info.name}
                  onClick={() => onOpen(info, false)}
                  onDoubleClick={() => onOpen(info, true)}
                >
                  <Icon aria-hidden />
                  <span className="name">{info.name}</span>
                  <span className="folder">
                    {who}, {TIME.format(info.atMs)}, {formatSize(info.size)}
                  </span>
                  {given !== null && (
                    <span className="given" title={done}>
                      <Check aria-hidden /> {braindump.promptPossible ? 'Given' : 'Copied'}
                    </span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

/** What came of fetching file `id`; another file than the one shown counts as still loading. */
type Loaded = { id: number } & ({ state: 'ready'; bytes: ArrayBuffer } | { state: 'failed'; message: string });

/** A shared file in a tab: an image, Markdown or text, with who shared it and what can be done with it. */
export function SharedFileTab({
  file,
  item,
  shared,
  progress,
  onKeep,
}: {
  /** Null once the file was removed. */
  file: SharedFileInfo | null;
  /** The file's place in the braindump, when it is in it. */
  item?: BraindumpItemInfo;
  shared: SharedFiles;
  /** Bytes received so far, while it is fetched. */
  progress?: number;
  onKeep: () => void;
}) {
  const [result, setLoaded] = useState<Loaded | null>(null);
  const [source, setSource] = useState(false);
  // The note's text while its author edits it.
  const [editing, setEditing] = useState<string | null>(null);
  const id = file?.file;
  const open = shared.open;
  const loaded = result?.id === id ? result : null;
  useEffect(() => {
    if (id === undefined) return;
    let current = true;
    open(id).then(
      (bytes) => current && setLoaded({ id, state: 'ready', bytes }),
      (error) => current && setLoaded({ id, state: 'failed', message: errorMessage(error) }),
    );
    return () => {
      current = false;
    };
  }, [id, open]);

  if (!file) {
    return (
      <div className="empty">
        <Trash2 aria-hidden />
        <strong>This file was removed</strong>
        <span>It is no longer shared.</span>
      </div>
    );
  }
  const mine = file.by === shared.me;
  const ready = loaded?.state === 'ready' ? loaded.bytes : null;
  const save = () => {
    if (!ready) return;
    shared.save(file.name, ready).then(
      (saved) => saved && shared.say(`${file.name} saved.`),
      (error) => shared.say(errorMessage(error)),
    );
  };
  const agent = shared.agentName;
  const editable = item?.note && item.given === null && mine && shared.braindump !== undefined;
  const forAgent = shared.braindump?.promptPossible !== false;
  const where = item
    ? item.given !== null
      ? forAgent
        ? `Given to ${agent}`
        : 'Copied to the session folder'
      : forAgent
        ? `In the braindump for ${agent}`
        : 'In the braindump'
    : 'Shared with everyone';
  const put = () =>
    shared.put?.(file.file).then(
      (path) => shared.say(`In ${path} now. ${shared.agentName} can read it there.`),
      (error) => shared.say(errorMessage(error)),
    );
  return (
    <div className="shared-file-tab">
      <div className="filebar">
        <span className="path selectable">{file.name}</span>
        <span className="muted">
          From {sharedBy(file, shared.me).replace(/^You$/, 'you')}, {TIME.format(file.atMs)}, {formatSize(file.size)}. {where}.
        </span>
        <span className="spacer" />
        {file.kind === 'markdown' && ready && editing === null && (
          <div className="toggle" role="group" aria-label="How to show the document">
            <button type="button" aria-pressed={!source} onClick={() => (setSource(false), onKeep())}>
              Formatted
            </button>
            <button type="button" aria-pressed={source} onClick={() => (setSource(true), onKeep())}>
              Source
            </button>
          </div>
        )}
        {editable && ready && editing === null && (
          <button className="btn small" onClick={() => (setEditing(textOf(ready)), onKeep())}>
            <Pencil aria-hidden /> Edit
          </button>
        )}
        {shared.host && shared.put && !item && (
          <button className="btn small" onClick={put} title={`Writes it into shared/ in the session folder, where ${shared.agentName} can read it`}>
            <FolderInput aria-hidden /> Put in the session folder
          </button>
        )}
        <button className="btn small" onClick={save} disabled={!ready}>
          <Download aria-hidden /> Save a copy
        </button>
        {(shared.host || mine) && (
          <button className="btn small ghost" onClick={() => shared.remove(file.file)}>
            <Trash2 aria-hidden /> Remove
          </button>
        )}
      </div>
      {loaded?.state === 'failed' ? (
        <div className="empty">
          <Share2 aria-hidden />
          <strong>Could not open {file.name}</strong>
          <span>{loaded.message}</span>
        </div>
      ) : !ready ? (
        <div className="empty" role="status">
          <span className="spinner" />
          <span>
            Getting {file.name}… {progress !== undefined && file.size > 0 ? `${Math.floor((progress / file.size) * 100)}%` : ''}
          </span>
        </div>
      ) : editing !== null && shared.braindump ? (
        <NoteEditor
          text={editing}
          onChange={setEditing}
          onCancel={() => setEditing(null)}
          onSave={() =>
            shared.braindump!.note(editing, file.file).then(
              () => (setEditing(null), shared.say(`${file.name} saved.`)),
              (error) => shared.say(errorMessage(error)),
            )
          }
        />
      ) : (
        <SharedContent file={file} bytes={ready} source={source} />
      )}
    </div>
  );
}

/** A note being edited by its author, before it is given. */
function NoteEditor({ text, onChange, onCancel, onSave }: { text: string; onChange: (text: string) => void; onCancel: () => void; onSave: () => void }) {
  const bytes = noteBytes(text);
  const tooLong = bytes > MAX_NOTE_BYTES;
  return (
    <div className="note-editor">
      <textarea className="field" aria-label="Note" value={text} onChange={(event) => onChange(event.target.value)} autoFocus />
      <div className="actions">
        <span className={`field-hint${tooLong ? ' error-text' : ''}`}>{formatSize(bytes)} of 32 KB</span>
        <span className="spacer" />
        <button className="btn small" onClick={onCancel}>
          Cancel
        </button>
        <button className="btn small primary" onClick={onSave} disabled={tooLong || !text.trim()}>
          Save
        </button>
      </div>
    </div>
  );
}

function SharedContent({ file, bytes, source }: { file: SharedFileInfo; bytes: ArrayBuffer; source: boolean }) {
  const text = useMemo(() => (file.kind === 'image' ? '' : textOf(bytes)), [file.kind, bytes]);
  const url = useMemo(() => (file.kind === 'image' ? dataUrl(file.mediaType, bytes) : ''), [file.kind, file.mediaType, bytes]);
  const lines = useMemo(() => {
    if (file.kind === 'image' || (file.kind === 'markdown' && !source)) return [];
    const language = file.kind === 'markdown' ? 'markdown' : languageOf(file.name);
    const tooLong = text.split('\n').length > MAX_HIGHLIGHTED_LINES;
    return tokenizeLines(text, tooLong ? null : language);
  }, [file.kind, file.name, source, text]);
  if (file.kind === 'image') {
    return (
      <div className="shared-image">
        <img src={url} alt={file.name} />
      </div>
    );
  }
  if (file.kind === 'markdown' && !source) {
    return (
      <article className="doc selectable" aria-label={file.name}>
        <Markdown text={text} />
      </article>
    );
  }
  return (
    <article className="doc-source" aria-label={file.name}>
      <CodeLines lines={lines} label={file.kind === 'markdown' ? 'Markdown source' : file.name} />
    </article>
  );
}
