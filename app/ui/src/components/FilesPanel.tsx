import { Eye, FilePen, FilePlus, FileText, FileX, Files, X } from 'lucide-react';
import type { SharedFileInfo } from '../api/types';
import type { FilesTab } from '../model/layout';
import type { TouchedFile } from '../model/timeline';
import { SharedTab, type SharedFiles } from './SharedFiles';

interface Props {
  files: TouchedFile[];
  timelineSupported: boolean;
  agentName: string;
  /** A terminal session, which never lists files. */
  terminal?: boolean;
  tab: FilesTab;
  onTab: (tab: FilesTab) => void;
  /** The file shown in the tab area, marked in the list. */
  active: string | null;
  /** A click opens a preview; a double click keeps it open. */
  onOpen: (file: TouchedFile, keep: boolean) => void;
  onClose: () => void;
  /** Files shared with everyone; absent where the session takes none, which then has no Shared tab. */
  shared?: SharedFiles;
  /** The shared file shown in the tab area, marked in the list. */
  activeShared?: number | null;
  onOpenShared?: (file: SharedFileInfo, keep: boolean) => void;
}

export const isDocument = (path: string) => /\.(md|markdown)$/i.test(path);

type Kind = 'read' | 'deleted' | 'new' | 'changed';

function status(file: TouchedFile): { tag: string; kind: Kind; Icon: typeof Eye } {
  if (file.readOnly) return { tag: 'read', kind: 'read', Icon: Eye };
  const created = file.changes.some((c) => c.change === 'created');
  if (file.changes.at(-1)?.change === 'deleted') return { tag: 'deleted', kind: 'deleted', Icon: FileX };
  const kind = created ? 'new' : 'changed';
  return { tag: file.changes.length === 1 ? kind : `${file.changes.length} changes`, kind, Icon: created ? FilePlus : FilePen };
}

/**
 * The files the agent touched, as Documents and Code (`session-window` spec: Files the agent
 * touched), and the files people shared (Shared tab).
 */
export function FilesPanel({ files, timelineSupported, agentName, terminal = false, tab: chosen, onTab, active, onOpen, onClose, shared, activeShared = null, onOpenShared }: Props) {
  const documents = files.filter((f) => isDocument(f.path));
  const code = files.filter((f) => !isDocument(f.path));
  // A session that takes no shared files shows Code where Shared was chosen.
  const tab = chosen === 'shared' && !shared ? 'code' : chosen;
  const shown = tab === 'documents' ? documents : code;
  const tabs = [
    ['documents', 'Documents', timelineSupported ? documents.length : null],
    ['code', 'Code', timelineSupported ? code.length : null],
    ...(shared ? [['shared', 'Shared', shared.files.length + (shared.braindump?.items.length ?? 0)] as const] : []),
  ] as const;
  const label = tabs.find(([id]) => id === tab)?.[1] ?? 'Code';
  return (
    <>
      <div className="pane-head">
        <div role="tablist" aria-label="Files" className="pane-tabs">
          {tabs.map(([id, name, count], index) => (
            <button
              key={id}
              role="tab"
              aria-selected={tab === id}
              tabIndex={tab === id ? 0 : -1}
              className="pane-tab"
              onClick={() => onTab(id)}
              onKeyDown={(event) => {
                if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
                const next = tabs[(index + (event.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length]!;
                onTab(next[0]);
              }}
            >
              <span className="label">{name}</span>
              {count !== null && <span className="count">{count}</span>}
            </button>
          ))}
        </div>
        <span className="spacer" />
        <button className="btn ghost small icon" aria-label="Hide files" title="Hide files" onClick={onClose}>
          <X aria-hidden />
        </button>
      </div>
      <div className="pane-body" role="tabpanel" aria-label={label}>
        {tab === 'shared' && shared ? (
          <SharedTab shared={shared} active={activeShared} onOpen={(file, keep) => onOpenShared?.(file, keep)} />
        ) : !timelineSupported ? (
          <div className="empty">
            <Files aria-hidden />
            <strong>{terminal ? 'No file list for a terminal session' : `No file list for ${agentName}`}</strong>
            <span>This session’s files can’t be listed: touched files are listed for Claude Code sessions.</span>
          </div>
        ) : shown.length === 0 ? (
          <div className="empty">
            <Files aria-hidden />
            <strong>{tab === 'documents' ? 'No documents yet' : 'No code touched yet'}</strong>
            <span>
              {tab === 'documents' ? 'Plans, specs and notes the agent writes are listed here, to read whole.' : 'Files the agent reads or changes are listed here, with their diffs.'}
            </span>
          </div>
        ) : (
          <ul className="files" aria-label={tab === 'documents' ? 'Documents' : 'Code'}>
            {shown.map((file) => {
              const { tag, kind, Icon } = status(file);
              const name = file.path.split('/').at(-1) ?? file.path;
              const folder = file.path.slice(0, file.path.length - name.length).replace(/\/$/, '');
              return (
                <li key={file.path}>
                  <button
                    aria-current={active === file.path || undefined}
                    title={file.readOnly ? `${file.path}: only read; opens the terminal where it was read` : file.path}
                    aria-label={`${file.path}, ${tag}`}
                    onClick={() => onOpen(file, false)}
                    onDoubleClick={() => onOpen(file, true)}
                  >
                    {isDocument(file.path) && !file.readOnly ? <FileText aria-hidden /> : <Icon aria-hidden />}
                    <span className="name">{name}</span>
                    {folder && <span className="folder">{folder}</span>}
                    <span className={`tag ${kind}`}>{tag}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </>
  );
}
