// Applying what arrives together as one batch (`replay-batching` design decisions 2, 4 and 5):
// a viewer joining a long session gets its history as thousands of events, and rendering the
// session view once per event made the replay grow with the square of the history. Items queue
// until the next frame, so a slow render lets more of them gather for the next one.

/** Runs `flush` later; returns a function that cancels it. */
export type Schedule = (flush: () => void) => () => void;

/** How long a hidden window, which gets no frames, waits before it applies what queued. */
export const HIDDEN_FLUSH_MS = 250;

/** The next animation frame, or the timer when frames stop, whichever comes first. */
export const frameSchedule: Schedule = (flush) => {
  let frame = 0;
  let timer = 0;
  const run = () => {
    cancelAnimationFrame(frame);
    clearTimeout(timer);
    flush();
  };
  frame = requestAnimationFrame(run);
  timer = window.setTimeout(run, HIDDEN_FLUSH_MS);
  return () => {
    cancelAnimationFrame(frame);
    clearTimeout(timer);
  };
};

let defaultSchedule: Schedule = frameSchedule;

/** Tests flush at once, unless they pass a schedule of their own. */
export function setDefaultSchedule(schedule: Schedule) {
  defaultSchedule = schedule;
}

export interface ViewQueue<T> {
  push(item: T): void;
  /** Applies what queued now, if anything did. */
  flush(): void;
  /** Cancels a pending flush and drops what queued. */
  dispose(): void;
}

export function createViewQueue<T>(apply: (items: T[]) => void, schedule: Schedule = defaultSchedule): ViewQueue<T> {
  let queued: T[] = [];
  let pending = false;
  let cancel: (() => void) | null = null;
  const flush = () => {
    pending = false;
    cancel?.();
    cancel = null;
    if (queued.length === 0) return;
    const items = queued;
    queued = [];
    apply(items);
  };
  return {
    push(item) {
      queued.push(item);
      if (pending) return;
      pending = true;
      const stop = schedule(flush);
      // A schedule that flushes at once has nothing left to cancel.
      if (pending) cancel = stop;
    },
    flush,
    dispose() {
      pending = false;
      cancel?.();
      cancel = null;
      queued = [];
    },
  };
}
