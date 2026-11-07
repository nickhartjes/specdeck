import type { LeftOut, Screenshot, ScreenshotImage } from '../api/types';

/** The widest a thumbnail in the timeline is, in CSS pixels. */
const THUMBNAIL_WIDTH = 240;
/** Only these are shown: the host sends JPEG or PNG, and nothing else is trusted to be a picture. */
const MEDIA_TYPES = new Set(['image/jpeg', 'image/png']);

const LEFT_OUT: Record<LeftOut, string> = {
  'turned-off': "Screenshots aren't shown in this session.",
  'too-large': 'Too large to show.',
  unreadable: "Couldn't be shown.",
};

/** What a screenshot's tab is called: its page, or just "Screenshot". */
export function screenshotTitle(screenshot: Screenshot): string {
  return screenshot.address ?? 'Screenshot';
}

function source(image: ScreenshotImage): string | null {
  return MEDIA_TYPES.has(image.mediaType) ? `data:${image.mediaType};base64,${image.data}` : null;
}

function alt(screenshot: Screenshot): string {
  return screenshot.address ? `Screenshot of ${screenshot.address}` : 'Screenshot';
}

/** Why the image is missing, in place of the picture. */
function Missing({ screenshot }: { screenshot: Screenshot }) {
  return <p className="hint screenshot-missing">Screenshot: {LEFT_OUT[screenshot.leftOut ?? 'unreadable']}</p>;
}

/**
 * A screenshot in the timeline, small, with its page; a click opens it full size in a preview tab,
 * a double click keeps that tab.
 */
export function ScreenshotThumbnail({ screenshot, onOpen }: { screenshot: Screenshot; onOpen?: (keep: boolean) => void }) {
  const src = screenshot.image && source(screenshot.image);
  if (!screenshot.image || !src) return <Missing screenshot={screenshot} />;
  const { width, height } = screenshot.image;
  const scale = Math.min(1, THUMBNAIL_WIDTH / Math.max(width, 1));
  return (
    <figure className="screenshot-thumbnail">
      <button
        type="button"
        title="Open full size"
        onClick={() => onOpen?.(false)}
        onDoubleClick={() => onOpen?.(true)}
        disabled={!onOpen}
      >
        <img src={src} alt={alt(screenshot)} width={Math.round(width * scale)} height={Math.round(height * scale)} />
      </button>
      {screenshot.address && <figcaption className="selectable">{screenshot.address}</figcaption>}
    </figure>
  );
}

/** A screenshot full size, in a tab of its own. */
export function ScreenshotTab({ screenshot }: { screenshot: Screenshot | null }) {
  if (!screenshot) return <p className="hint screenshot-missing">This screenshot is no longer in the timeline.</p>;
  const src = screenshot.image && source(screenshot.image);
  return (
    <div className="screenshot-tab">
      <div className="filebar">
        <span className="path selectable">{screenshotTitle(screenshot)}</span>
      </div>
      {screenshot.image && src ? (
        <img src={src} alt={alt(screenshot)} width={screenshot.image.width} height={screenshot.image.height} />
      ) : (
        <Missing screenshot={screenshot} />
      )}
    </div>
  );
}
