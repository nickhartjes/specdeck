import { describe, expect, it } from 'vitest';
import { MAX_VIEWER_FONT, MIN_VIEWER_FONT, viewerFontSize } from './fontScale';

describe('a viewer’s terminal font', () => {
  it('scales the host’s columns to the pane', () => {
    // Drawn 600 px wide at 13 px; the pane has 480 px.
    expect(viewerFontSize(13, 480, 600)).toBe(10.25);
  });

  it('grows to at most 16 px when the pane is much wider than the host’s terminal', () => {
    // An 80-column host in a maximized browser: 3.5 times the room.
    expect(viewerFontSize(13, 2100, 600)).toBe(MAX_VIEWER_FONT);
    expect(MAX_VIEWER_FONT).toBe(16);
  });

  it('shrinks no further than 6 px', () => {
    expect(viewerFontSize(13, 100, 600)).toBe(MIN_VIEWER_FONT);
  });
});
