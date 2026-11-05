import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TerminalSnapshot } from '../api/types';
import { REPORT_INTERVAL, TerminalReporter } from './terminalState';

const base: TerminalSnapshot = {
  screen: 'normal',
  mouseTracking: 'none',
  scrollbackLines: 0,
  scrollbackLimit: 100_000,
  linesFromBottom: 0,
  following: true,
  fontSize: 13,
  recordsWritten: 0,
};

describe('terminal state reports', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('reports a change of mouse tracking at once', () => {
    const send = vi.fn();
    const reporter = new TerminalReporter(send);
    reporter.update(base);
    reporter.update({ ...base, recordsWritten: 5 });
    expect(send).toHaveBeenCalledTimes(1);
    reporter.update({ ...base, recordsWritten: 6, mouseTracking: 'any' });
    expect(send).toHaveBeenCalledTimes(2);
    expect(send).toHaveBeenLastCalledWith(expect.objectContaining({ mouseTracking: 'any' }));
    reporter.dispose();
  });

  it('reports a minute of scrolling at most 12 times, with the latest state', () => {
    const send = vi.fn();
    const reporter = new TerminalReporter(send);
    reporter.update(base);
    send.mockClear();
    let records = 0;
    for (let ms = 0; ms < 60_000; ms += 16) {
      records += 1;
      reporter.update({ ...base, recordsWritten: records, scrollbackLines: Math.min(records, base.scrollbackLimit) });
      vi.advanceTimersByTime(16);
    }
    vi.advanceTimersByTime(REPORT_INTERVAL);
    expect(send.mock.calls.length).toBeLessThanOrEqual(12);
    expect(send.mock.calls.length).toBeGreaterThanOrEqual(11);
    expect(send).toHaveBeenLastCalledWith(expect.objectContaining({ recordsWritten: records }));
    reporter.dispose();
  });

  it('reports nothing while nothing changes', () => {
    const send = vi.fn();
    const reporter = new TerminalReporter(send);
    reporter.update(base);
    for (let i = 0; i < 100; i++) reporter.update({ ...base });
    vi.advanceTimersByTime(60_000);
    expect(send).toHaveBeenCalledTimes(1);
    reporter.dispose();
  });

  it('holds only enums and numbers', () => {
    const send = vi.fn();
    const reporter = new TerminalReporter(send);
    reporter.update(base);
    const [snapshot] = send.mock.calls[0] as [TerminalSnapshot];
    for (const value of Object.values(snapshot)) {
      expect(['number', 'boolean', 'string'].includes(typeof value)).toBe(true);
      if (typeof value === 'string') expect(value).toMatch(/^[a-z0-9]+$/);
    }
    reporter.dispose();
  });
});
