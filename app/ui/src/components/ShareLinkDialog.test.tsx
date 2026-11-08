import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { fakeBridge, renderWith } from '../test/helpers';
import { ShareLinkDialog } from './ShareLinkDialog';

function dialog(host: boolean, onShare = vi.fn(async () => {})) {
  const onClose = vi.fn();
  renderWith(fakeBridge(host ? 'host' : 'viewer'), <ShareLinkDialog host={host} hostName="Nick" onShare={onShare} onClose={onClose} />);
  return { onShare, onClose };
}

describe('ShareLinkDialog', () => {
  it('shares the normalized address with its title, and closes', async () => {
    const { onShare, onClose } = dialog(true);
    await userEvent.type(screen.getByLabelText('Address'), 'https://Staging.example.com');
    await userEvent.type(screen.getByLabelText('Title (optional)'), '  Staging  ');
    await userEvent.click(screen.getByRole('button', { name: 'Share with everyone' }));
    expect(onShare).toHaveBeenCalledWith('https://staging.example.com/', 'Staging', false);
    expect(onClose).toHaveBeenCalled();
  });

  it('marks a link with a # part for this session only, until the host changes it', async () => {
    const { onShare } = dialog(true);
    const mark = screen.getByRole('checkbox', { name: /For this session only/ });
    await userEvent.type(screen.getByLabelText('Address'), 'https://excalidraw.com/#room=a,b');
    expect((mark as HTMLInputElement).checked).toBe(true);
    await userEvent.click(mark);
    expect((mark as HTMLInputElement).checked).toBe(false);
    await userEvent.click(screen.getByRole('button', { name: 'Share with everyone' }));
    expect(onShare).toHaveBeenCalledWith('https://excalidraw.com/#room=a,b', null, false);
  });

  it('refuses localhost with the hint for the host, and shares nothing', async () => {
    const { onShare } = dialog(true);
    await userEvent.type(screen.getByLabelText('Address'), 'http://localhost:5173/');
    await userEvent.click(screen.getByRole('button', { name: 'Share with everyone' }));
    expect(screen.getByRole('alert').textContent).toMatch(/Viewers can’t reach your computer.*not available yet/);
    expect(onShare).not.toHaveBeenCalled();
  });

  it('tells a viewer a localhost link is on their own computer', async () => {
    const { onShare } = dialog(false);
    await userEvent.type(screen.getByLabelText('Address'), 'http://127.0.0.1:3000/');
    await userEvent.click(screen.getByRole('button', { name: 'Send to Nick' }));
    expect(screen.getByRole('alert').textContent).toMatch(/on your own computer/);
    expect(onShare).not.toHaveBeenCalled();
  });

  it('refuses other schemes and addresses with a password', async () => {
    const { onShare } = dialog(true);
    const address = screen.getByLabelText('Address');
    await userEvent.type(address, 'javascript:alert(1)');
    await userEvent.click(screen.getByRole('button', { name: 'Share with everyone' }));
    expect(screen.getByRole('alert').textContent).toMatch(/Only web addresses/);
    await userEvent.clear(address);
    await userEvent.type(address, 'https://user:pw@example.com/');
    await userEvent.click(screen.getByRole('button', { name: 'Share with everyone' }));
    expect(screen.getByRole('alert').textContent).toMatch(/user name or password/);
    expect(onShare).not.toHaveBeenCalled();
  });

  it('keeps the dialog open with the reason when sharing fails', async () => {
    const { onClose } = dialog(true, vi.fn(async () => Promise.reject(new Error('The session has not started.'))));
    await userEvent.type(screen.getByLabelText('Address'), 'https://example.com/');
    await userEvent.click(screen.getByRole('button', { name: 'Share with everyone' }));
    expect((await screen.findByRole('alert')).textContent).toBe('The session has not started.');
    expect(onClose).not.toHaveBeenCalled();
  });
});
