import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { demoProfile } from '../api/demo';
import { createNoticeList } from '../model/noticeList';
import { DEFAULT_VIEWER_NOTIFICATIONS } from '../model/viewerNotices';
import { fakeBridge, renderWith } from '../test/helpers';
import { NotificationBell } from './NotificationBell';

function setup(bridge = fakeBridge('viewer')) {
  const list = createNoticeList();
  const onOpen = vi.fn();
  const onSwitches = vi.fn();
  renderWith(bridge, <NotificationBell list={list} onOpen={onOpen} onSwitches={onSwitches} />);
  return { list, onOpen, onSwitches, bridge };
}

const bell = () => screen.getByRole('button', { name: /^Notifications/ });

describe('the bell', () => {
  afterEach(() => delete document.documentElement.dataset.platform);

  it('counts what is new, in its name too, and opening resets it', async () => {
    const { list } = setup();
    expect(bell()).toHaveAccessibleName('Notifications');
    act(() => {
      list.add({ kind: 'reads', title: 'Henk asks you to read', body: 'design.md', open: { kind: 'read', id: 1 } });
      list.add({ kind: 'documents', title: 'README.md changed', body: '', open: { kind: 'document', path: 'README.md' } });
    });
    expect(bell()).toHaveAccessibleName('Notifications, 2 new');
    expect(bell()).toHaveTextContent('2');
    await userEvent.click(bell());
    expect(bell()).toHaveAccessibleName('Notifications');
    const items = within(screen.getByRole('list', { name: 'What happened' })).getAllByRole('listitem');
    expect(items.map((item) => item.textContent)).toEqual([expect.stringContaining('README.md changed'), expect.stringContaining('Henk asks you to read')]);
    // Marked as new while the list is open.
    expect(items.every((item) => item.classList.contains('new'))).toBe(true);
  });

  it('says what it will show while empty', async () => {
    setup();
    await userEvent.click(bell());
    expect(screen.getByRole('dialog', { name: 'Notifications' })).toHaveTextContent('What happens in this session from now on shows up here.');
  });

  it('opens an entry, closing the list', async () => {
    const { list, onOpen } = setup();
    act(() => list.add({ kind: 'plans', title: 'Plan add-login is ready for review', body: '', open: { kind: 'document', path: 'proposal.md' } }));
    await userEvent.click(bell());
    await userEvent.click(screen.getByRole('button', { name: /Plan add-login is ready for review/ }));
    expect(onOpen).toHaveBeenCalledWith({ kind: 'document', path: 'proposal.md' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('shows an entry that opens nothing as text', async () => {
    const { list } = setup();
    act(() => list.add({ kind: 'finished', title: 'Claude Code finished', body: "It waits for the host's next prompt." }));
    await userEvent.click(bell());
    expect(screen.getByText('Claude Code finished')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Claude Code finished/ })).not.toBeInTheDocument();
  });

  it('closes on Escape, and gives the focus back', async () => {
    setup();
    await userEvent.click(bell());
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(bell()).toHaveFocus();
  });

  it('switches which kinds also pop up, the same settings as Settings', async () => {
    const { bridge, onSwitches } = setup();
    await userEvent.click(bell());
    const group = await screen.findByRole('group', { name: 'Also pop up when' });
    const finished = within(group).getByRole('checkbox', { name: 'The agent finished' });
    expect(finished).not.toBeChecked();
    expect(within(group).getAllByRole('checkbox')).toHaveLength(8);
    await userEvent.click(finished);
    const kinds = { ...DEFAULT_VIEWER_NOTIFICATIONS, finished: true };
    expect(bridge.setViewerNotifications).toHaveBeenCalledWith(kinds);
    await waitFor(() => expect(onSwitches).toHaveBeenCalledWith(kinds));
    expect(finished).toBeChecked();
  });

  it('leaves the kinds alone, unavailable, while notifications are off in Settings', async () => {
    setup(fakeBridge('viewer', { getProfile: vi.fn(async () => ({ ...demoProfile, notifications: false })) }));
    await userEvent.click(bell());
    const plans = await screen.findByRole('checkbox', { name: 'A plan is ready for review' });
    expect(plans).toBeDisabled();
    expect(plans).toBeChecked();
    expect(screen.getByRole('group', { name: 'Also pop up when' })).toHaveAccessibleDescription('Notifications are off in Settings.');
    expect(screen.queryByRole('checkbox', { name: 'While this tab is in the background' })).not.toBeInTheDocument();
  });

  describe('in the web viewer', () => {
    it('has the main switch, and the kinds once it is on', async () => {
      document.documentElement.dataset.platform = 'web';
      const profile = { ...demoProfile, notifications: false };
      const bridge = fakeBridge('viewer', {
        getProfile: vi.fn(async () => profile),
        setNotifications: vi.fn(async (notifications: boolean) => ({ ...profile, notifications })),
      });
      setup(bridge);
      await userEvent.click(bell());
      const main = await screen.findByRole('checkbox', { name: 'While this tab is in the background' });
      expect(main).not.toBeChecked();
      expect(screen.queryByRole('checkbox', { name: 'A plan is ready for review' })).not.toBeInTheDocument();
      await userEvent.click(main);
      expect(bridge.setNotifications).toHaveBeenCalledWith(true);
      expect(await screen.findByRole('checkbox', { name: 'A plan is ready for review' })).toBeChecked();
      await userEvent.click(screen.getByRole('checkbox', { name: 'The host asks for an estimate' }));
      expect(bridge.setViewerNotifications).toHaveBeenCalledWith({ ...DEFAULT_VIEWER_NOTIFICATIONS, estimates: false });
    });

    it('says so when the browser does not allow them', async () => {
      document.documentElement.dataset.platform = 'web';
      setup(
        fakeBridge('viewer', {
          getProfile: vi.fn(async () => ({ ...demoProfile, notifications: false })),
          setNotifications: vi.fn(async () => {
            throw new Error('This browser does not allow notifications for this page.');
          }),
        }),
      );
      await userEvent.click(bell());
      await userEvent.click(await screen.findByRole('checkbox', { name: 'While this tab is in the background' }));
      expect(await screen.findByRole('status')).toHaveTextContent('This browser does not allow notifications for this page.');
      expect(screen.getByRole('checkbox', { name: 'While this tab is in the background' })).not.toBeChecked();
      expect(screen.queryByRole('checkbox', { name: 'A plan is ready for review' })).not.toBeInTheDocument();
    });
  });
});
