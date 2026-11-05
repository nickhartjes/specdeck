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
  socket.receive({ type: 'turn-credentials', urls: ['turn:turn.example.com:3478?transport=udp'], username: 'u', credential: 'c', ttlSeconds: 60 });
  socket.receive({ type: 'joined', connection: 'c1' });
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
    expect(socket.sent[0]).toEqual({ type: 'turn-credentials' });
    expect(socket.sent[1]).toEqual({ type: 'join', room: 'code-room' });
    expect(socket.sent[2]).toEqual({
      type: 'signal',
      connection: 'c1',
      payload: { type: 'description', kind: 'offer', sdp: 'offer-sdp', mac: 'mac-c1' },
    });
    expect(peer.config.iceServers).toEqual([{ urls: 'turn:turn.example.com:3478?transport=udp', username: 'u', credential: 'c' }]);
    expect(peer.channels.map((channel) => channel.label)).toEqual(['control', 'stream']);
    expect(peer.remote).toEqual({ type: 'answer', sdp: 'answer-sdp' });
  });

  it('says the code is invalid when its room does not exist', async () => {
    const { sockets, events } = setup();
    sockets[0]!.open();
    sockets[0]!.receive({ type: 'error', code: 'turn-unavailable' });
    sockets[0]!.receive({ type: 'error', code: 'room-not-found' });
    await vi.advanceTimersByTimeAsync(0);
    expect(events).toEqual([{ type: 'ended', reason: 'invalid-code' }]);
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
    expect(second.socket.sent[1]).toEqual({ type: 'join', room: 'rejoin' });
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
    socket.receive({ type: 'error', code: 'turn-unavailable' });
    socket.receive({ type: 'joined', connection: 'c1' });
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
