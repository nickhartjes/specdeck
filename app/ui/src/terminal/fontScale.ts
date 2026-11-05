// A viewer's terminal keeps the host's columns and rows and scales its text to the pane instead.

/** The range a viewer's text is scaled in to fit the host's terminal in its pane. */
export const MIN_VIEWER_FONT = 6;
/**
 * A little above the usual 13 px: a host terminal narrower than the pane stays readable, with room
 * beside it, instead of growing huge in a wide window or a maximized browser.
 */
export const MAX_VIEWER_FONT = 16;

/**
 * The font size that makes text drawn `drawn` pixels wide at `current` fill `width` pixels,
 * in quarter pixels and within the viewer's range.
 */
export function viewerFontSize(current: number, width: number, drawn: number): number {
  return clampViewerFont((current * width) / drawn);
}

export function clampViewerFont(size: number): number {
  return Math.min(MAX_VIEWER_FONT, Math.max(MIN_VIEWER_FONT, Math.floor(size * 4) / 4));
}
