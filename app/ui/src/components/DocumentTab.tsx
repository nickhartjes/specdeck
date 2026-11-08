import { useEffect, useMemo, useRef, useState } from 'react';
import { FileText } from 'lucide-react';
import { blocksWithLines, compareBlocks, sourceMarks, type DocumentVersion } from '../model/documents';
import { tokenizeLines } from '../model/highlight';
import type { FileChange } from '../model/timeline';
import { CodeLines } from './CodeLines';
import { DiffView } from './DiffView';
import { Markdown } from './Markdown';

const SOURCE_LABELS = { added: 'New since you last read it', changed: 'Changed since you last read it' };

/** The first element in `root` with `selector` that is still in view of the scrolling tab. */
function topmost(root: HTMLElement | null, selector: string): HTMLElement | null {
  if (!root) return null;
  const top = (root.closest('.tab-panel') ?? root).getBoundingClientRect().top;
  const all = [...root.querySelectorAll<HTMLElement>(selector)];
  return all.find((element) => element.getBoundingClientRect().bottom > top) ?? null;
}

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
  // A document always opens formatted (`plan-documents`: Formatted or source).
  const [source, setSource] = useState(false);
  const article = useRef<HTMLElement>(null);
  // The line to bring back into view after switching, so the reader stays at the same part.
  const scrollTo = useRef<number | null>(null);
  // What the document is compared against: the version read before this tab first showed it.
  const [base] = useState(() => lastRead.get(path) ?? null);
  const latest = versions.get(path)?.at(-1) ?? null;

  // Shown is read.
  useEffect(() => {
    if (latest && lastRead.get(path) !== latest.version) onRead(path, latest.version);
  }, [path, latest, lastRead, onRead]);

  const baseText = marks && base !== null ? (versions.get(path)?.[base]?.text ?? null) : null;
  const text = latest?.text ?? null;
  const blocks = useMemo(() => (text === null ? [] : compareBlocks(baseText, text)), [baseText, text]);
  const blockLines = useMemo(() => (text === null ? [] : blocksWithLines(text).map((block) => block.first)), [text]);
  const sourceLines = useMemo(() => (source && text !== null ? tokenizeLines(text, 'markdown') : []), [source, text]);
  const lineMarks = useMemo(
    () => (source && text !== null ? sourceMarks(baseText, text).map((mark) => (mark === 'new' ? 'added' : mark)) : []),
    [source, baseText, text],
  );

  useEffect(() => {
    const line = scrollTo.current;
    if (line === null) return;
    scrollTo.current = null;
    const target = source
      ? article.current?.querySelector<HTMLElement>(`[data-line="${line}"]`)
      : [...(article.current?.querySelectorAll<HTMLElement>('[data-first-line]') ?? [])].findLast((el) => Number(el.dataset.firstLine) <= line);
    target?.scrollIntoView?.({ block: 'start' });
  }, [source]);

  const switchTo = (toSource: boolean) => {
    if (toSource === source) return;
    // The part at the top now: a block's first line, or a source line.
    const top = toSource ? topmost(article.current, '[data-first-line]')?.dataset.firstLine : topmost(article.current, '[data-line]')?.dataset.line;
    scrollTo.current = top === undefined ? null : Number(top) + (toSource ? 1 : -1);
    setSource(toSource);
    onKeep();
  };

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
        {text !== null && (
          <div className="toggle" role="group" aria-label="How to show the document">
            <button type="button" aria-pressed={!source} onClick={() => switchTo(false)}>
              Formatted
            </button>
            <button type="button" aria-pressed={source} onClick={() => switchTo(true)}>
              Source
            </button>
          </div>
        )}
      </div>
      <article ref={article} className={source ? 'doc-source' : 'doc selectable'} aria-label={path.split('/').at(-1)}>
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
        ) : source ? (
          <CodeLines lines={sourceLines} marks={lineMarks} labels={SOURCE_LABELS} label="Markdown source" />
        ) : (
          blocks.map(({ block, mark }, i) => (
            <div key={i} className={`doc-block${mark ? ` mark-${mark}` : ''}`} data-first-line={blockLines[i]}>
              {mark && <span className="mark-tag">{mark === 'new' ? 'New' : 'Changed'}</span>}
              <Markdown text={block} />
            </div>
          ))
        )}
      </article>
    </div>
  );
}
