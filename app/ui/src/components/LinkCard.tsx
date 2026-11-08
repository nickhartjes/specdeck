import { useState } from 'react';
import { ExternalLink, Link2, Link2Off, Lock } from 'lucide-react';
import { errorMessage, useBridge } from '../api/bridge';
import type { LinkItem } from '../model/timeline';
import { checkLink, maskAfterHash, site } from '../model/links';
import { ConfirmDialog } from './ConfirmDialog';

const TIME = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });

interface Props {
  link: LinkItem;
  /** The host's: removes the link once they confirm. */
  onRemove?: (linkId: number) => void;
}

/**
 * A shared link (`shared-links` spec: Link card). It opens in the participant's own browser,
 * through the app by its id; nothing here loads the page. Title and address are text only.
 */
export function LinkCard({ link, onRemove }: Props) {
  const bridge = useBridge();
  const [note, setNote] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const label = link.title ?? (link.url && site(link.url)) ?? 'Link';
  const when = link.at ? TIME.format(link.at) : '';

  if (link.url === null) {
    return (
      <div className="link-card removed" role="group" aria-label={`Removed link: ${label}`}>
        <Link2Off aria-hidden />
        <span>
          {link.title && <strong>{link.title} · </strong>}
          The host removed this link.
        </span>
      </div>
    );
  }

  // The same check the host made; a modified host could send anything.
  const checked = checkLink(link.url);
  if (!('url' in checked)) {
    return (
      <div className="link-card removed" role="group" aria-label={`Link: ${label}`}>
        <Link2Off aria-hidden />
        <span>This link can’t be opened. It is not a web address others can reach.</span>
      </div>
    );
  }

  const open = () => void bridge.openLink(link.linkId).catch((e) => setNote(errorMessage(e)));
  const copy = async () => {
    try {
      await bridge.copyText(link.url!);
      setNote(link.sessionOnly ? 'Copied. It holds a key: share it only with people in this session.' : 'Copied.');
    } catch {
      setNote('Could not copy. Open the link and copy it from your browser.');
    }
  };

  return (
    <div className="link-card" role="group" aria-label={`Link: ${label}`}>
      <div className="link-head">
        <Link2 aria-hidden />
        <strong className="selectable">{label}</strong>
      </div>
      <span className="field-hint">
        {site(link.url)} · shared by {link.by}
        {when && ` at ${when}`}
      </span>
      <code className={`link-address${link.sessionOnly ? '' : ' selectable'}`}>{link.sessionOnly ? maskAfterHash(link.url) : link.url}</code>
      {link.sessionOnly && (
        <span className="link-secret">
          <Lock aria-hidden />
          For this session only. The part after # works as a key, so it is hidden here and will be left out of recaps and exports.
        </span>
      )}
      <span className="link-actions">
        <button className="btn primary small" onClick={open}>
          <ExternalLink aria-hidden /> Open in your browser
        </button>
        <button className="btn small" onClick={() => void copy()}>
          Copy link
        </button>
        {onRemove && (
          <button className="btn ghost small" onClick={() => setConfirm(true)}>
            Remove…
          </button>
        )}
      </span>
      {note && (
        <span className="field-hint" role="status">
          {note}
        </span>
      )}
      {confirm && onRemove && (
        <ConfirmDialog
          title="Remove this link?"
          confirm="Remove link"
          danger
          onCancel={() => setConfirm(false)}
          onConfirm={() => {
            setConfirm(false);
            onRemove(link.linkId);
          }}
        >
          <p>Nobody who joins from now on gets the address. People who already saw it still have it.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}
