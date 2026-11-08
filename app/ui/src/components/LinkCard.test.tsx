import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { fakeBridge, renderWith } from '../test/helpers';
import type { LinkItem } from '../model/timeline';
import { LinkCard } from './LinkCard';

const ROOM = 'https://excalidraw.com/#room=5c1e2a9b,Q3h9vX2m';

function link(overrides: Partial<LinkItem> = {}): LinkItem {
  return {
    kind: 'link',
    id: 'link-1',
    linkId: 1,
    url: 'https://staging.example.com/s/demo',
    title: 'Staging: the new pill',
    by: 'Mo',
    sessionOnly: false,
    at: 0,
    outputSeq: 0,
    ...overrides,
  };
}

describe('LinkCard', () => {
  it('shows the title, site, address and who shared it, and opens by id', async () => {
    const bridge = fakeBridge('host');
    renderWith(bridge, <LinkCard link={link()} />);
    const card = screen.getByRole('group', { name: 'Link: Staging: the new pill' });
    expect(within(card).getByText(/staging\.example\.com · shared by Mo/)).toBeTruthy();
    expect(within(card).getByText('https://staging.example.com/s/demo')).toBeTruthy();
    await userEvent.click(within(card).getByRole('button', { name: 'Open in your browser' }));
    expect(bridge.openLink).toHaveBeenCalledWith(1);
    await userEvent.click(within(card).getByRole('button', { name: 'Copy link' }));
    expect(bridge.copyText).toHaveBeenCalledWith('https://staging.example.com/s/demo');
    expect(screen.getByRole('status').textContent).toBe('Copied.');
  });

  it('shows a title made of markup as text', () => {
    renderWith(fakeBridge('host'), <LinkCard link={link({ title: '<img src=x onerror=alert(1)>' })} />);
    expect(screen.getByText('<img src=x onerror=alert(1)>')).toBeTruthy();
    expect(document.querySelector('img')).toBeNull();
  });

  it('hides the key of a link for this session only, and warns when copying it', async () => {
    const bridge = fakeBridge('host');
    renderWith(bridge, <LinkCard link={link({ url: ROOM, title: undefined, sessionOnly: true })} />);
    expect(screen.getByText('https://excalidraw.com/#…')).toBeTruthy();
    expect(screen.queryByText(/Q3h9vX2m/)).toBeNull();
    expect(screen.getByText(/For this session only/)).toBeTruthy();
    // Titled by its site.
    expect(screen.getByRole('group', { name: 'Link: excalidraw.com' })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Copy link' }));
    expect(bridge.copyText).toHaveBeenCalledWith(ROOM);
    expect(screen.getByRole('status').textContent).toMatch(/holds a key/);
  });

  it('lets the host remove a link after saying who still has it', async () => {
    const onRemove = vi.fn();
    renderWith(fakeBridge('host'), <LinkCard link={link()} onRemove={onRemove} />);
    await userEvent.click(screen.getByRole('button', { name: 'Remove…' }));
    expect(screen.getByRole('alertdialog').textContent).toMatch(/People who already saw it still have it/);
    await userEvent.click(screen.getByRole('button', { name: 'Remove link' }));
    expect(onRemove).toHaveBeenCalledWith(1);
  });

  it('offers no removal to viewers', () => {
    renderWith(fakeBridge('viewer'), <LinkCard link={link()} />);
    expect(screen.queryByRole('button', { name: 'Remove…' })).toBeNull();
  });

  it('says a removed link was removed, without its address or buttons', () => {
    renderWith(fakeBridge('viewer'), <LinkCard link={link({ url: null, title: 'Draw' })} />);
    expect(screen.getByText(/The host removed this link/)).toBeTruthy();
    expect(screen.queryByRole('button')).toBeNull();
  });

  it('never offers to open a link that fails the check, as from a modified host', () => {
    renderWith(fakeBridge('viewer'), <LinkCard link={link({ url: 'file:///etc/passwd', title: undefined })} />);
    expect(screen.getByText(/can’t be opened/)).toBeTruthy();
    expect(screen.queryByRole('button')).toBeNull();
    expect(screen.queryByText(/etc\/passwd/)).toBeNull();
  });
});
