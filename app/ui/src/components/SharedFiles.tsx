import { useEffect, useMemo, useRef, useState } from 'react';
import { Download, FileText, FolderInput, Image, Share2, Trash2, Upload } from 'lucide-react';
import { errorMessage } from '../api/bridge';
import type { SharedFileInfo } from '../api/types';
import { languageOf, tokenizeLines } from '../model/highlight';
import { dataUrl, formatSize, refusal, SHAREABLE, sharedBy, textOf } from '../model/sharedFiles';
import { CodeLines } from './CodeLines';
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
}

/**
 * Checks `files` and shares those that may go, one by one; says why for each one that may not.
 * The host decides again from the content.
 */
export async function shareEach(files: File[], send: (name: string, bytes: ArrayBuffer) => Promise<unknown>, say: (message: string) => void) {
  for (const file of files) {
    const refused = refusal(file);
    if (refused) {
      say(refused);
      continue;
    }
    try {
      await send(file.name, await file.arrayBuffer());
      say(`${file.name} is shared with everyone.`);
    } catch (error) {
      say(`${file.name}: ${errorMessage(error)}`);
    }
  }
}

const iconOf = (file: SharedFileInfo) => (file.kind === 'image' ? Image : FileText);
const TIME = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });

/** The Shared tab of the files panel: for now the group Shared with everyone (`session-window` spec: Shared tab). */
export function SharedTab({ shared, active, onOpen }: { shared: SharedFiles; active: number | null; onOpen: (file: SharedFileInfo, keep: boolean) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const [over, setOver] = useState(false);
  const newestFirst = useMemo(() => [...shared.files].reverse(), [shared.files]);
  const take = (files: FileList | null) => {
    if (files && files.length > 0) shared.share([...files]);
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
            <button className="btn small" onClick={() => input.current?.click()} title={`${SHAREABLE}. Or drop them here.`}>
              <Upload aria-hidden /> Share files
            </button>
          </>
        ) : (
          <span className="field-hint">{shared.hostName} has not turned on sharing files.</span>
        )}
      </div>
    </div>
  );
}

/** What came of fetching file `id`; another file than the one shown counts as still loading. */
type Loaded = { id: number } & ({ state: 'ready'; bytes: ArrayBuffer } | { state: 'failed'; message: string });

/** A shared file in a tab: an image, Markdown or text, with who shared it and what can be done with it. */
export function SharedFileTab({
  file,
  shared,
  progress,
  onKeep,
}: {
  /** Null once the file was removed. */
  file: SharedFileInfo | null;
  shared: SharedFiles;
  /** Bytes received so far, while it is fetched. */
  progress?: number;
  onKeep: () => void;
}) {
  const [result, setLoaded] = useState<Loaded | null>(null);
  const [source, setSource] = useState(false);
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
          From {sharedBy(file, shared.me).replace(/^You$/, 'you')}, {TIME.format(file.atMs)}, {formatSize(file.size)}. Shared with everyone.
        </span>
        <span className="spacer" />
        {file.kind === 'markdown' && ready && (
          <div className="toggle" role="group" aria-label="How to show the document">
            <button type="button" aria-pressed={!source} onClick={() => (setSource(false), onKeep())}>
              Formatted
            </button>
            <button type="button" aria-pressed={source} onClick={() => (setSource(true), onKeep())}>
              Source
            </button>
          </div>
        )}
        {shared.host && shared.put && (
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
      ) : (
        <SharedContent file={file} bytes={ready} source={source} />
      )}
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
