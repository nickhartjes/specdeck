import { afterEach, describe, expect, it, vi } from 'vitest';
import { applyAppearance, asAppearance, onAppearanceChange } from './appearance';

afterEach(() => {
  delete document.documentElement.dataset.theme;
  vi.unstubAllGlobals();
});

describe('appearance', () => {
  it('marks Light and Dark on the page, and leaves System to the OS', () => {
    applyAppearance('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    applyAppearance('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    applyAppearance('system');
    expect(document.documentElement).not.toHaveAttribute('data-theme');
  });

  it('reads anything unknown as System', () => {
    expect(asAppearance('dark')).toBe('dark');
    expect(asAppearance('sepia')).toBe('system');
    expect(asAppearance(null)).toBe('system');
  });

  it('tells the terminal when the appearance is chosen in the app or changes in the OS', async () => {
    const media = { addEventListener: vi.fn(), removeEventListener: vi.fn() };
    vi.stubGlobal('matchMedia', () => media);
    const onChange = vi.fn();
    const stop = onAppearanceChange(onChange);
    expect(media.addEventListener).toHaveBeenCalledWith('change', onChange);

    applyAppearance('dark');
    await vi.waitFor(() => expect(onChange).toHaveBeenCalledTimes(1));

    stop();
    expect(media.removeEventListener).toHaveBeenCalledWith('change', onChange);
    applyAppearance('light');
    await new Promise((resolve) => setTimeout(resolve));
    expect(onChange).toHaveBeenCalledTimes(1);
  });
});
