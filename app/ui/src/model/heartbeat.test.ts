import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BEAT_MS, setTimelineEvents, startHeartbeat } from './heartbeat';

let state: DocumentVisibilityState = 'visible';

function setVisibility(next: DocumentVisibilityState) {
  state = next;
  document.dispatchEvent(new Event('visibilitychange'));
}

describe('heartbeat', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    state = 'visible';
    vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => state);
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('beats once a second, counting elements every tenth beat', () => {
    const send = vi.fn();
    setTimelineEvents(42);
    const stop = startHeartbeat(send);
    vi.advanceTimersByTime(BEAT_MS * 10);
    expect(send).toHaveBeenCalledTimes(11);
    const counted = send.mock.calls.filter(([beat]) => beat.elements !== null);
    expect(counted).toHaveLength(2);
    expect(send).toHaveBeenLastCalledWith(expect.objectContaining({ visible: true, timelineEvents: 42 }));
    stop();
  });

  it('says once that the page is hidden and then rests until it is shown', () => {
    const send = vi.fn();
    const stop = startHeartbeat(send);
    send.mockClear();
    setVisibility('hidden');
    vi.advanceTimersByTime(BEAT_MS * 30);
    expect(send.mock.calls).toEqual([[{ visible: false, timelineEvents: expect.any(Number), elements: null }]]);
    setVisibility('visible');
    expect(send).toHaveBeenLastCalledWith(expect.objectContaining({ visible: true }));
    stop();
  });

  it('stops', () => {
    const send = vi.fn();
    startHeartbeat(send)();
    send.mockClear();
    vi.advanceTimersByTime(BEAT_MS * 5);
    setVisibility('hidden');
    expect(send).not.toHaveBeenCalled();
  });
});
