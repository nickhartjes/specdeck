import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { languageOf, tokenizeLines } from '../model/highlight';
import { wholeFile, type WholeFile } from '../model/lineDiff';
import type { TouchedFile } from '../model/timeline';
import { CodeLines } from './CodeLines';
import { DiffView } from './DiffView';

export type CodeView = 'changes' | 'whole';

interface Props {
  file: TouchedFile;
  /** The tool call whose change to scroll to and highlight; `n` changes to do it again. */
  focus?: { toolUseId: string; n: number };
  /** Scrolls the terminal to where a change happened. */
  onJump: (outputSeq: number) => void;
  /** The file's whole text after each change it was shared for, by tool call. */
  texts?: Map<string, string | null>;
  /** Whether the host shares whole code files now. */
  wholeFiles?: boolean;
  /** The host's own window, which words the reasons for the host. */
  host?: boolean;
  /** The view this file opens on; a step from the timeline always shows its change. */
  view?: CodeView;
  /** The participant chose a view: remembered for the file, and the preview tab is kept. */
  onView?: (view: CodeView) => void;
}

/** Why the whole file can't be shown, in plain words. */
function unavailableText(whole: Extract<WholeFile, { unavailable: unknown }>, wholeFiles: boolean, host: boolean): string {
  switch (whole.unavailable) {
    case 'deleted':
      return 'Deleted in this session.';
    case 'too-large':
      return 'Too large to show whole.';
    case 'no-text':
      if (!wholeFiles)
        return host
          ? 'You share only the changes. To let everyone open whole files, change it in the sharing settings.'
          : 'The host shares only the changes.';
      return 'Not shared whole: it changed while only the changes were shared, or it is private, ignored by git, outside the session folder, or not text.';
  }
}

/** Every change the agent made to one file, in order, as diffs; or the whole file, changes marked. */
export function DiffTab({ file, focus, onJump, texts, wholeFiles = false, host = false, view = 'changes', onView }: Props) {
  const list = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState<CodeView>(view);
  const reasonId = useId();
  // A step from the timeline shows its change, whatever this file was left on; adjusted while rendering.
  const [seenFocus, setSeenFocus] = useState(focus);
  if (focus !== seenFocus) {
    setSeenFocus(focus);
    if (focus) setShown('changes');
  }
  useEffect(() => {
    if (!focus || shown !== 'changes') return;
    const element = list.current?.querySelector<HTMLElement>(`[data-tool="${CSS.escape(focus.toolUseId)}"]`);
    if (!element) return;
    element.scrollIntoView?.({ block: 'start' });
    element.classList.remove('flash');
    // Restarts the highlight when the same change is chosen again.
    void element.offsetWidth;
    element.classList.add('flash');
  }, [focus, shown]);

  const whole = useMemo(() => wholeFile(file.changes, texts ?? new Map()), [file.changes, texts]);
  const lines = useMemo(() => ('lines' in whole ? tokenizeLines(whole.lines.join('\n'), languageOf(file.path)) : []), [whole, file.path]);
  const available = 'lines' in whole;
  const deleted = 'unavailable' in whole && whole.unavailable === 'deleted';
  const reason = 'lines' in whole ? null : unavailableText(whole, wholeFiles, host);
  const choose = (next: CodeView) => {
    setShown(next);
    onView?.(next);
  };

  return (
    <div className="diff-tab">
      <div className="filebar">
        <span className="path selectable">{file.path}</span>
        <span className="muted">
          {file.changes.length === 1 ? '1 change' : `${file.changes.length} changes`} in this session
        </span>
        <span className="spacer" />
        {file.changes.length > 0 && !deleted && (
          <div className="toggle" role="group" aria-label="How to show the file">
            <button type="button" aria-pressed={!(shown === 'whole' && available)} onClick={() => choose('changes')}>
              Changes
            </button>
            <button
              type="button"
              aria-pressed={shown === 'whole' && available}
              aria-disabled={!available}
              aria-describedby={reason ? reasonId : undefined}
              className={available ? undefined : 'unavailable'}
              onClick={() => available && choose('whole')}
            >
              Whole file, changes marked
            </button>
          </div>
        )}
      </div>
      {reason && !deleted && (
        <p className="whole-note" id={reasonId}>
          {reason}
        </p>
      )}
      {shown === 'whole' && 'lines' in whole ? (
        <>
          {whole.markedFrom > 0 && (
            <p className="whole-note">
              {whole.markedFrom === 1 ? 'The first change is' : `The first ${whole.markedFrom} changes are`} not marked: the file was not shared whole
              then.
            </p>
          )}
          <CodeLines lines={lines} marks={whole.marks} removed={whole.removed} label={`${file.path}, whole file`} />
        </>
      ) : (
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
      )}
    </div>
  );
}
