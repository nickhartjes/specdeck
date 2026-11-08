import { useEffect, useId, useRef, useState } from 'react';
import { SlidersHorizontal } from 'lucide-react';

interface Props {
  /** Whether viewers can open whole code files, with its setting; absent for a session without a timeline. */
  wholeFiles?: { on: boolean; onChange: (on: boolean) => void };
  /** Whether viewers may suggest prompts, with its switch; absent where they cannot. */
  suggestions?: { allowed: boolean; onChange: (on: boolean) => void };
  /** Whether viewers may send links, with its switch; absent for a session that can't share links. */
  links?: { allowed: boolean; onChange: (on: boolean) => void };
  agentName?: string;
}

/**
 * The host's sharing settings (`session-window` spec: Sharing settings): what viewers may see and
 * do, from a button in the toolbar. Not shown when there is nothing to set, as in a terminal
 * session.
 */
export function SharingSettings({ wholeFiles, suggestions, links, agentName = 'the agent' }: Props) {
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const selectId = useId();

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      setOpen(false);
      button.current?.focus();
    };
    const onDown = (event: MouseEvent) => {
      const target = event.target as Element;
      if (!panel.current?.contains(target) && !button.current?.contains(target)) setOpen(false);
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('mousedown', onDown);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('mousedown', onDown);
    };
  }, [open]);

  if (!wholeFiles && !suggestions && !links) return null;
  return (
    <div className="sharing-menu">
      <button
        ref={button}
        className="btn ghost icon"
        aria-expanded={open}
        aria-controls="sharing-panel"
        aria-label="Sharing settings"
        title="Sharing settings"
        onClick={() => setOpen((o) => !o)}
      >
        <SlidersHorizontal aria-hidden />
      </button>
      {open && (
        <div ref={panel} id="sharing-panel" className="menu-panel sharing-panel" role="dialog" aria-label="What viewers may see and do">
          <strong>What viewers may see and do</strong>
          {wholeFiles && (
            <div className="setting">
              <label htmlFor={selectId}>Code viewers can open</label>
              <select id={selectId} value={wholeFiles.on ? 'whole' : 'changes'} onChange={(event) => wholeFiles.onChange(event.target.value === 'whole')}>
                <option value="changes">Only the changes {agentName} makes</option>
                <option value="whole">Changed files, whole, with changes marked</option>
              </select>
              <span className="field-hint">
                From the next change on. Files outside the session folder, ignored by git or private, such as <code>.env</code>, are never
                shared whole.
              </span>
            </div>
          )}
          {suggestions && (
            <label className="check">
              <input type="checkbox" checked={suggestions.allowed} onChange={(event) => suggestions.onChange(event.target.checked)} />
              <span>
                Viewers can suggest prompts
                <span className="field-hint"> · nothing reaches {agentName} until you send it</span>
              </span>
            </label>
          )}
          {links && (
            <label className="check">
              <input type="checkbox" checked={links.allowed} onChange={(event) => links.onChange(event.target.checked)} />
              <span>
                Viewers can share links
                <span className="field-hint"> · you see each one first, and share or decline it</span>
              </span>
            </label>
          )}
          <p className="field-hint">Anything the terminal shows is shared either way.</p>
        </div>
      )}
    </div>
  );
}
