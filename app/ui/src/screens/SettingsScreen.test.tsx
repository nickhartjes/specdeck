import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { demoProfile } from '../api/demo';
import { fakeBridge, renderWith } from '../test/helpers';
import { SettingsScreen } from './SettingsScreen';

describe('settings window', () => {
  it('changes the name when the field loses focus or on Enter', async () => {
    const bridge = fakeBridge();
    renderWith(bridge, <SettingsScreen />);
    const name = await screen.findByLabelText('Your name');
    expect(name).toHaveValue('Nick');
    await userEvent.clear(name);
    await userEvent.type(name, 'Anna{Enter}');
    expect(bridge.setUsername).toHaveBeenCalledWith('Anna');
    expect(await screen.findByText(/Saved/)).toBeInTheDocument();
  });

  it('does not accept an empty name', async () => {
    const bridge = fakeBridge();
    renderWith(bridge, <SettingsScreen />);
    const name = await screen.findByLabelText('Your name');
    await userEvent.clear(name);
    await userEvent.tab();
    expect(screen.getByRole('alert')).toHaveTextContent('Enter the name others should see.');
    expect(bridge.setUsername).not.toHaveBeenCalled();
  });

  it('turns notifications off and on', async () => {
    const bridge = fakeBridge();
    renderWith(bridge, <SettingsScreen />);
    const toggle = await screen.findByRole('checkbox', { name: 'Notifications' });
    expect(toggle).toBeChecked();
    await userEvent.click(toggle);
    expect(bridge.setNotifications).toHaveBeenCalledWith(false);
    await waitFor(() => expect(toggle).not.toBeChecked());
  });
});

describe('screen reader setting', () => {
  it('is off by default and turns on at once', async () => {
    const bridge = fakeBridge();
    renderWith(bridge, <SettingsScreen />);
    const toggle = await screen.findByRole('checkbox', { name: 'Screen reader' });
    expect(toggle).not.toBeChecked();
    await userEvent.click(toggle);
    expect(bridge.setScreenReader).toHaveBeenCalledWith(true);
    await waitFor(() => expect(toggle).toBeChecked());
  });

  it('shows when it is on', async () => {
    renderWith(fakeBridge('start', { getProfile: async () => ({ ...demoProfile, screenReader: true }) }), <SettingsScreen />);
    expect(await screen.findByRole('checkbox', { name: 'Screen reader' })).toBeChecked();
  });
});

describe('verbose logging setting', () => {
  it('is off by default and turns on at once', async () => {
    const bridge = fakeBridge();
    renderWith(bridge, <SettingsScreen />);
    const toggle = await screen.findByRole('checkbox', { name: 'Diagnostics' });
    expect(toggle).not.toBeChecked();
    await userEvent.click(toggle);
    expect(bridge.setVerboseLogging).toHaveBeenCalledWith(true);
    await waitFor(() => expect(toggle).toBeChecked());
  });
});

describe('browser setting', () => {
  it('is on by default on macOS and turns off at once', async () => {
    const bridge = fakeBridge();
    renderWith(bridge, <SettingsScreen />);
    const toggle = await screen.findByRole('checkbox', { name: 'Browser' });
    expect(toggle).toBeChecked();
    await userEvent.click(toggle);
    expect(bridge.setBrowser).toHaveBeenCalledWith(false);
    await waitFor(() => expect(toggle).not.toBeChecked());
  });

  it('is not offered on other platforms', async () => {
    renderWith(fakeBridge('start', { getProfile: async () => ({ ...demoProfile, platform: 'windows' }) }), <SettingsScreen />);
    await screen.findByRole('checkbox', { name: 'Diagnostics' });
    expect(screen.queryByRole('checkbox', { name: 'Browser' })).toBeNull();
  });
});

describe('update setting', () => {
  it('is on by default and turns off at once', async () => {
    const bridge = fakeBridge();
    renderWith(bridge, <SettingsScreen />);
    const toggle = await screen.findByRole('checkbox', { name: 'Updates' });
    expect(toggle).toBeChecked();
    await userEvent.click(toggle);
    expect(bridge.setCheckUpdates).toHaveBeenCalledWith(false);
    await waitFor(() => expect(toggle).not.toBeChecked());
  });

  it('shows when it is off', async () => {
    renderWith(fakeBridge('start', { getProfile: async () => ({ ...demoProfile, checkUpdates: false }) }), <SettingsScreen />);
    expect(await screen.findByRole('checkbox', { name: 'Updates' })).not.toBeChecked();
  });

  it('is not offered on installs that do not update themselves', async () => {
    renderWith(fakeBridge('start', { getProfile: async () => ({ ...demoProfile, updatesItself: false }) }), <SettingsScreen />);
    await screen.findByRole('checkbox', { name: 'Notifications' });
    expect(screen.queryByRole('checkbox', { name: 'Updates' })).not.toBeInTheDocument();
    expect(screen.queryByText(/update/i)).not.toBeInTheDocument();
  });
});

describe('sharing service setting', () => {
  const field = () => screen.findByLabelText('Sharing service');

  it('uses the built-in service until one is entered', async () => {
    renderWith(fakeBridge(), <SettingsScreen />);
    const input = await field();
    expect(input).toHaveValue('');
    expect(input).toHaveAttribute('placeholder', 'specdeck.hartj.es');
    expect(screen.getByText(/Leave empty for the built-in service/)).toBeInTheDocument();
  });

  it('saves a domain as the full address and says it can be reached', async () => {
    const bridge = fakeBridge();
    renderWith(bridge, <SettingsScreen />);
    await userEvent.type(await field(), 'specdeck.example.com{Enter}');
    expect(bridge.setSharingService).toHaveBeenCalledWith('specdeck.example.com');
    expect(bridge.checkSharingService).toHaveBeenCalledWith('wss://specdeck.example.com/signal');
    expect(await screen.findByText('Saved. specdeck.example.com can be reached.')).toBeInTheDocument();
    expect(await field()).toHaveValue('wss://specdeck.example.com/signal');
  });

  it('saves a service that cannot be reached, and says why', async () => {
    const bridge = fakeBridge('start', { checkSharingService: vi.fn(async () => ({ reachable: false, problem: 'Nothing answers at that address.' })) });
    renderWith(bridge, <SettingsScreen />);
    await userEvent.type(await field(), 'specdeck.example.com');
    await userEvent.tab();
    expect(await screen.findByText('Saved, but specdeck.example.com cannot be reached. Nothing answers at that address.')).toBeInTheDocument();
    expect(bridge.setSharingService).toHaveBeenCalled();
  });

  it('marks a service without encryption', async () => {
    const local = { ...demoProfile, sharingService: 'ws://127.0.0.1:18081/signal', effectiveService: 'ws://127.0.0.1:18081/signal' };
    const bridge = fakeBridge('start', { setSharingService: vi.fn(async () => local) });
    renderWith(bridge, <SettingsScreen />);
    await userEvent.type(await field(), 'ws://127.0.0.1:18081{Enter}');
    expect(await screen.findByText('Saved. 127.0.0.1:18081 can be reached. Not encrypted.')).toBeInTheDocument();
  });

  it('shows why an address is refused', async () => {
    const bridge = fakeBridge('start', {
      setSharingService: vi.fn(async () => Promise.reject('Enter a domain such as specdeck.example.com, or a URL such as wss://specdeck.example.com/signal.')),
    });
    renderWith(bridge, <SettingsScreen />);
    await userEvent.type(await field(), 'https://example.com{Enter}');
    expect(await screen.findByRole('alert')).toHaveTextContent('Enter a domain such as specdeck.example.com');
    expect(bridge.checkSharingService).not.toHaveBeenCalled();
  });

  it('cannot change during a session', async () => {
    renderWith(fakeBridge('start', { getProfile: vi.fn(async () => ({ ...demoProfile, inSession: true })) }), <SettingsScreen />);
    expect(await field()).toBeDisabled();
    expect(screen.getByText('Applies to the next session you start or join.')).toBeInTheDocument();
  });

  it('cannot change when the environment decides the service', async () => {
    const profile = { ...demoProfile, serviceOverridden: true, effectiveService: 'ws://127.0.0.1:18081/signal' };
    renderWith(fakeBridge('start', { getProfile: vi.fn(async () => profile) }), <SettingsScreen />);
    expect(await field()).toBeDisabled();
    expect(screen.getByText('Set to 127.0.0.1:18081 by SPECDECK_SIGNALLING_URL when SpecDeck started.')).toBeInTheDocument();
  });
});
