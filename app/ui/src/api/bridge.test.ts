import { describe, expect, it, vi } from 'vitest';
import { errorMessage } from './bridge';

describe('errorMessage', () => {
  it('passes on what the app said', () => {
    expect(errorMessage('Not hosting')).toBe('Not hosting');
    expect(errorMessage(new Error('Lost the session'))).toBe('Lost the session');
    expect(errorMessage(42)).toBe('Something went wrong.');
  });

  it('explains a command the running app does not have', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    expect(errorMessage('command host_share_code not found')).toBe(
      'This window is newer than the SpecDeck app running it. Quit SpecDeck and open it again.',
    );
    expect(warn).toHaveBeenCalledWith('command host_share_code not found');
    warn.mockRestore();
  });
});
