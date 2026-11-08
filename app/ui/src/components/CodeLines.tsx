import { memo } from 'react';
import type { Span } from '../model/highlight';
import type { LineMark } from '../model/lineDiff';

/** Highlighted spans of one line, as text: nothing in a file is ever read as markup. */
export const CodeSpans = memo(function CodeSpans({ spans }: { spans: Span[] }) {
  return (
    <>
      {spans.map((span, i) =>
        span.kind ? (
          <span key={i} className={`tok-${span.kind}`}>
            {span.text}
          </span>
        ) : (
          span.text
        ),
      )}
    </>
  );
});

const MARKS = {
  added: { sign: '+', label: 'Added in this session' },
  changed: { sign: '~', label: 'Changed in this session' },
} as const;

interface Props {
  /** The text's lines, highlighted. */
  lines: Span[][];
  /** A mark per line, shown by a sign and a label as well as by colour. */
  marks?: LineMark[];
  /** Lines removed just before each line; the last entry is for the end. */
  removed?: number[];
  /** Overrides the marks' wording, such as for lines new since the participant last read them. */
  labels?: Partial<Record<'added' | 'changed', string>>;
  label: string;
}

function Removed({ count }: { count: number }) {
  const text = count === 1 ? '1 line removed' : `${count} lines removed`;
  return (
    <div className="removed" title={`${text} in this session`}>
      <span />
      <span className="m" aria-hidden>
        −
      </span>
      <span className="t">{text}</span>
    </div>
  );
}

/** Numbered lines of code, highlighted, with what changed marked. */
export function CodeLines({ lines, marks, removed, labels, label }: Props) {
  return (
    <div className="code-lines selectable" role="group" aria-label={label}>
      {lines.map((spans, i) => {
        const mark = marks?.[i] ?? null;
        const markLabel = mark ? (labels?.[mark] ?? MARKS[mark].label) : undefined;
        return (
          <div key={i}>
            {(removed?.[i] ?? 0) > 0 && <Removed count={removed![i]!} />}
            <div className={`ln${mark ? ` mark-${mark}` : ''}`} title={markLabel} data-line={i + 1}>
              <span className="n">{i + 1}</span>
              <span className="m">
                {mark && <span aria-hidden>{MARKS[mark].sign}</span>}
                {markLabel && <span className="visually-hidden">{markLabel}: </span>}
              </span>
              <span className="t">{spans.length ? <CodeSpans spans={spans} /> : ' '}</span>
            </div>
          </div>
        );
      })}
      {(removed?.[lines.length] ?? 0) > 0 && <Removed count={removed![lines.length]!} />}
    </div>
  );
}
