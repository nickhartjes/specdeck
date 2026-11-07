import { afterEach, describe, expect, it, vi } from 'vitest';
import { currentTheme, terminalFont } from './TerminalView';

afterEach(() => {
  vi.unstubAllGlobals();
  document.documentElement.style.removeProperty('--terminal-bg');
  document.documentElement.style.removeProperty('--terminal-font');
});

describe('the terminal’s look', () => {
  it('is dark in the light appearance too, so the agent picks colours for a dark background', () => {
    vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }));
    const theme = currentTheme();
    expect(theme.background).toBe('#102221');
    expect(theme.foreground).toBe('#d6e3df');
  });

  it('takes the stage colour of the appearance', () => {
    document.documentElement.style.setProperty('--terminal-bg', '#091312');
    expect(currentTheme().background).toBe('#091312');
    expect(currentTheme().cursorAccent).toBe('#091312');
  });

  it('draws text in the platform’s monospace font, not the app’s type', () => {
    expect(terminalFont()).not.toMatch(/Atkinson/);
    document.documentElement.style.setProperty('--terminal-font', 'ui-monospace, Menlo, monospace');
    expect(terminalFont()).toBe('ui-monospace, Menlo, monospace');
  });
});
