import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { demoDiagnostics } from '../api/demo';
import type { DiagnosticsStatus, ViewerStatus } from '../api/types';
import { fakeBridge, renderWith } from '../test/helpers';
import { ConnectionStatus } from './ConnectionStatus';

function viewerStatus(viewer: Partial<ViewerStatus>): DiagnosticsStatus {
  const status = demoDiagnostics('viewer');
  if (status.session.role !== 'viewer') throw new Error('a viewer status');
  return { ...status, session: { role: 'viewer', viewer: { ...status.session.viewer, ...viewer } } };
}

afterEach(() => vi.useRealTimers());

describe('connection status panel', () => {
  it('announces the verdict', async () => {
    renderWith(fakeBridge('viewer'), <ConnectionStatus onClose={() => {}} />);
    expect(await screen.findByRole('status')).toHaveTextContent('Connected to the host through the relay.');
    expect(screen.getByRole('status')).toHaveAttribute('aria-live', 'polite');
  });

  it('polls once a second while open and stops when closed', async () => {
    vi.useFakeTimers();
    const bridge = fakeBridge('viewer');
    const view = renderWith(bridge, <ConnectionStatus onClose={() => {}} />);
    await act(() => vi.advanceTimersByTimeAsync(3_000));
    expect(bridge.diagnosticsStatus).toHaveBeenCalledTimes(4);
    view.unmount();
    await act(() => vi.advanceTimersByTimeAsync(5_000));
    expect(bridge.diagnosticsStatus).toHaveBeenCalledTimes(4);
  });

  it('offers Reconnect to a connected viewer', async () => {
    const bridge = fakeBridge('viewer');
    renderWith(bridge, <ConnectionStatus onClose={() => {}} />);
    const reconnect = await screen.findByRole('button', { name: 'Reconnect' });
    await waitFor(() => expect(reconnect).toBeEnabled());
    await userEvent.click(reconnect);
    expect(bridge.reconnectViewer).toHaveBeenCalled();
  });

  it('makes Reconnect unavailable while reconnecting', async () => {
    const reconnecting = viewerStatus({ phase: 'reconnecting', attempt: 3, retryAt: Date.now() + 2_000 });
    renderWith(fakeBridge('viewer', { diagnosticsStatus: async () => reconnecting }), <ConnectionStatus onClose={() => {}} />);
    expect(await screen.findByRole('status')).toHaveTextContent('Reconnecting, attempt 3.');
    expect(screen.getByRole('button', { name: 'Reconnect' })).toBeDisabled();
  });

  it('does not offer Reconnect to the host', async () => {
    renderWith(fakeBridge('host'), <ConnectionStatus onClose={() => {}} />);
    expect(await screen.findByRole('status')).toHaveTextContent('Sharing. 2 viewers are connected, 1 through the relay.');
    expect(screen.queryByRole('button', { name: 'Reconnect' })).toBeNull();
  });

  it('can be used with the keyboard alone', async () => {
    const onClose = vi.fn();
    renderWith(fakeBridge('viewer'), <ConnectionStatus onClose={onClose} />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Reconnect' })).toBeEnabled());
    expect(screen.getByRole('button', { name: 'Done' })).toHaveFocus();
    const reached = new Set<Element | null>();
    for (let i = 0; i < 8; i++) {
      await userEvent.tab();
      reached.add(document.activeElement);
    }
    for (const control of [
      screen.getByRole('group', { name: 'Connection details and recent events' }),
      screen.getByRole('button', { name: 'Reconnect' }),
      screen.getByRole('button', { name: /Export Diagnostics/ }),
      screen.getByRole('button', { name: 'Done' }),
    ]) {
      expect(reached).toContain(control);
    }
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalled();
  });

  it('says what the export holds before saving, and when it fails', async () => {
    const bridge = fakeBridge('viewer', { exportDiagnostics: () => Promise.reject(new Error('The diagnostics could not be saved there.')) });
    renderWith(bridge, <ConnectionStatus onClose={() => {}} />);
    const exportButton = await screen.findByRole('button', { name: /Export Diagnostics/ });
    expect(exportButton).toHaveAccessibleDescription(/never holds terminal output/);
    await userEvent.click(exportButton);
    expect(await screen.findByText('Export failed: The diagnostics could not be saved there.')).toBeInTheDocument();
  });

  it('says when the log file cannot be written', async () => {
    const status = { ...demoDiagnostics('viewer'), logFileError: 'PermissionDenied' };
    renderWith(fakeBridge('viewer', { diagnosticsStatus: async () => status }), <ConnectionStatus onClose={() => {}} />);
    expect(await screen.findByText(/log file cannot be written/)).toBeInTheDocument();
  });
});
