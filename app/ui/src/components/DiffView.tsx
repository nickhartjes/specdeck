import { SHOWN_IN_PART, type FileChange } from '../model/timeline';

const LABELS = {
  created: { verb: 'Created', kind: 'new file' },
  edited: { verb: 'Changed', kind: 'edited' },
  deleted: { verb: 'Deleted', kind: 'deleted' },
} as const;

/** One change to one file, as unified diff hunks with line numbers. */
export function DiffView({ change, showPath = true }: { change: FileChange; showPath?: boolean }) {
  const label = LABELS[change.change];
  return (
    <div className="diff" aria-label={`${label.verb} ${change.path}`}>
      {showPath && (
        <div className="file">
          <span>{change.path}</span>
          <span className="kind">{label.kind}</span>
        </div>
      )}
      {/* Binary files, and changes too large to send, come without hunks. */}
      {change.hunks.length === 0 && <div className="hunk">No diff to show</div>}
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
                    <span>{line.slice(1)}</span>
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
