import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ViewEvent } from '../api/types';
import { Connection, refusalReason, type ControlUpdate, type Platform, type ViewerCore } from './connection';

/** A WebSocket to the sharing service, driven by the test. */
class FakeSocket {
  sent: Record<string, unknown>[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  closed = false;
  send(text: string) {
    this.sent.push(JSON.parse(text) as Record<string, unknown>);
  }
  close() {
    this.closed = true;
  }
  open() {
    this.onopen?.();
  }
  receive(message: object) {
    this.onmessage?.({ data: JSON.stringify(message) });
  }
}

class FakeChannel {
  binaryType = 'blob';
  readyState = 'connecting';
  sent: Uint8Array[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: ArrayBuffer }) => void) | null = null;
  constructor(readonly label: string) {}
  send(bytes: Uint8Array) {
    this.sent.push(bytes);
  }
  open() {
    this.readyState = 'open';
    this.onopen?.();
  }
  receive(bytes: number[]) {
    this.onmessage?.({ data: new Uint8Array(bytes).buffer });
  }
}

class FakePeer {
  channels: FakeChannel[] = [];
  remote: RTCSessionDescriptionInit | null = null;
  candidates: RTCIceCandidateInit[] = [];
  localDescription: { sdp: string } | null = null;
  connectionState = 'new';
  closed = false;
  onicecandidate: ((event: { candidate: { candidate: string; sdpMid: string } | null }) => void) | null = null;
  onconnectionstatechange: (() => void) | null = null;
  constructor(readonly config: RTCConfiguration) {}
  createDataChannel(label: string) {
    const channel = new FakeChannel(label);
    this.channels.push(channel);
    return channel;
  }
  createOffer() {
    return Promise.resolve({ type: 'offer', sdp: 'offer-sdp' });
  }
  setLocalDescription(description: { sdp: string }) {
    this.localDescription = description;
    return Promise.resolve();
  }
  setRemoteDescription(description: RTCSessionDescriptionInit) {
    this.remote = description;
    return Promise.resolve();
  }
  addIceCandidate(candidate: RTCIceCandidateInit) {
    this.candidates.push(candidate);
    return Promise.resolve();
  }
  close() {
    this.closed = true;
  }
  fail() {
    this.connectionState = 'failed';
    this.onconnectionstatechange?.();
  }
  channel(label: string) {
    return this.channels.find((channel) => channel.label === label)!;
  }
}

interface FakeCore extends ViewerCore {
  /** What the next control messages mean. */
  controls: ControlUpdate[];
  answerValid: boolean;
  /** Requests made and not answered, as the Rust core keeps them. */
  unanswered: { id: number; message: Uint8Array }[];
  verifyAnswer: ReturnType<typeof vi.fn<ViewerCore['verifyAnswer']>>;
}

/** The shared Rust core, faked: admitted after a welcome, then rejoining through "rejoin". */
function fakeCore(): FakeCore {
  let admitted = false;
  const core: FakeCore = {
    controls: [] as ControlUpdate[],
    answerValid: true,
    startAttempt: () => (admitted ? 'rejoin' : 'code-room'),
    signOffer: (connection: string, sdp: string) => ({ kind: 'offer', sdp, mac: `mac-${connection}` }),
    verifyAnswer: vi.fn<ViewerCore['verifyAnswer']>((_, description) => (core.answerValid ? (description as { sdp: string }).sdp : undefined)),
    hello: () => new Uint8Array([1]),
    leave: () => new Uint8Array([9]),
    onControl: (): ControlUpdate => {
      const update = core.controls.shift() ?? { type: 'retry' };
      if (update.type === 'welcome') admitted = true;
      return update;
    },
    onStream: () => ({ records: new Uint8Array([7]) }),
    admittedOnce: () => admitted,
    suggest: (text) => {
      if (!text.trim()) throw new Error('A suggestion is 1 to 4096 bytes of text.');
      const request = { id: core.unanswered.length + 1, message: new Uint8Array([3, core.unanswered.length + 1]) };
      core.unanswered.push(request);
      return request;
    },
    unansweredRequests: () => core.unanswered,
    unanswered: [],
    shareFile: (name) => ({ id: 50 + name.length, message: new Uint8Array([3, 50 + name.length]) }),
    removeFile: (file) => ({ id: 90, message: new Uint8Array([3, 90, file]) }),
    // Fetches: [5, file] asks; a reply [1] is a chunk, [2] the end, [3] gone.
    fetchFile: (file) => ({
      request: () => new Uint8Array([5, file]),
      onReply: (bytes) => (bytes[0] === 1 ? { progress: 1 } : bytes[0] === 2 ? { done: new Uint8Array([42]) } : bytes[0] === 3 ? { failed: true, gone: true } : {}),
    }),
    // Uploads: one chunk [6, upload]; a reply [4] finishes it.
    uploadFile: (upload) => {
      let sent = false;
      return {
        nextChunks: () => (sent ? [] : ((sent = true), [new Uint8Array([6, upload])])),
        onReply: (bytes) => (bytes[0] === 4 ? { finished: 'Received' as const } : {}),
      };
    },
  };
  return core;
}

function setup(core = fakeCore()) {
  const sockets: FakeSocket[] = [];
  const peers: FakePeer[] = [];
  const platform = {
    socket: () => {
      const socket = new FakeSocket();
      sockets.push(socket);
      return socket as unknown as WebSocket;
    },
    peer: (config: RTCConfiguration) => {
      const peer = new FakePeer(config);
      peers.push(peer);
      return peer as unknown as RTCPeerConnection;
    },
  } satisfies Platform;
  const events: ViewEvent[] = [];
  const records: Uint8Array[] = [];
  const connection = new Connection({
    url: 'wss://specdeck.example.com/signal',
    username: 'anna',
    core,
    platform,
    onEvent: (event) => events.push(event),
    onRecords: (bytes) => records.push(bytes),
  });
  return { core, sockets, peers, events, records, connection };
}

const welcome: ControlUpdate = { type: 'welcome', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 80, rows: 24, reconnected: false };

/** Gets the newest attempt as far as the host's answer. */
async function reachHost(sockets: FakeSocket[], peers: FakePeer[]) {
  const socket = sockets.at(-1)!;
  socket.open();
  socket.receive({ type: 'joined', connection: 'c1' });
  socket.receive({ type: 'turn-credentials', urls: ['turn:turn.example.com:3478?transport=udp'], username: 'u', credential: 'c', ttlSeconds: 60 });
  await vi.advanceTimersByTimeAsync(0);
  socket.receive({ type: 'signal', connection: 'c1', payload: { type: 'description', kind: 'answer', sdp: 'answer-sdp', mac: 'm' } });
  await vi.advanceTimersByTimeAsync(0);
  return { socket, peer: peers.at(-1)! };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('web viewer connection', () => {
  it('joins the code’s room with TURN, signs its offer and applies a valid answer', async () => {
    const { sockets, peers } = setup();
    const { socket, peer } = await reachHost(sockets, peers);
    // Credentials only after joining: the service gives them only to members.
    expect(socket.sent[0]).toEqual({ type: 'join', room: 'code-room' });
    expect(socket.sent[1]).toEqual({ type: 'turn-credentials' });
    expect(socket.sent[2]).toEqual({
      type: 'signal',
      connection: 'c1',
      payload: { type: 'description', kind: 'offer', sdp: 'offer-sdp', mac: 'mac-c1' },
    });
    expect(peer.config.iceServers).toEqual([{ urls: 'turn:turn.example.com:3478?transport=udp', username: 'u', credential: 'c' }]);
    expect(peer.channels.map((channel) => channel.label)).toEqual(['control', 'stream', 'files']);
    expect(peer.remote).toEqual({ type: 'answer', sdp: 'answer-sdp' });
  });

  it('sends suggestions once admitted, and those left unanswered again after a reconnect', async () => {
    const core = fakeCore();
    core.controls.push(
      welcome,
      { type: 'allowed', suggestions: true },
      { type: 'shared', wholeFiles: true },
      { type: 'reply', id: 2, outcome: 'Received' },
      { ...welcome, reconnected: true },
    );
    const { sockets, peers, events, connection } = setup(core);
    // Made before the connection is admitted: kept for later.
    connection.suggest('Add a test\r\nfor it');
    expect(events).toEqual([{ type: 'request-sent', id: 1 }]);
    const { peer } = await reachHost(sockets, peers);
    const control = peer.channel('control');
    control.open();
    expect(control.sent).toEqual([new Uint8Array([1])]);
    control.receive([0]);
    expect(control.sent.at(-1)).toEqual(new Uint8Array([3, 1]));
    control.receive([0]);
    expect(events.at(-1)).toEqual({ type: 'allowed', suggestions: true });
    control.receive([0]);
    expect(events.at(-1)).toEqual({ type: 'shared', wholeFiles: true });
    // Admitted: sent at once.
    connection.suggest('Fix it');
    expect(control.sent.at(-1)).toEqual(new Uint8Array([3, 2]));
    control.receive([0]);
    expect(events.at(-1)).toEqual({ type: 'reply', id: 2, outcome: 'Received' });
    expect(() => connection.suggest('  ')).toThrow();

    peer.fail();
    await vi.advanceTimersByTimeAsync(250);
    const second = await reachHost(sockets, peers);
    second.peer.channel('control').open();
    second.peer.channel('control').receive([0]);
    // The fake core never removed them, so both go again; the real one drops answered ones.
    expect(second.peer.channel('control').sent.slice(1)).toEqual([new Uint8Array([3, 1]), new Uint8Array([3, 2])]);
  });

  it('opens a files channel, fetches one file at a time and shares files after the host is ready', async () => {
    const core = fakeCore();
    const file = { file: 7, name: 'a.md', kind: 'markdown', mediaType: 'text/markdown', size: 1, sha256: [], by: 0, byLabel: 'nick', atMs: 1 } as const;
    core.controls.push(
      welcome,
      { type: 'files-allowed', files: true },
      { type: 'files', files: [{ ...file, sha256: [] }] },
      { type: 'reply', id: 54, outcome: { Ready: { upload: 3 } } },
    );
    const { sockets, peers, events, connection } = setup(core);
    const { peer } = await reachHost(sockets, peers);
    expect(peer.channels.map((channel) => channel.label)).toEqual(['control', 'stream', 'files']);
    const control = peer.channel('control');
    const files = peer.channel('files');
    control.open();
    files.open();
    control.receive([0]);
    control.receive([0]);
    expect(events.at(-1)).toEqual({ type: 'files-allowed', files: true });
    control.receive([0]);
    expect(events.at(-1)).toEqual({ type: 'shared-files', files: [file] });

    const opened = connection.openFile(7);
    const missing = connection.openFile(8);
    expect(files.sent).toEqual([new Uint8Array([5, 7])]);
    files.receive([1]);
    expect(events.at(-1)).toEqual({ type: 'file-progress', file: 7, received: 1, size: 1 });
    files.receive([2]);
    expect(new Uint8Array(await opened)).toEqual(new Uint8Array([42]));
    await expect(missing).rejects.toThrow('no longer shared');

    const shared = connection.shareFile('b.md', new Uint8Array([1]));
    expect(control.sent.at(-1)).toEqual(new Uint8Array([3, 54]));
    control.receive([0]);
    expect(files.sent.at(-1)).toEqual(new Uint8Array([6, 3]));
    files.receive([4]);
    await expect(shared).resolves.toBe('Received');
    // Files report back on their own: the suggestions' latest request stays as it was.
    expect(events.some((event) => event.type === 'request-sent')).toBe(false);
  });

  it('says the code is invalid when its room does not exist', async () => {
    const { sockets, events } = setup();
    sockets[0]!.open();
    sockets[0]!.receive({ type: 'error', code: 'room-not-found' });
    await vi.advanceTimersByTimeAsync(0);
    expect(events).toEqual([{ type: 'ended', reason: 'invalid-code' }]);
    expect(sockets[0]!.closed).toBe(true);
  });

  it('starts the peer only once the relay is known', async () => {
    const { sockets, peers } = setup();
    const socket = sockets[0]!;
    socket.open();
    socket.receive({ type: 'joined', connection: 'c1' });
    await vi.advanceTimersByTimeAsync(0);
    expect(peers).toEqual([]);
    socket.receive({ type: 'error', code: 'turn-unavailable' });
    await vi.advanceTimersByTimeAsync(0);
    expect(peers).toHaveLength(1);
    expect(peers[0]!.config.iceServers).toEqual([]);
  });

  it('says the sharing service is busy', async () => {
    const { sockets, events } = setup();
    sockets[0]!.open();
    sockets[0]!.receive({ type: 'error', code: 'server-busy' });
    await vi.advanceTimersByTimeAsync(0);
    expect(events).toEqual([{ type: 'ended', reason: 'service-busy' }]);
    expect(sockets[0]!.closed).toBe(true);
  });

  it('says the network blocks the connection when none is made within 20 seconds', async () => {
    const { sockets, peers, events } = setup();
    await reachHost(sockets, peers);
    await vi.advanceTimersByTimeAsync(19_000);
    expect(events).toEqual([]);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(events).toEqual([{ type: 'ended', reason: 'network-blocked' }]);
  });

  it('says the service cannot be reached when the first socket never opens', async () => {
    const { sockets, events } = setup();
    sockets[0]!.onclose?.();
    await vi.advanceTimersByTimeAsync(0);
    expect(events).toEqual([{ type: 'ended', reason: 'service-unavailable' }]);
  });

  it('after admission, reconnects through the rejoin room when the connection drops', async () => {
    const core = fakeCore();
    core.controls.push(welcome, { ...welcome, reconnected: true });
    const { sockets, peers, events, records } = setup(core);
    const { peer } = await reachHost(sockets, peers);
    peer.channel('control').open();
    expect(peer.channel('control').sent).toEqual([new Uint8Array([1])]);
    peer.channel('control').receive([0]);
    peer.channel('stream').receive([0]);
    expect(events).toEqual([{ type: 'joined', viewerId: 2, agent: 'claude-code', timelineSupported: true, cols: 80, rows: 24 }]);
    expect(records).toEqual([new Uint8Array([7])]);

    peer.fail();
    await vi.advanceTimersByTimeAsync(0);
    expect(events.at(-1)).toEqual({ type: 'reconnecting' });
    expect(peer.closed).toBe(true);
    await vi.advanceTimersByTimeAsync(250);
    const second = await reachHost(sockets, peers);
    expect(second.socket.sent[0]).toEqual({ type: 'join', room: 'rejoin' });
    second.peer.channel('control').open();
    second.peer.channel('control').receive([0]);
    expect(events.at(-1)).toEqual({ type: 'reconnected' });
  });

  it('takes bytes from the core as plain arrays too, as WebAssembly gives them', async () => {
    const core = fakeCore();
    core.controls.push(welcome);
    core.onStream = () => ({ records: [1, 0, 0, 0, 1, 104], resend: [2, 5] });
    const { sockets, peers, records } = setup(core);
    const { peer } = await reachHost(sockets, peers);
    peer.channel('control').open();
    peer.channel('control').receive([0]);
    peer.channel('stream').receive([0]);
    expect(records).toEqual([new Uint8Array([1, 0, 0, 0, 1, 104])]);
    expect(records[0]!.buffer).toBeInstanceOf(ArrayBuffer);
    expect(peer.channel('control').sent.at(-1)).toEqual(new Uint8Array([2, 5]));
  });

  it('does not apply an answer that fails the check, and tries again', async () => {
    const core = fakeCore();
    core.answerValid = false;
    const { sockets, peers, events } = setup(core);
    const { peer } = await reachHost(sockets, peers);
    expect(core.verifyAnswer).toHaveBeenCalledWith('c1', { kind: 'answer', sdp: 'answer-sdp', mac: 'm' });
    expect(peer.remote).toBeNull();
    expect(peer.closed).toBe(true);
    expect(events).toEqual([{ type: 'reconnecting' }]);
  });

  it('holds the host’s candidates until its answer is applied', async () => {
    const { sockets, peers } = setup();
    const socket = sockets[0]!;
    socket.open();
    socket.receive({ type: 'joined', connection: 'c1' });
    socket.receive({ type: 'error', code: 'turn-unavailable' });
    await vi.advanceTimersByTimeAsync(0);
    socket.receive({ type: 'signal', connection: 'c1', payload: { type: 'candidate', candidate: 'candidate:1', mid: '0' } });
    await vi.advanceTimersByTimeAsync(0);
    expect(peers[0]!.candidates).toEqual([]);
    socket.receive({ type: 'signal', connection: 'c1', payload: { type: 'description', kind: 'answer', sdp: 'a', mac: 'm' } });
    await vi.advanceTimersByTimeAsync(0);
    expect(peers[0]!.candidates).toEqual([{ candidate: 'candidate:1', sdpMid: '0' }]);
  });

  it('tells the host when the viewer leaves, and stops', async () => {
    const core = fakeCore();
    core.controls.push(welcome);
    const { sockets, peers, connection, events } = setup(core);
    const { peer } = await reachHost(sockets, peers);
    peer.channel('control').open();
    peer.channel('control').receive([0]);
    connection.leave();
    expect(peer.channel('control').sent.at(-1)).toEqual(new Uint8Array([9]));
    expect(peer.closed).toBe(true);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(sockets).toHaveLength(1);
    expect(events.map((event) => event.type)).toEqual(['joined']);
  });

  it('ends with the host’s refusal, saying the service needs updating when the host is newer', async () => {
    const core = fakeCore();
    core.controls.push({ type: 'refused', reason: 'ViewerOutdated' });
    const { sockets, peers, events } = setup(core);
    const { peer } = await reachHost(sockets, peers);
    peer.channel('control').open();
    peer.channel('control').receive([0]);
    await vi.advanceTimersByTimeAsync(0);
    expect(events).toEqual([{ type: 'ended', reason: 'service-outdated' }]);
  });
});

describe('a browser without WebRTC', () => {
  it('is told at once, without connecting anywhere', async () => {
    const sockets: unknown[] = [];
    const events: ViewEvent[] = [];
    new Connection({
      url: 'wss://specdeck.example.com/signal',
      username: 'anna',
      core: fakeCore(),
      platform: {
        socket: () => (sockets.push(1), new FakeSocket() as unknown as WebSocket),
        peer: () => {
          throw new Error('no WebRTC');
        },
        webrtc: () => false,
      },
      onEvent: (event) => events.push(event),
      onRecords: () => {},
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(events).toEqual([{ type: 'ended', reason: 'webrtc-off' }]);
    expect(sockets).toEqual([]);
  });
});

describe('refusals', () => {
  it('map to the reasons the window explains', () => {
    expect(refusalReason('ViewerOutdated')).toBe('service-outdated');
    expect(refusalReason('HostOutdated')).toBe('host-update');
    expect(refusalReason('SessionFull')).toBe('full');
    expect(refusalReason('Removed')).toBe('removed');
    expect(refusalReason('SessionEnded')).toBe('closed');
  });
});
