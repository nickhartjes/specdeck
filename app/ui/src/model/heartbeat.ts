// The main window's heartbeat for diagnostics: once a second while visible, so the app notices
// when the window's thread is stuck (design decision 11 of the diagnostics change). The
// element count is read every 10th beat, since counting a large page is work too.

export interface Heartbeat {
  visible: boolean;
  timelineEvents: number;
  elements: number | null;
}

export const BEAT_MS = 1_000;
const ELEMENTS_EVERY = 10;

let timelineEvents = 0;

/** The session screen says how many timeline events it holds. */
export function setTimelineEvents(count: number) {
  timelineEvents = count;
}

/** Starts beating; returns a function that stops. A hidden page says so once and then rests. */
export function startHeartbeat(send: (beat: Heartbeat) => void): () => void {
  let beats = 0;
  const visible = () => document.visibilityState !== 'hidden';
  const tick = () => {
    if (!visible()) return;
    const elements = beats % ELEMENTS_EVERY === 0 ? document.getElementsByTagName('*').length : null;
    beats += 1;
    send({ visible: true, timelineEvents, elements });
  };
  const onVisibility = () => {
    if (visible()) tick();
    else send({ visible: false, timelineEvents, elements: null });
  };
  tick();
  const timer = window.setInterval(tick, BEAT_MS);
  document.addEventListener('visibilitychange', onVisibility);
  return () => {
    window.clearInterval(timer);
    document.removeEventListener('visibilitychange', onVisibility);
  };
}
