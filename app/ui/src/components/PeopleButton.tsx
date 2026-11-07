import { useEffect, useRef, useState } from 'react';
import type { Participant } from '../api/types';
import { initials, PeoplePanel } from './PeoplePanel';

interface Props {
  participants: Participant[];
  me?: number;
  onRemove?: (participant: Participant) => void;
  viewerLimit?: number;
  onViewerLimit?: (limit: number) => void;
}

/** Shown on the avatars: "Nick (host), Anna and Bram watching". */
export function whoIsHere(participants: Participant[]): string {
  const host = participants.find((p) => p.role === 'Host');
  const viewers = participants.filter((p) => p.role === 'Viewer').map((p) => p.label);
  const watching = viewers.length === 0 ? 'nobody watching yet' : `${viewers.length > 1 ? `${viewers.slice(0, -1).join(', ')} and ${viewers.at(-1)}` : viewers[0]} watching`;
  return host ? `${host.label} (host), ${watching}` : watching;
}

/** The people in the session as avatars in the title bar; they open the people list. */
export function PeopleButton({ participants, me, onRemove, viewerLimit, onViewerLimit }: Props) {
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const shown = participants.slice(0, 4);
  const more = participants.length - shown.length;
  const label = whoIsHere(participants);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      // A dialog inside, such as the question before removing someone, handles its own Escape.
      if (event.key !== 'Escape' || document.querySelector('.overlay')) return;
      setOpen(false);
      button.current?.focus();
    };
    const onDown = (event: MouseEvent) => {
      const target = event.target as Element;
      if (!panel.current?.contains(target) && !button.current?.contains(target) && !target.closest?.('.overlay')) setOpen(false);
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('mousedown', onDown);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('mousedown', onDown);
    };
  }, [open]);

  return (
    <div className="people-menu">
      <button
        ref={button}
        className="avatars"
        aria-expanded={open}
        aria-controls="people-panel"
        aria-label={`People: ${label}`}
        title={label}
        onClick={() => setOpen((o) => !o)}
      >
        {shown.map((p) => (
          <span key={p.id} className={`avatar${p.role === 'Host' ? ' host' : ''}`} aria-hidden>
            {initials(p.username)}
          </span>
        ))}
        {more > 0 && (
          <span className="avatar more" aria-hidden>
            +{more}
          </span>
        )}
      </button>
      {open && (
        <div ref={panel} id="people-panel" className="menu-panel people-panel" role="dialog" aria-label="People in this session">
          <PeoplePanel participants={participants} me={me} onRemove={onRemove} viewerLimit={viewerLimit} onViewerLimit={onViewerLimit} />
        </div>
      )}
    </div>
  );
}
