// Readable bodies for calls that asked the host something or handed work to a helper: questions
// with their options and answers, plans with the host's decision, to-do lists and helper reports.

import { Ban, Check, CircleDot, ShieldCheck, Square, SquareCheck } from 'lucide-react';
import type { Answer, Permission } from '../api/types';
import type { ToolItem } from '../model/timeline';
import { Markdown } from './Markdown';

interface Option {
  label: string;
  description?: string;
}

interface Question {
  question: string;
  options: Option[];
}

/** Calls where the host decided something: shown opened, so the decision is seen without a click. */
export function asksTheHost(item: ToolItem): boolean {
  return item.toolName === 'AskUserQuestion' || item.toolName === 'ExitPlanMode';
}

/** A body for the call when it has a readable form; null to show its raw input. */
export function interactionBody(item: ToolItem) {
  switch (item.toolName) {
    case 'AskUserQuestion':
      return <QuestionView item={item} />;
    case 'ExitPlanMode':
      return typeof item.input.plan === 'string' ? <PlanView item={item} plan={item.input.plan} /> : null;
    case 'TodoWrite':
      return Array.isArray(item.input.todos) ? <TodoView todos={item.input.todos} /> : null;
    case 'Agent':
    case 'Task':
      return item.outcome?.kind === 'report' ? <ReportView text={item.outcome.text} truncated={item.outcome.truncated} /> : null;
    default:
      return null;
  }
}

/** Says whether the host allowed or denied the call, when Claude Code asked them. */
export function PermissionBadge({ permission }: { permission?: Permission }) {
  if (!permission) return null;
  const Icon = permission === 'allowed' ? ShieldCheck : Ban;
  return (
    <span className={`permission ${permission}`}>
      <Icon aria-hidden />
      {permission === 'allowed' ? 'Allowed by the host' : 'Denied by the host'}
    </span>
  );
}

function questionsOf(item: ToolItem): Question[] {
  const raw = item.input.questions;
  if (!Array.isArray(raw)) return [];
  return raw.flatMap((q: Record<string, unknown>) => {
    if (typeof q?.question !== 'string') return [];
    const options = Array.isArray(q.options)
      ? q.options.flatMap((o: Record<string, unknown>) =>
          typeof o?.label === 'string' ? [{ label: o.label, description: typeof o.description === 'string' ? o.description : undefined }] : [],
        )
      : [];
    return [{ question: q.question, options }];
  });
}

function QuestionView({ item }: { item: ToolItem }) {
  const answers = item.outcome?.kind === 'answers' ? item.outcome.answers : null;
  const waiting = !answers && item.status === 'running';
  return (
    <div className="questions">
      {questionsOf(item).map((question) => {
        const answer: Answer | undefined = answers?.find((a) => a.question === question.question);
        return (
          <div key={question.question} className="question" role="group" aria-label={question.question}>
            <p className="question-text">{question.question}</p>
            <ul className="options">
              {question.options.map((option) => {
                const picked = answer?.selected.includes(option.label) ?? false;
                return (
                  <li key={option.label} className={picked ? 'picked' : undefined} aria-current={picked ? 'true' : undefined}>
                    {picked ? <Check aria-hidden /> : <span className="option-mark" aria-hidden />}
                    <span>
                      <span className="option-label">{option.label}</span>
                      {picked && <span className="visually-hidden"> (chosen)</span>}
                      {option.description && <span className="option-description">{option.description}</span>}
                    </span>
                  </li>
                );
              })}
            </ul>
            {answer?.typed && (
              <p className="typed selectable">
                <span className="hint">Typed by the host:</span> {answer.typed}
              </p>
            )}
            {answer?.notes && (
              <p className="typed selectable">
                <span className="hint">Notes:</span> {answer.notes}
              </p>
            )}
          </div>
        );
      })}
      {waiting && <p className="hint">Waiting for the host's answer.</p>}
      {!answers && item.status !== 'running' && <p className="hint">Not answered.</p>}
    </div>
  );
}

/** A plan the agent presented, with the host's decision; also the body of a plan tab. */
export function PlanView({ item, plan }: { item: ToolItem; plan: string }) {
  const decided = item.outcome?.kind === 'plan-decision' ? item.outcome : null;
  const [state, text] = decided ? (decided.approved ? ['approved', 'Approved'] : ['rejected', 'Rejected']) : ['waiting', 'Waiting for the host'];
  return (
    <div className="plan-view">
      <span className={`decision ${state}`}>{text}</span>
      <Markdown text={plan} />
    </div>
  );
}

const TODO_STATES = { pending: ['To do', Square], in_progress: ['In progress', CircleDot], completed: ['Done', SquareCheck] } as const;

function TodoView({ todos }: { todos: unknown[] }) {
  const items = todos.flatMap((todo) => {
    const t = todo as Record<string, unknown>;
    if (typeof t?.content !== 'string') return [];
    const status = (typeof t.status === 'string' && t.status in TODO_STATES ? t.status : 'pending') as keyof typeof TODO_STATES;
    return [{ content: t.content, status }];
  });
  return (
    <ul className="todo-list" aria-label="To-do list">
      {items.map((todo, i) => {
        const [name, Icon] = TODO_STATES[todo.status];
        return (
          <li key={i} className={todo.status.replace('_', '-')}>
            <Icon aria-hidden />
            <span>
              {todo.content}
              <span className="visually-hidden"> ({name})</span>
            </span>
          </li>
        );
      })}
    </ul>
  );
}

function ReportView({ text, truncated }: { text: string; truncated: boolean }) {
  return (
    <div className="report">
      <Markdown text={text} />
      {truncated && <p className="hint">The report was cut to its first 200 KB.</p>}
    </div>
  );
}
