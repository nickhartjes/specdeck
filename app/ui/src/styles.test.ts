import { describe, expect, it } from 'vitest';
import css from './styles.css?raw';

/** The declarations of the first rule whose selector ends with `selector`, in order. */
function declarations(selector: string): string[] {
  const open = css.indexOf(`${selector} {`);
  expect(open).toBeGreaterThan(-1);
  const body = css.slice(open + selector.length + 2, css.indexOf('}', open));
  return body
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split(';')
    .map((line) => line.trim())
    .filter(Boolean);
}

describe('dark tokens', () => {
  it('are the same whether chosen in the app or taken from the OS', () => {
    const chosen = declarations(':root[data-theme="dark"]');
    expect(chosen).toContain('color-scheme: dark');
    expect(declarations('  :root:not([data-theme="light"])')).toEqual(chosen);
  });

  it('keep the terminal dark', () => {
    expect(declarations(':root[data-theme="dark"]')).toContain('--terminal-bg: #091312');
    expect(declarations(':root')).toContain('--terminal-bg: #102221');
  });
});

describe('start screen accent', () => {
  it('is blue in both appearances, not plum', () => {
    const accents = [...css.matchAll(/^\s*(\S.*start\)*) \{ --accent: (#[0-9a-f]+);/gm)].map((match) => [match[1]!.trim(), match[2]]);
    expect(accents).toEqual([
      ['.start, #root:has(> .start)', '#3d5a99'],
      [':root[data-theme="dark"] :is(.start, #root:has(> .start))', '#8ea6e0'],
      [':root:not([data-theme="light"]) :is(.start, #root:has(> .start))', '#8ea6e0'],
    ]);
  });
});

describe('side panels', () => {
  it('keep their content as wide as the panel, so nothing is cut off at its edge (#86)', () => {
    expect(declarations('.pane')).toContain('grid-template-columns: minmax(0, 1fr)');
    expect(declarations('.pane-tab .label')).toEqual(['overflow: hidden', 'text-overflow: ellipsis', 'white-space: nowrap']);
  });
});
