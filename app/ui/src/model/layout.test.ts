import { describe, expect, it } from 'vitest';
import { adjust, choose, fromView, type Layout, isAdjusted, MAX_PANEL, MIN_PANEL, panelWidth, parseLayout, setWidth, storedLayout } from './layout';

describe('layout presets', () => {
  it('arranges each preset', () => {
    expect(choose('overview')).toMatchObject({ files: true, timeline: true, filesTab: 'documents', steps: 'summary', terminalTab: false });
    expect(choose('developer')).toMatchObject({ files: true, timeline: true, filesTab: 'code', steps: 'details', terminalTab: true });
    expect(choose('present')).toMatchObject({ files: false, timeline: false, terminalTab: true });
  });

  it('gives Overview a wider timeline', () => {
    expect(panelWidth(choose('overview'), 'timeline')).toBe(400);
    expect(panelWidth(choose('developer'), 'timeline')).toBe(340);
    expect(panelWidth(choose('developer'), 'files')).toBe(230);
  });

  it('shows a change by hand as adjusted, until the preset is chosen again', () => {
    const changed = adjust(choose('developer'), { files: false });
    expect(changed.preset).toBe('developer');
    expect(isAdjusted(changed)).toBe(true);
    const again = choose('developer', changed);
    expect(again.files).toBe(true);
    expect(isAdjusted(again)).toBe(false);
  });

  it('counts a change back as no change', () => {
    const layout = adjust(adjust(choose('developer'), { files: false }), { files: true });
    expect(isAdjusted(layout)).toBe(false);
  });

  it('does not count the terminal tab for the host', () => {
    const layout = adjust(choose('overview'), { terminalTab: true });
    expect(isAdjusted(layout)).toBe(true);
    expect(isAdjusted(layout, true)).toBe(false);
  });

  it('keeps widths per preset', () => {
    const overview = setWidth(choose('overview'), 'timeline', 500);
    const developer = choose('developer', overview);
    expect(panelWidth(developer, 'timeline')).toBe(340);
    expect(panelWidth(choose('overview', developer), 'timeline')).toBe(500);
  });

  it('keeps widths within bounds', () => {
    expect(panelWidth(setWidth(choose('developer'), 'files', 10), 'files')).toBe(MIN_PANEL);
    expect(panelWidth(setWidth(choose('developer'), 'files', 5000), 'files')).toBe(MAX_PANEL);
  });
});

describe('stored layouts', () => {
  it('reads back what was stored', () => {
    const layout = setWidth(adjust(choose('developer'), { files: false }), 'timeline', 420);
    expect(parseLayout(JSON.parse(JSON.stringify(layout)))).toEqual(layout);
  });

  it('ignores what it cannot read', () => {
    expect(parseLayout(null)).toBeNull();
    expect(parseLayout({ preset: 'cinema' })).toBeNull();
    expect(parseLayout({ ...choose('developer'), steps: 'some' })).toBeNull();
  });

  it('fills in missing widths', () => {
    const stored: Partial<Layout> = choose('present');
    delete stored.widths;
    expect(parseLayout(stored)?.widths.overview.timeline).toBe(400);
  });

  it('is unset until chosen', () => {
    expect(storedLayout(null, null, null)).toBeNull();
  });
});

describe('earlier view settings', () => {
  it('turns the terminal off into Overview', () => {
    const layout = fromView({ terminal: false, steps: 'summary' });
    expect(layout.preset).toBe('overview');
    expect(isAdjusted(layout)).toBe(false);
  });

  it('turns the terminal on with details into Developer', () => {
    const layout = fromView({ terminal: true, steps: 'details' });
    expect(layout.preset).toBe('developer');
    expect(isAdjusted(layout)).toBe(false);
  });

  it('keeps summary steps with the terminal on, as an adjusted Developer', () => {
    const layout = fromView({ terminal: true, steps: 'summary' });
    expect(layout).toMatchObject({ preset: 'developer', steps: 'summary' });
    expect(isAdjusted(layout)).toBe(true);
  });

  it('makes the old side panel width the timeline width of every preset', () => {
    const layout = storedLayout(null, { terminal: true, steps: 'details' }, '480')!;
    expect(layout.preset).toBe('developer');
    expect(layout.widths.overview.timeline).toBe(480);
    expect(layout.widths.developer.timeline).toBe(480);
    expect(layout.widths.developer.files).toBe(230);
  });

  it('prefers a stored layout over the old view', () => {
    const stored = choose('present');
    expect(storedLayout(stored, { terminal: false, steps: 'summary' }, '480')?.preset).toBe('present');
  });
});
