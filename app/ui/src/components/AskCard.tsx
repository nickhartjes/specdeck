import { BookOpenCheck, Clock } from 'lucide-react';
import type { AskItem } from '../model/timeline';
import { CARD_SETS, spreadText } from '../model/asks';
import { CardFace } from './VoteBanner';

/** "From M to L" as the middle of a sentence. */
const inSentence = (text: string) => text.charAt(0).toLowerCase() + text.slice(1);

const TIME = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });

/**
 * A read request that closed or an estimate the host kept, in the timeline (`session-timeline`:
 * Read requests and estimates in the timeline). Counts only: no names, question texts or votes.
 */
export function AskCard({ item }: { item: AskItem }) {
  const when = item.at ? TIME.format(item.at) : '';
  const entry = item.entry;
  if (entry.kind === 'read-done') {
    const counts = [`${entry.ready} of ${entry.of} ready`, entry.questions > 0 && `${entry.questions} with a question`].filter(Boolean).join(', ');
    return (
      <div className="ask-card" role="group" aria-label={`Read request: ${entry.label}`}>
        <div className="ask-card-head">
          <BookOpenCheck aria-hidden />
          <strong className="selectable">Read {entry.label}</strong>
          <span className="field-hint ask-when">{when}</span>
        </div>
        <span>
          {counts}
          {entry.timeUp && (
            <>
              {' '}
              · <Clock aria-hidden className="inline-icon" /> the time was up
            </>
          )}
        </span>
      </div>
    );
  }
  const set = CARD_SETS[entry.cards];
  return (
    <div className="ask-card estimate" role="group" aria-label={`Estimate: ${entry.task}`}>
      <CardFace card={entry.kept} set={entry.cards} small />
      <div className="ask-card-body">
        <div className="ask-card-head">
          <strong className="selectable ask-task">{entry.task}</strong>
          <span className="field-hint ask-when">{when}</span>
        </div>
        <span className="field-hint">
          Estimated in <span className="nowrap">{set.inSentence}</span> · {inSentence(spreadText(entry.low, entry.high, entry.cards))} ·{' '}
          {entry.votes === 1 ? '1 vote' : `${entry.votes} votes`}
        </span>
      </div>
    </div>
  );
}
