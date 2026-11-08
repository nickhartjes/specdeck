import { useId, useState } from 'react';
import { Check, Gauge } from 'lucide-react';
import type { Card, CardSet, Participant, VoteView } from '../api/types';
import { cardLabel, cardName, cardsOf, proposedCard, sameCard, spreadOf, spreadText } from '../model/asks';
import { nameOf } from './ReadBanner';

interface Props {
  view: VoteView;
  /** Everyone in the session now; those on an older app (`view.older`) get no seat. */
  participants: Participant[];
  /** This participant: 0 for the host. */
  me: number;
  /** Votes, or takes the vote back with null. */
  onVote: (card: Card | null) => void;
  /** The host's controls; absent for a viewer. */
  host?: {
    onReveal: () => void;
    onVoteAgain: () => void;
    onStop: () => void;
    onKeep: (card: Card) => void;
  };
  /** Why the last vote did not count, if it did not. */
  notice?: string | null;
}

/** A card face, as on the table and in the timeline. */
export function CardFace({ card, set, small = false }: { card: Card; set: CardSet; small?: boolean }) {
  const label = cardLabel(card, set);
  return (
    <span className={`card-face${small ? ' small' : ''}${label.length > 1 ? ' wide' : ''}`} aria-label={cardName(card, set)} role="img">
      {label}
    </span>
  );
}

/** One seat at the table: empty until they vote, face down until the reveal. */
function Seat({ name, card, voted, set, revealed }: { name: string; card: Card | null; voted: boolean; set: CardSet; revealed: boolean }) {
  const state = revealed ? (card ? 'shown' : 'none') : voted ? 'down' : 'empty';
  const said = revealed ? (card ? cardName(card, set) : 'no vote') : voted ? 'voted' : 'not voted yet';
  return (
    <li className={`seat seat-${state}`} aria-label={`${name}: ${said}`}>
      <span className="seat-card" aria-hidden>
        <span className="seat-flip">
          <span className="seat-back" />
          <span className="seat-front">{card ? cardLabel(card, set) : '–'}</span>
        </span>
      </span>
      <span className="seat-name">{name}</span>
    </li>
  );
}

/** An open vote in the request row (`host-requests`: Hidden votes, Reveal and spread, Vote again, stop or keep). */
export function VoteBanner({ view, participants, me, onVote, host, notice }: Props) {
  const ids = useId();
  const set = view.cards;
  const revealed = view.revealed;
  const votes = revealed?.map(([, card]) => card) ?? [];
  const proposed = proposedCard(votes);
  const [kept, setKept] = useState<number | null>(null);
  const cards = cardsOf(set);
  const keepIndex = kept ?? Math.max(0, cards.findIndex((card) => sameCard(card, proposed)));
  const spread = spreadOf(votes);
  // Everyone who can vote, in the session's order; someone who voted and left is not at the table.
  const seats = participants.filter((p) => !view.older.includes(p.id));
  const cardOf = (id: number) => revealed?.find(([who]) => who === id)?.[1] ?? null;
  const older = view.older.map((id) => nameOf(participants, id));

  return (
    <section className="ask-banner vote-banner" aria-labelledby={`${ids}-title`}>
      <div className="ask-banner-head">
        <Gauge aria-hidden />
        <strong id={`${ids}-title`}>
          {revealed ? 'Votes' : 'How big is it?'}
          {view.round > 1 && <span className="field-hint"> · round {view.round}</span>}
        </strong>
        <span className="selectable ask-task">{view.task}</span>
      </div>
      <div className="vote-body">
        {!revealed && (
          <div className="vote-hand" role="group" aria-label="Your vote">
            {cards.map((card, i) => {
              const picked = sameCard(view.mine, card);
              const label = cardLabel(card, set);
              return (
                <button
                  key={i}
                  className={`vote-card${picked ? ' on' : ''}${label.length > 1 ? ' wide' : ''}`}
                  aria-pressed={picked}
                  aria-label={cardName(card, set)}
                  title={picked ? `${cardName(card, set)}: click again to take your vote back` : cardName(card, set)}
                  onClick={() => onVote(picked ? null : card)}
                >
                  {label}
                </button>
              );
            })}
          </div>
        )}
        <ul className={`vote-table${revealed ? ' revealed' : ''}`} aria-label={revealed ? 'Votes' : 'Who voted'}>
          {seats.map((p) => (
            <Seat key={p.id} name={nameOf(participants, p.id, me)} card={cardOf(p.id)} voted={view.voted.includes(p.id)} set={set} revealed={revealed !== null} />
          ))}
        </ul>
      </div>
      <p className="ask-note">
        {revealed ? (
          <strong className="vote-spread">{spreadText(spread?.low ?? null, spread?.high ?? null, set)}</strong>
        ) : view.voted.length === 0 ? (
          'No votes yet. Nobody sees a card until the host reveals them.'
        ) : (
          `${view.voted.length} of ${seats.length} voted. Nobody sees a card until the host reveals them.`
        )}
        {older.length > 0 && ` ${older.join(', ')} can’t vote: on an older app.`}
      </p>
      {host && (
        <div className="ask-actions">
          {!revealed ? (
            <button className="btn small primary" onClick={host.onReveal} disabled={view.voted.length === 0}>
              Reveal the votes
            </button>
          ) : (
            <>
              <label className="keep">
                <span className="visually-hidden">Keep</span>
                <select className="field" value={keepIndex} onChange={(event) => setKept(Number(event.target.value))}>
                  {cards.map((card, i) => (
                    <option key={i} value={i}>
                      {cardLabel(card, set)}
                    </option>
                  ))}
                </select>
              </label>
              <button className="btn small primary" onClick={() => host.onKeep(cards[keepIndex]!)}>
                <Check aria-hidden /> Keep {cardLabel(cards[keepIndex]!, set)}
              </button>
              <button className="btn small" onClick={host.onVoteAgain}>
                Vote again
              </button>
            </>
          )}
          <button className="btn small" onClick={host.onStop}>
            Stop voting
          </button>
        </div>
      )}
      {!host && revealed && <p className="field-hint">Waiting for the host to keep a value or vote again.</p>}
      {notice && (
        <p className="error-text" role="alert">
          {notice}
        </p>
      )}
    </section>
  );
}
