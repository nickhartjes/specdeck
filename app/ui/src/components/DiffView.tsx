import { useMemo } from 'react';
import { highlightHunk, languageOf } from '../model/highlight';
import { SHOWN_IN_PART, type FileChange } from '../model/timeline';
import { CodeSpans } from './CodeLines';

const LABELS = {
  created: { verb: 'Created', kind: 'new file' },
  edited: { verb: 'Changed', kind: 'edited' },
  deleted: { verb: 'Deleted', kind: 'deleted' },
} as const;

/**
 * Whether the host showed this path from outside the session folder: paths inside it are
 * relative, others start with `~` or are absolute. The host sends no diff for those.
 */
function outsideFolder(path: string): boolean {
  return path.startsWith('~') || path.startsWith('/') || path.startsWith('\\') || /^[A-Za-z]:[\\/]/.test(path);
}

/** One change to one file, as unified diff hunks with line numbers, highlighted. */
export function DiffView({ change, showPath = true }: { change: FileChange; showPath?: boolean }) {
  const label = LABELS[change.change];
  const highlighted = useMemo(() => {
    const language = languageOf(change.path);
    return change.hunks.map((hunk) => highlightHunk(hunk.lines, language));
  }, [change.path, change.hunks]);
  return (
    <div className="diff" aria-label={`${label.verb} ${change.path}`}>
      {showPath && (
        <div className="file">
          <span>{change.path}</span>
          <span className="kind">{label.kind}</span>
        </div>
      )}
      {/* Binary files, changes too large to send, and files outside the session folder come without hunks. */}
      {change.hunks.length === 0 && <div className="hunk">{outsideFolder(change.path) ? 'Outside the session folder' : 'No diff to show'}</div>}
      {change.hunks.map((hunk, i) => {
        let oldLine = hunk.oldStart;
        let newLine = hunk.newStart;
        return (
          <div key={i}>
            {change.change === 'edited' && (
              <div className="hunk">
                @@ −{hunk.oldStart},{hunk.oldLines} +{hunk.newStart},{hunk.newLines} @@
              </div>
            )}
            <div className="lines">
              {hunk.lines.map((line, j) => {
                const sign = line[0] ?? ' ';
                const kind = sign === '+' ? 'add' : sign === '-' ? 'del' : '';
                const left = sign === '+' ? '' : oldLine++;
                const right = sign === '-' ? '' : newLine++;
                return (
                  <div key={j} className={`line ${kind}`}>
                    <span className="n">{left}</span>
                    <span className="n">{right}</span>
                    <span className="sign">{sign === ' ' ? '' : sign}</span>
                    <span>
                      <CodeSpans spans={highlighted[i]?.[j] ?? [{ text: line.slice(1), kind: null }]} />
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        );
      })}
      {change.truncated && <div className="hunk">{SHOWN_IN_PART}</div>}
    </div>
  );
}
