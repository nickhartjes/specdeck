// The app's appearance on the page (`appearance-setting` design decision 4): `data-theme` on
// <html> for Light or Dark, none for System, where the OS or browser decides.

import type { Appearance } from './api/types';

export const APPEARANCES: { value: Appearance; label: string }[] = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
];

/** A stored value as an appearance; anything else follows the OS. */
export function asAppearance(value: unknown): Appearance {
  return value === 'light' || value === 'dark' ? value : 'system';
}

export function applyAppearance(appearance: Appearance, root: HTMLElement = document.documentElement) {
  if (appearance === 'system') delete root.dataset.theme;
  else root.dataset.theme = appearance;
}

/**
 * Calls `onChange` when the page's appearance may have changed: the OS's (or browser's) under
 * System, or Light or Dark chosen in the app. Returns a function that stops watching.
 */
export function onAppearanceChange(onChange: () => void): () => void {
  const media = window.matchMedia?.('(prefers-color-scheme: dark)');
  media?.addEventListener('change', onChange);
  const chosen = new MutationObserver(onChange);
  chosen.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  return () => {
    media?.removeEventListener('change', onChange);
    chosen.disconnect();
  };
}
