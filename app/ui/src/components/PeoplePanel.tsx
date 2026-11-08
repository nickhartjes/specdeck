import { useState } from 'react';
import { Minus, Plus, UserMinus } from 'lucide-react';
import type { Participant } from '../api/types';
import { ConfirmDialog } from './ConfirmDialog';

interface Props {
  participants: Participant[];
  /** The viewer's own id, to mark "you". */
  me?: number;
  /** Host only: remove a viewer and change the limit. */
  onRemove?: (participant: Participant) => void;
  viewerLimit?: number;
  onViewerLimit?: (limit: number) => void;
  agentName?: string;
}

export function initials(name: string): string {
  return name.trim().split(/\s+/).map((part) => part[0]).join('').slice(0, 2).toUpperCase() || '?';
}

export function PeoplePanel({ participants, me, onRemove, viewerLimit, onViewerLimit }: Props) {
  const [removing, setRemoving] = useState<Participant | null>(null);
  const viewers = participants.filter((p) => p.role === 'Viewer').length;
  return (
    <div style={{ display: 'grid', gridTemplateRows: '1fr auto auto', minHeight: '100%' }}>
      <ul className="people" aria-label="People in this session">
        {participants.map((p) => (
          <li key={p.id}>
            <span className="avatar" aria-hidden>
              {initials(p.username)}
            </span>
            <span className="who">
              <strong className="selectable">
                {p.label}
                {p.id === me ? ' (you)' : ''}
              </strong>
              <span>{p.role === 'Host' ? 'Host · controls the session' : 'Watching'}</span>
            </span>
            {onRemove && p.role === 'Viewer' && (
              <button className="btn ghost small remove" onClick={() => setRemoving(p)} aria-label={`Remove ${p.label}`}>
                <UserMinus aria-hidden /> Remove
              </button>
            )}
          </li>
        ))}
        {participants.length <= 1 && (
          <li style={{ color: 'var(--text-muted)' }}>Nobody is watching yet. Share the invite code to let people join.</li>
        )}
      </ul>
      {onViewerLimit && viewerLimit !== undefined && (
        <div className="limit">
          <span>
            Max viewers
            <span className="field-hint"> · {viewers} watching</span>
          </span>
          <span className="stepper">
            <button onClick={() => onViewerLimit(viewerLimit - 1)} disabled={viewerLimit <= 1} aria-label="Fewer viewers">
              <Minus size={13} aria-hidden />
            </button>
            <output aria-live="polite" aria-label="Maximum viewers">
              {viewerLimit}
            </output>
            <button onClick={() => onViewerLimit(viewerLimit + 1)} disabled={viewerLimit >= 20} aria-label="More viewers">
              <Plus size={13} aria-hidden />
            </button>
          </span>
        </div>
      )}
      {removing && (
        <ConfirmDialog
          title={`Remove ${removing.label}?`}
          confirm="Remove"
          danger
          onCancel={() => setRemoving(null)}
          onConfirm={() => {
            onRemove?.(removing);
            setRemoving(null);
          }}
        >
          <p>They are disconnected right away. The invite code changes, so they cannot join again with it; share the new code with anyone still to join.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}
