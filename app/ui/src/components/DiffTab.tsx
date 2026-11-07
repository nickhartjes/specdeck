import { useEffect, useRef } from 'react';
import type { TouchedFile } from '../model/timeline';
import { DiffView } from './DiffView';

interface Props {
  file: TouchedFile;
  /** The tool call whose change to scroll to and highlight; `n` changes to do it again. */
  focus?: { toolUseId: string; n: number };
  /** Scrolls the terminal to where a change happened. */
  onJump: (outputSeq: number) => void;
}

/** Every change the agent made to one file, in order, as diffs. */
export function DiffTab({ file, focus, onJump }: Props) {
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!focus) return;
    const element = list.current?.querySelector<HTMLElement>(`[data-tool="${CSS.escape(focus.toolUseId)}"]`);
    if (!element) return;
    element.scrollIntoView?.({ block: 'start' });
    element.classList.remove('flash');
    // Restarts the highlight when the same change is chosen again.
    void element.offsetWidth;
    element.classList.add('flash');
  }, [focus]);
  return (
    <div className="diff-tab">
      <div className="filebar">
        <span className="path selectable">{file.path}</span>
        <span className="muted">
          {file.changes.length === 1 ? '1 change' : `${file.changes.length} changes`} in this session
        </span>
      </div>
      <div ref={list} className="diff-list">
        {file.changes.map((change, i) => (
          <section key={i} data-tool={change.toolUseId} aria-label={`Change ${i + 1} of ${file.changes.length}`}>
            <button className="btn ghost small" onClick={() => onJump(change.outputSeq)}>
              Change {i + 1} of {file.changes.length} · show in terminal
            </button>
            <DiffView change={change} showPath={false} />
          </section>
        ))}
      </div>
    </div>
  );
}
