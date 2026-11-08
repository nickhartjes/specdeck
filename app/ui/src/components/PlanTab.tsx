// A plan the agent presented for the host's approval, in a tab of its own (`notification-open`
// design decision 5): opened from a viewer's notification, read-only, with the host's decision
// as it comes.

import type { ToolItem } from '../model/timeline';
import { PlanView } from './HostInteractions';

export function PlanTab({ item, title }: { item: ToolItem | null; title: string }) {
  const plan = item?.input.plan;
  if (!item || typeof plan !== 'string') return <p className="hint screenshot-missing">This plan is no longer in the timeline.</p>;
  return (
    <div className="document-tab">
      <div className="filebar">
        <span className="path selectable">{title}</span>
      </div>
      <article className="doc selectable" aria-label={title}>
        <PlanView item={item} plan={plan} />
      </article>
    </div>
  );
}
