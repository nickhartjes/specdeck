import { FileText } from 'lucide-react';
import type { DocumentVersion, Plan } from '../model/documents';
import type { ReaderDocument } from './DocumentTab';

interface Props {
  plan: Plan;
  versions: Map<string, DocumentVersion[]>;
  /** The version this participant last read, by path. */
  lastRead: Map<string, number>;
  /** Opens the reader on a file of this plan. */
  onOpen: (path: string) => void;
}

/** "viewer-views" → "Viewer views". */
function readable(name: string): string {
  const words = name.replace(/[-_]/g, ' ');
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** The proposal's first heading, else the change's name made readable. */
export function planTitle(plan: Plan, versions: Map<string, DocumentVersion[]>): string {
  const proposal = plan.documents.find((doc) => doc.id === 'proposal')?.paths[0];
  const text = proposal ? versions.get(proposal)?.at(-1)?.text : null;
  const heading = text?.match(/^#\s+(.+)$/m)?.[1]?.trim();
  return heading && heading.toLowerCase() !== 'proposal' ? heading : readable(plan.change);
}

/** The plan's files as reader tabs, in schema order; a spec is named by its capability. */
export function planReaderDocuments(plan: Plan): ReaderDocument[] {
  return plan.documents.flatMap((doc) =>
    doc.paths.map((path) => {
      const capability = doc.paths.length > 1 || doc.id === 'specs' ? path.split('/').at(-2) : null;
      return { path, title: capability ? `Spec: ${capability}` : doc.name };
    }),
  );
}

/** An OpenSpec change the agent works on: its documents, what changed in them, and its tasks. */
export function PlanCard({ plan, versions, lastRead, onOpen }: Props) {
  const title = planTitle(plan, versions);
  const { done, total } = plan.tasks;
  return (
    <section className="plan-card" aria-label={`Plan: ${title}`}>
      <div className="plan-head">
        <span className="plan-kind">Plan</span>
        <strong>{title}</strong>
        <code>{plan.change}</code>
      </div>
      <div className="plan-docs">
        {plan.documents.map((doc) => {
          const state = doc.paths.some((path) => {
            const latest = versions.get(path)?.at(-1)?.version;
            const read = lastRead.get(path);
            return latest !== undefined && read !== undefined && latest > read;
          })
            ? 'changed'
            : doc.exists && doc.paths.every((path) => !lastRead.has(path)) && doc.paths.some((path) => versions.has(path))
              ? 'new'
              : null;
          return (
            <button key={doc.id} className="btn small" disabled={!doc.exists} onClick={() => doc.paths[0] && onOpen(doc.paths[0])}>
              <FileText aria-hidden />
              {doc.paths.length > 1 ? `${doc.name} (${doc.paths.length})` : doc.name}
              {state && <span className={`plan-badge ${state}`}>{state}</span>}
            </button>
          );
        })}
      </div>
      {total > 0 && (
        <div className="plan-tasks">
          <span>
            {done} of {total} tasks done
          </span>
          <span className="bar" role="progressbar" aria-label="Tasks" aria-valuemin={0} aria-valuemax={total} aria-valuenow={done}>
            <span style={{ width: `${Math.round((done / total) * 100)}%` }} />
          </span>
        </div>
      )}
    </section>
  );
}
