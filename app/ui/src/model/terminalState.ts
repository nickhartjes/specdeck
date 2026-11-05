// When the terminal reports its state for diagnostics: a change of screen, mouse tracking or
// following at once, anything else at most every 5 seconds and only when something changed.

import type { TerminalSnapshot } from '../api/types';

export const REPORT_INTERVAL = 5_000;

function sameMode(a: TerminalSnapshot, b: TerminalSnapshot): boolean {
  return a.screen === b.screen && a.mouseTracking === b.mouseTracking && a.following === b.following;
}

function same(a: TerminalSnapshot, b: TerminalSnapshot): boolean {
  return (Object.keys(a) as (keyof TerminalSnapshot)[]).every((key) => a[key] === b[key]);
}

export class TerminalReporter {
  private sent: TerminalSnapshot | null = null;
  private sentAt = 0;
  private pending: TerminalSnapshot | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly send: (snapshot: TerminalSnapshot) => void) {}

  update(snapshot: TerminalSnapshot) {
    if (this.sent && same(snapshot, this.sent)) {
      this.pending = null;
      return;
    }
    if (!this.sent || !sameMode(snapshot, this.sent)) {
      this.flush(snapshot);
      return;
    }
    this.pending = snapshot;
    if (this.timer === undefined) {
      const wait = Math.max(0, this.sentAt + REPORT_INTERVAL - Date.now());
      this.timer = setTimeout(() => {
        this.timer = undefined;
        if (this.pending) this.flush(this.pending);
      }, wait);
    }
  }

  dispose() {
    clearTimeout(this.timer);
    this.timer = undefined;
  }

  private flush(snapshot: TerminalSnapshot) {
    clearTimeout(this.timer);
    this.timer = undefined;
    this.pending = null;
    this.sent = snapshot;
    this.sentAt = Date.now();
    this.send(snapshot);
  }
}
