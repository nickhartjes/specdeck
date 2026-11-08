import { useState, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { errorMessage, useBridge } from '../api/bridge';
import { ranFor, type SoFar } from '../model/catchUp';
import type { LinkItem, Turn } from '../model/timeline';
import { PromptPicker } from './PromptPicker';

/** Each list shows this many, then how many more there are. */
const SHOWN = 5;

interface Props {
  soFar: SoFar;
  turns: Turn[];
  /** The ids of files still shared; others are left out. */
  sharedNow?: Set<number>;
  /** The session's links; removed ones are left out. */
  links: LinkItem[];
  onJump: (promptId: string) => void;
  onLatest: () => void;
  onClose: () => void;
  onReadPlan?: (change: string, paths: string[]) => void;
  onOpenDocument?: (path: string) => void;
  onOpenShared?: (file: number, name: string) => void;
}

const base = (path: string) => path.slice(path.lastIndexOf('/') + 1);

/** What happened before a late joiner came in (`viewer-experience` spec: So far overview). */
export function CatchUpCard({ soFar, turns, sharedNow, links, onJump, onLatest, onClose, onReadPlan, onOpenDocument, onOpenShared }: Props) {
  const bridge = useBridge();
  const [note, setNote] = useState<string | null>(null);
  const ran = ranFor(soFar.ranMs);
  const shared = soFar.shared.filter((file) => sharedNow?.has(file.file));
  const linked = soFar.links.flatMap((id) => links.filter((link) => link.linkId === id && link.url !== null));
  const openLink = (link: LinkItem) => void bridge.openLink(link.linkId).catch((e) => setNote(errorMessage(e)));
  const prompts = `${soFar.prompts} ${soFar.prompts === 1 ? 'prompt' : 'prompts'}`;

  return (
    <section className="catch-up" aria-label="So far">
      <div className="catch-up-head">
        <h3>{ran ? `You joined ${ran} in. So far: ${prompts}.` : `You joined just after the start. So far: ${prompts}.`}</h3>
        <button className="btn ghost icon" onClick={onClose} aria-label="Close the overview" title="Close">
          <X aria-hidden />
        </button>
      </div>
      <dl>
        {soFar.plans.length > 0 && (
          <Row title="Plans" items={soFar.plans}>
            {(plan) => (
              <li key={plan.change}>
                {plan.change}
                {plan.tasks.total > 0 && <span className="muted"> · {plan.tasks.done} of {plan.tasks.total} tasks</span>}
                {onReadPlan && (
                  <button className="btn small" onClick={() => onReadPlan(plan.change, [])}>
                    Read the plan
                  </button>
                )}
              </li>
            )}
          </Row>
        )}
        {soFar.documents.length > 0 && (
          <Row title="Documents" items={soFar.documents}>
            {(path) => (
              <li key={path}>
                <button className="link" title={path} onClick={() => onOpenDocument?.(path)}>
                  {base(path)}
                </button>
              </li>
            )}
          </Row>
        )}
        {shared.length + linked.length > 0 && (
          <Row title="Shared" items={[...shared.map((file) => ({ file })), ...linked.map((link) => ({ link }))]}>
            {(item) =>
              'file' in item ? (
                <li key={`file-${item.file.file}`}>
                  <button className="link" onClick={() => onOpenShared?.(item.file.file, item.file.name)}>
                    {item.file.name}
                  </button>
                </li>
              ) : (
                <li key={`link-${item.link.linkId}`}>
                  <button className="link" onClick={() => openLink(item.link)}>
                    {item.link.title ?? 'Link'}
                  </button>
                </li>
              )
            }
          </Row>
        )}
      </dl>
      {soFar.codeFiles > 0 && <p className="muted">{soFar.codeFiles === 1 ? '1 code file changed' : `${soFar.codeFiles} code files changed`}</p>}
      {note && <p className="field-hint">{note}</p>}
      <div className="catch-up-actions">
        <PromptPicker id="catch-up-prompt" label="Jump to a prompt" turns={turns} onPick={onJump} />
        <button className="btn small" onClick={onLatest}>
          Go to the latest prompt
        </button>
      </div>
    </section>
  );
}

function Row<T>({ title, items, children }: { title: string; items: T[]; children: (item: T) => ReactNode }) {
  const more = items.length - SHOWN;
  return (
    <>
      <dt>{title}</dt>
      <dd>
        <ul>
          {items.slice(0, SHOWN).map(children)}
          {more > 0 && <li className="muted">and {more} more</li>}
        </ul>
      </dd>
    </>
  );
}
