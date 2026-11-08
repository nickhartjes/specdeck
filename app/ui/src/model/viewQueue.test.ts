import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createViewQueue, frameSchedule, HIDDEN_FLUSH_MS, type Schedule } from './viewQueue';

/** A schedule the test runs by hand. */
function manual() {
  const pending: (() => void)[] = [];
  const schedule: Schedule = vi.fn((flush: () => void) => {
    pending.push(flush);
    return () => void pending.splice(pending.indexOf(flush), 1);
  });
  return { schedule, frame: () => pending.splice(0).forEach((flush) => flush()), pending };
}

describe('the view queue', () => {
  it('applies what arrived before a frame as one batch, in order', () => {
    const { schedule, frame } = manual();
    const apply = vi.fn();
    const queue = createViewQueue<number>(apply, schedule);
    queue.push(1);
    queue.push(2);
    queue.push(3);
    expect(apply).not.toHaveBeenCalled();
    frame();
    expect(apply.mock.calls).toEqual([[[1, 2, 3]]]);
    queue.push(4);
    frame();
    expect(apply.mock.calls).toEqual([[[1, 2, 3]], [[4]]]);
  });

  it('schedules once per batch', () => {
    const { schedule, frame } = manual();
    const queue = createViewQueue<number>(() => {}, schedule);
    queue.push(1);
    queue.push(2);
    expect(schedule).toHaveBeenCalledTimes(1);
    frame();
    queue.push(3);
    expect(schedule).toHaveBeenCalledTimes(2);
  });

  it('does nothing when flushed with nothing queued', () => {
    const apply = vi.fn();
    const queue = createViewQueue<number>(apply, manual().schedule);
    queue.flush();
    expect(apply).not.toHaveBeenCalled();
  });

  it('flushes on demand and cancels the frame it scheduled', () => {
    const { schedule, pending } = manual();
    const apply = vi.fn();
    const queue = createViewQueue<number>(apply, schedule);
    queue.push(1);
    queue.flush();
    expect(apply.mock.calls).toEqual([[[1]]]);
    expect(pending).toHaveLength(0);
  });

  it('drops what queued when disposed', () => {
    const { schedule, frame, pending } = manual();
    const apply = vi.fn();
    const queue = createViewQueue<number>(apply, schedule);
    queue.push(1);
    queue.dispose();
    expect(pending).toHaveLength(0);
    frame();
    queue.flush();
    expect(apply).not.toHaveBeenCalled();
  });

  it('keeps working with a schedule that flushes at once', () => {
    const apply = vi.fn();
    const queue = createViewQueue<number>(apply, (flush) => {
      flush();
      return () => {};
    });
    queue.push(1);
    queue.push(2);
    expect(apply.mock.calls).toEqual([[[1]], [[2]]]);
  });
});

describe('the frame schedule', () => {
  beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'requestAnimationFrame', 'cancelAnimationFrame'] }));
  afterEach(() => vi.useRealTimers());

  it('flushes on the next frame and cancels the timer', () => {
    const flush = vi.fn();
    frameSchedule(flush);
    vi.advanceTimersByTime(16);
    expect(flush).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(HIDDEN_FLUSH_MS);
    expect(flush).toHaveBeenCalledTimes(1);
  });

  it('flushes after the timer when no frame comes, as in a hidden window, and cancels the frame', () => {
    const raf = vi.spyOn(window, 'requestAnimationFrame').mockImplementation(() => 7);
    const cancelRaf = vi.spyOn(window, 'cancelAnimationFrame');
    const flush = vi.fn();
    frameSchedule(flush);
    vi.advanceTimersByTime(HIDDEN_FLUSH_MS);
    expect(flush).toHaveBeenCalledTimes(1);
    expect(cancelRaf).toHaveBeenCalledWith(7);
    raf.mockRestore();
    cancelRaf.mockRestore();
  });

  it('runs neither once cancelled', () => {
    const flush = vi.fn();
    const cancel = frameSchedule(flush);
    cancel();
    vi.advanceTimersByTime(HIDDEN_FLUSH_MS * 2);
    expect(flush).not.toHaveBeenCalled();
  });
});
