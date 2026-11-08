import { useState } from 'react';
import { Link2 } from 'lucide-react';
import type { HostLink, RequestOutcome, SentLink, SentLinkState } from '../api/types';
import { maskAfterHash } from '../model/links';
import { ShareLinkDialog } from './ShareLinkDialog';

type Share = (url: string, title: string | null, sessionOnly: boolean) => Promise<void>;

export type LinksBarProps =
  | {
      host: true;
      /** Links viewers sent, oldest first. */
      viewerLinks: HostLink[];
      onShare: Share;
      /** Shares a viewer's link with everyone as sent, or declines it. */
      onAnswer: (link: HostLink, share: boolean) => void;
    }
  | {
      host: false;
      hostName: string;
      /** Whether the host lets viewers send links now. */
      allowed: boolean;
      /** This viewer's links and what became of each. */
      mine: SentLink[];
      /** The host's reply to the link just sent, once it came. */
      outcome: RequestOutcome | null;
      onShare: Share;
    };

/** What became of a viewer's link, in words. */
export function linkStateText(state: SentLinkState, host: string): string {
  if (state === 'Waiting') return `Waiting for ${host}`;
  return state === 'Shared' ? `${host} shared it with everyone` : `${host} declined it`;
}

/**
 * Above the timeline (`shared-links`): sharing a link, the links viewers sent the host to share
 * or decline, and a viewer's own links.
 */
export function LinksBar(props: LinksBarProps) {
  const [sharing, setSharing] = useState(false);
  const hostName = props.host ? 'You' : props.hostName;
  const waiting = props.host ? props.viewerLinks.filter((link) => link.state === 'Waiting') : [];
  const notice = props.host
    ? null
    : props.outcome === 'Off'
      ? `${props.hostName} has turned off links from viewers. Your link was not sent.`
      : props.outcome === 'RateLimited'
        ? 'Wait a moment before sending again.'
        : null;
  const canShare = props.host || props.allowed;

  return (
    <div className="links-bar">
      <div className="share-row">
        {canShare ? (
          <button className="btn small" onClick={() => setSharing(true)}>
            <Link2 aria-hidden /> {props.host ? 'Share a link' : `Send a link to ${props.hostName}`}
          </button>
        ) : (
          <span className="field-hint">{props.hostName} has turned off links from viewers.</span>
        )}
      </div>
      {notice && (
        <p className="error-text" role="status">
          {notice}
        </p>
      )}
      {props.host && waiting.length > 0 && (
        <ul aria-label="Links waiting for you">
          {waiting.map((link) => (
            <li key={`${link.viewer}-${link.id}`} aria-label={`${link.from}’s link`}>
              <span className="field-hint">{link.from} wants to share a link</span>
              {link.title && <strong className="selectable">{link.title}</strong>}
              {/* The whole address, so the host sees exactly what everyone would get. */}
              <code className="link-address selectable">{link.url}</code>
              {link.sessionOnly && <span className="field-hint">For this session only</span>}
              <span className="link-actions">
                <button className="btn primary small" onClick={() => props.onAnswer(link, true)}>
                  Share
                </button>
                <button className="btn ghost small" onClick={() => props.onAnswer(link, false)}>
                  Decline
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}
      {!props.host && props.mine.length > 0 && (
        <ul aria-label="Your links">
          {[...props.mine].reverse().map((link) => (
            <li key={link.id}>
              <span className="selectable">{link.title ?? (link.sessionOnly ? maskAfterHash(link.url) : link.url)}</span>
              <span className="field-hint">{linkStateText(link.state, hostName)}</span>
            </li>
          ))}
        </ul>
      )}
      {sharing && (
        <ShareLinkDialog
          host={props.host}
          hostName={props.host ? undefined : props.hostName}
          onShare={props.onShare}
          onClose={() => setSharing(false)}
        />
      )}
    </div>
  );
}
