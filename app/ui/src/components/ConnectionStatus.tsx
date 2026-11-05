import { useEffect, useRef, useState } from 'react';
import { errorMessage, useBridge } from '../api/bridge';
import type { DiagnosticEvent, DiagnosticsStatus, SharingStatus, TerminalSnapshot, ViewerStatus } from '../api/types';
import { attemptEnd, canReconnect, departure, formatBytes, logFileNotice, timeUntil, verdict } from '../model/diagnostics';
import { Toast, useToast } from './Toast';

const POLL_MS = 1_000;

/**
 * What this app's connection and terminal are doing, from Help → Connection Status: a verdict
 * in plain words, the details, and the recent log. Polls while open and not at all while closed.
 */
export function ConnectionStatus({ onClose }: { onClose: () => void }) {
  const bridge = useBridge();
  const [status, setStatus] = useState<DiagnosticsStatus | null>(null);
  const [unavailable, setUnavailable] = useState<string | null>(null);
  const [reconnecting, setReconnecting] = useState(false);
  const [toast, showToast] = useToast();
  const done = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    let stopped = false;
    const poll = async () => {
      try {
        const next = await bridge.diagnosticsStatus();
        if (stopped) return;
        setStatus(next);
        setUnavailable(null);
      } catch (error) {
        if (!stopped) setUnavailable(errorMessage(error));
      }
    };
    void poll();
    const timer = window.setInterval(() => void poll(), POLL_MS);
    return () => {
      stopped = true;
      window.clearInterval(timer);
    };
  }, [bridge]);

  useEffect(() => {
    done.current?.focus();
    const onKey = (event: KeyboardEvent) => event.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  const reconnect = async () => {
    setReconnecting(true);
    try {
      await bridge.reconnectViewer();
    } catch (error) {
      showToast(errorMessage(error));
    } finally {
      setReconnecting(false);
    }
  };

  const exportDiagnostics = async () => {
    try {
      if (await bridge.exportDiagnostics()) showToast('Diagnostics saved');
    } catch (error) {
      showToast(`Export failed: ${errorMessage(error)}`);
    }
  };

  const shown = status ? verdict(status) : null;
  const notice = status ? logFileNotice(status) : null;
  const isViewer = status?.session.role === 'viewer';

  return (
    <div className="overlay" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="dialog connection-status" role="dialog" aria-modal="true" aria-labelledby="connection-status-title">
        <h2 id="connection-status-title">Connection status</h2>
        <p className={`verdict ${shown?.tone ?? ''}`} role="status" aria-live="polite">
          {shown?.text ?? unavailable ?? 'Reading the connection…'}
        </p>
        {notice && <p className="notice">{notice}</p>}

        {status && (
          <div className="status-details" role="group" tabIndex={0} aria-label="Connection details and recent events">
            {status.session.role === 'host' && <HostDetails sharing={status.session.sharing} now={status.now} />}
            {status.session.role === 'viewer' && <ViewerDetails viewer={status.session.viewer} now={status.now} />}
            {status.terminal && <TerminalDetails terminal={status.terminal} />}
            {status.window && (
              <>
                <h3>Window</h3>
                <dl>
                  <Row name="Timeline events" value={status.window.timelineEvents.toLocaleString()} />
                  <Row name="Elements on the page" value={status.window.elements.toLocaleString()} />
                </dl>
              </>
            )}
            <h3>Recent events</h3>
            {status.events.length === 0 ? (
              <p>None yet.</p>
            ) : (
              <ol className="events">
                {status.events.slice(0, 100).map((event, index) => (
                  <li key={`${event.ts}-${index}`} className={event.level === 'WARN' ? 'warn' : undefined}>
                    <time dateTime={event.ts}>{new Date(event.ts).toLocaleTimeString()}</time> {event.event}{' '}
                    <span className="fields">{fields(event)}</span>
                  </li>
                ))}
              </ol>
            )}
          </div>
        )}

        <p className="field-hint" id="export-hint">
          The export holds this state and the last 7 days of this log: connection events, timings, sizes and whether you
          connected directly or through the relay. It never holds terminal output, the timeline, invite codes, names, file
          paths or addresses. Nothing is sent anywhere; attach the file to a bug report yourself.
        </p>
        <div className="actions">
          {isViewer && (
            <button className="btn" disabled={reconnecting || !canReconnect(status)} onClick={() => void reconnect()}>
              Reconnect
            </button>
          )}
          <button className="btn" aria-describedby="export-hint" onClick={() => void exportDiagnostics()}>
            Export Diagnostics…
          </button>
          <button ref={done} className="btn primary" onClick={onClose}>
            Done
          </button>
        </div>
        <Toast message={toast} />
      </div>
    </div>
  );
}

function fields(event: DiagnosticEvent): string {
  return Object.entries(event)
    .filter(([key]) => key !== 'ts' && key !== 'level' && key !== 'event')
    .map(([key, value]) => `${key}=${String(value)}`)
    .join(' ');
}

function path(relayed: boolean | null): string {
  if (relayed === null) return 'not yet known';
  return relayed ? 'through the relay' : 'direct';
}

function HostDetails({ sharing, now }: { sharing: SharingStatus; now: number }) {
  return (
    <>
      <h3>Sharing</h3>
      <dl>
        <Row name="Sharing service" value={sharing.service === 'unavailable' && sharing.retryAt !== null ? `unavailable, trying again ${timeUntil(sharing.retryAt, now)}` : sharing.service} />
        {sharing.turnExpiresAt !== null && (
          <Row name="Relay credentials" value={sharing.turnExpiresAt > now ? `expire ${timeUntil(sharing.turnExpiresAt, now)}` : 'expired'} />
        )}
        {sharing.lastDeparture && (
          <Row
            name="Last to go"
            value={`${sharing.lastDeparture.viewer === null ? 'a viewer joining' : `viewer ${sharing.lastDeparture.viewer}`} ${departure(sharing.lastDeparture.reason)}, ${new Date(sharing.lastDeparture.at).toLocaleTimeString()}`}
          />
        )}
      </dl>
      {sharing.viewers.map((link) => (
        <dl key={link.peer} aria-label={link.viewer === null ? `Connection ${link.peer}` : `Viewer ${link.viewer}`}>
          <Row name={link.viewer === null ? `Connection ${link.peer}` : `Viewer ${link.viewer}`} value={`${link.state}, ${path(link.relayed)}`} />
          <Row name="Sent" value={`${link.recordsSent} records, ${link.eventsSent} events, ${formatBytes(link.bytesSent)}`} />
          <Row name="Waiting to send" value={formatBytes(link.queued)} />
        </dl>
      ))}
    </>
  );
}

function ViewerDetails({ viewer, now }: { viewer: ViewerStatus; now: number }) {
  return (
    <>
      <h3>Connection</h3>
      <dl>
        <Row name="State" value={`${viewer.phase}, attempt ${viewer.attempt}`} />
        {viewer.state && <Row name="Peer connection" value={`${viewer.state}, ${path(viewer.relayed)}`} />}
        {viewer.retryAt !== null && viewer.retryAt > now && <Row name="Next attempt" value={timeUntil(viewer.retryAt, now)} />}
        {viewer.lastAttempt && (
          <Row
            name="Previous attempt"
            value={`ended because ${attemptEnd(viewer.lastAttempt.reason)}, after ${Math.round(viewer.lastAttempt.connectedMs / 1000)} s connected`}
          />
        )}
        <Row name="Sharing service" value={viewer.signallingConnected ? 'connected' : 'not connected (not needed once connected)'} />
        <Row name="Received" value={`${viewer.recordsApplied} records, ${viewer.eventsApplied} events, ${formatBytes(viewer.bytesReceived)}`} />
      </dl>
    </>
  );
}

function TerminalDetails({ terminal }: { terminal: TerminalSnapshot }) {
  return (
    <>
      <h3>Terminal</h3>
      <dl>
        <Row name="Screen" value={terminal.screen === 'alternate' ? 'alternate (full-screen program, no scrollback)' : 'normal'} />
        <Row
          name="Mouse"
          value={terminal.mouseTracking === 'none' ? 'not used by the program' : `used by the program (${terminal.mouseTracking}): the wheel goes to it`}
        />
        <Row name="Scrollback" value={`${terminal.scrollbackLines.toLocaleString()} of ${terminal.scrollbackLimit.toLocaleString()} lines`} />
        <Row name="View" value={terminal.following ? 'following the latest output' : `${terminal.linesFromBottom.toLocaleString()} lines up`} />
        <Row name="Font size" value={`${terminal.fontSize}`} />
      </dl>
    </>
  );
}

function Row({ name, value }: { name: string; value: string }) {
  return (
    <div>
      <dt>{name}</dt>
      <dd>{value}</dd>
    </div>
  );
}
