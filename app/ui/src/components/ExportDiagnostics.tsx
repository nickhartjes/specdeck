import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { errorMessage, useBridge } from '../api/bridge';

/**
 * Says what the web viewer's diagnostics file holds before downloading it (`viewer-diagnostics`
 * design decision 3). The browser has no Connection status panel to carry the hint, so it is
 * its own small dialog, on the page's body: its openers sit in paragraphs and live regions.
 */
export function ExportDiagnosticsDialog({ onClose }: { onClose: () => void }) {
  const bridge = useBridge();
  const [error, setError] = useState<string | null>(null);
  const download = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    download.current?.focus();
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const save = async () => {
    try {
      if (await bridge.exportDiagnostics()) onClose();
    } catch (failure) {
      setError(`Export failed: ${errorMessage(failure)}`);
    }
  };

  return createPortal(
    <div className="overlay" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="export-diagnostics-title" aria-describedby="export-diagnostics-hint">
        <h2 id="export-diagnostics-title">Export diagnostics</h2>
        <p className="field-hint" id="export-diagnostics-hint">
          The file holds this page&rsquo;s connection events since the tab opened: when connections started and ended and why, sizes and timings, and
          whether you connected directly or through the relay. It also names your browser and its version. It never holds terminal output, the
          timeline, invite codes, names, file names or addresses. Nothing is sent anywhere; attach the file to a bug report yourself.
        </p>
        {error && (
          <p className="error-text" role="alert">
            {error}
          </p>
        )}
        <div className="actions">
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button ref={download} className="btn primary" onClick={() => void save()}>
            Download
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

/** A link that opens the dialog, for the join page and the screen of an ended session. */
export function ExportDiagnosticsLink() {
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  return (
    <>
      <button type="button" className="link" onClick={() => setOpen(true)}>
        Export diagnostics…
      </button>
      {open && <ExportDiagnosticsDialog onClose={close} />}
    </>
  );
}
