import { FileDiff } from 'lucide-react';
import type { TimelineEvent } from '../api/types';
import { changesIn } from '../model/asks';
import { DiffView } from './DiffView';

/**
 * The changes in the last prompt, as the host asked to read them (`session-window` spec: Changes
 * in the last prompt): every file change in the range, in the order made, each under its file's
 * name. Built from the timeline already received; later changes are outside the range.
 */
export function ChangesTab({ timeline, fromSeq, toSeq }: { timeline: TimelineEvent[]; fromSeq: number; toSeq: number }) {
  const changes = changesIn(timeline, fromSeq, toSeq);
  if (changes.length === 0) {
    return (
      <div className="empty">
        <FileDiff aria-hidden />
        <strong>No changes</strong>
        <span>The agent changed no files in the last prompt.</span>
      </div>
    );
  }
  return (
    <div className="changes-tab">
      {changes.map((change) => (
        <DiffView key={`${change.toolUseId}:${change.outputSeq}`} change={change} />
      ))}
    </div>
  );
}
