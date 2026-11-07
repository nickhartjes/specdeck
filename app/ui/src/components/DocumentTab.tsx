import { useEffect, useState } from 'react';
import { FileText } from 'lucide-react';
import { compareBlocks, type DocumentVersion } from '../model/documents';
import type { FileChange } from '../model/timeline';
import { DiffView } from './DiffView';
import { Markdown } from './Markdown';

export interface ReaderDocument {
  title: string;
  path: string;
}

interface Props {
  path: string;
  /** The plan this document belongs to, with its documents in the schema's order. */
  plan?: { title: string; docs: ReaderDocument[] };
  versions: Map<string, DocumentVersion[]>;
  /** The version this participant last read, by path; kept by the caller across tabs. */
  lastRead: Map<string, number>;
  onRead: (path: string, version: number) => void;
  /** File changes, for a document too large to show whole. */
  changes: FileChange[];
  /** Opens another document of the plan. */
  onOpen: (path: string) => void;
  /** Switching the marks is reading: the tab is kept. */
  onKeep: () => void;
}

/** A document the agent wrote, in a tab, with what changed since this participant last read it. */
export function DocumentTab({ path, plan, versions, lastRead, onRead, changes, onOpen, onKeep }: Props) {
  const [marks, setMarks] = useState(true);
  // What the document is compared against: the version read before this tab first showed it.
  const [base] = useState(() => lastRead.get(path) ?? null);
  const latest = versions.get(path)?.at(-1) ?? null;

  // Shown is read.
  useEffect(() => {
    if (latest && lastRead.get(path) !== latest.version) onRead(path, latest.version);
  }, [path, latest, lastRead, onRead]);

  const baseText = marks && base !== null ? (versions.get(path)?.[base]?.text ?? null) : null;
  return (
    <div className="document-tab">
      <div className="filebar">
        {plan ? (
          <nav aria-label={`Plan: ${plan.title}`} className="plan-links">
            <span className="muted">Plan: {plan.title}</span>
            {plan.docs.map((doc) => (
              <button key={doc.path} className="link" aria-current={doc.path === path ? 'page' : undefined} onClick={() => doc.path !== path && onOpen(doc.path)}>
                {doc.title}
              </button>
            ))}
          </nav>
        ) : (
          <span className="path selectable">{path}</span>
        )}
        <span className="spacer" />
        <label className="check">
          <input
            type="checkbox"
            checked={marks}
            onChange={() => {
              setMarks((on) => !on);
              onKeep();
            }}
          />
          Highlight changes since you last read it
        </label>
      </div>
      <article className="doc selectable" aria-label={path.split('/').at(-1)}>
        {!latest ? (
          <div className="empty">
            <FileText aria-hidden />
            <strong>Not written yet</strong>
          </div>
        ) : latest.text === null ? (
          <>
            <p>This document is too large to show whole. Its changes:</p>
            {changes
              .filter((change) => change.path === path)
              .map((change, i) => (
                <DiffView key={i} change={change} />
              ))}
          </>
        ) : (
          compareBlocks(baseText, latest.text).map(({ block, mark }, i) => (
            <div key={i} className={`doc-block${mark ? ` mark-${mark}` : ''}`}>
              {mark && <span className="mark-tag">{mark === 'new' ? 'New' : 'Changed'}</span>}
              <Markdown text={block} />
            </div>
          ))
        )}
      </article>
    </div>
  );
}
