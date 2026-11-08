// A browser viewer's connection to a session (`web-viewer` design decision 3): the WebSocket to
// the sharing service, the WebRTC connection to the host, timeouts and reconnecting. It is a port
// of `Viewer::run` and `attempt` in crates/core/src/viewer.rs, on the browser's own WebSocket and
// RTCPeerConnection. What each message means is decided by `ViewerCore`, the shared Rust code
// compiled to WebAssembly (crates/web), so verification and ordering match the app.

import type { AgentId, AgentStatus, EndReason, Participant, RequestOutcome, SharedFileInfo, Suggestion, TimelineEvent, ViewEvent } from '../api/types';

/** How long joining may take, and one reconnection attempt; as `CONNECT_TIMEOUT` in the app. */
export const CONNECT_TIMEOUT_MS = 20_000;
const FIRST_RETRY_MS = 250;
const MAX_RETRY_MS = 5_000;

/** Why the host refused a viewer; `RefusalReason` in crates/protocol. */
export type Refusal = 'ViewerOutdated' | 'HostOutdated' | 'SessionFull' | 'Removed' | 'SessionEnded';

export type ControlUpdate =
  | { type: 'welcome'; viewerId: number; agent: AgentId; timelineSupported: boolean; cols: number; rows: number; reconnected: boolean }
  | { type: 'participants'; participants: Participant[] }
  | { type: 'status'; status: AgentStatus }
  | { type: 'refused'; reason: Refusal }
  | { type: 'ended' }
  | { type: 'allowed'; suggestions: boolean }
  | { type: 'reply'; id: number; outcome: RequestOutcome }
  | { type: 'suggestions'; mine: Suggestion[] }
  | { type: 'shared'; wholeFiles: boolean }
  | { type: 'files-allowed'; files: boolean }
  | { type: 'files'; files: SharedFileInfo[] }
  | { type: 'retry' };

/** What a message on the files channel did to a fetch; `FileFetch` in crates/web. */
export interface FetchUpdate {
  progress?: number;
  done?: Uint8Array | number[];
  /** A request to send on the files channel, fetching it again. */
  retry?: Uint8Array | number[];
  failed?: boolean;
  gone?: boolean;
}

/** Fetching one shared file; `FileFetch` in crates/web. */
export interface FileFetchCore {
  request(): Uint8Array;
  onReply(bytes: Uint8Array): FetchUpdate;
}

/** Sending one file the host accepted; `FileUpload` in crates/web. */
export interface FileUploadCore {
  nextChunks(): (Uint8Array | number[])[];
  onReply(bytes: Uint8Array): { acked?: boolean; finished?: RequestOutcome };
}

/** A numbered request and the control message that carries it. */
export interface Request {
  id: number;
  message: Uint8Array | number[];
}

export interface StreamUpdate {
  /** Terminal records in the format of src/terminal/stream.ts. */
  records?: Uint8Array | number[];
  timeline?: TimelineEvent;
  /** A control message to send, asking for records again. */
  resend?: Uint8Array | number[];
}

/** `WebViewer` from crates/web, as this module uses it. */
export interface ViewerCore {
  startAttempt(): string;
  signOffer(connection: string, sdp: string): unknown;
  verifyAnswer(connection: string, description: unknown): string | undefined;
  hello(username: string): Uint8Array;
  leave(): Uint8Array;
  onControl(bytes: Uint8Array): ControlUpdate;
  onStream(bytes: Uint8Array): StreamUpdate;
  admittedOnce(): boolean;
  /** Throws for text the host would refuse. */
  suggest(text: string): Request;
  /** Requests the host has not answered, to send once admitted. */
  unansweredRequests(): Request[];
  /** A file to share, announced with its size and hash. */
  shareFile(name: string, bytes: Uint8Array): Request;
  removeFile(file: number): Request;
  fetchFile(file: number, size: number, sha256: Uint8Array): FileFetchCore;
  uploadFile(upload: number, bytes: Uint8Array): FileUploadCore;
}

interface Fetch {
  file: number;
  resolve: (bytes: ArrayBuffer) => void;
  reject: (error: Error) => void;
}

interface Share {
  name: string;
  bytes: Uint8Array;
  resolve: (outcome: RequestOutcome) => void;
}

/** After the host says to wait, the next file is offered again this much later; as the app's viewer. */
const RATE_LIMIT_WAIT_MS = 10_000;

/** The browser APIs used, so tests can pass fakes. */
export interface Platform {
  socket(url: string): WebSocket;
  peer(config: RTCConfiguration): RTCPeerConnection;
  /** Whether the browser offers WebRTC at all; when absent, it does. */
  webrtc?(): boolean;
}

export const browserPlatform: Platform = {
  socket: (url) => new WebSocket(url),
  peer: (config) => new RTCPeerConnection(config),
  // Browsers with WebRTC turned off in their settings (such as LibreWolf's
  // media.peerconnection.enabled) leave it out entirely.
  webrtc: () => typeof globalThis.RTCPeerConnection === 'function',
};

/**
 * What the app tells a viewer for a refusal. A web viewer is as new as its sharing service, so
 * when the host's app is newer it is the service that needs updating, not an app.
 */
export function refusalReason(reason: Refusal): EndReason {
  switch (reason) {
    case 'ViewerOutdated':
      return 'service-outdated';
    case 'HostOutdated':
      return 'host-update';
    case 'SessionFull':
      return 'full';
    case 'Removed':
      return 'removed';
    case 'SessionEnded':
      return 'closed';
  }
}

type ServerMessage =
  | { type: 'turn-credentials'; urls: string[]; username: string; credential: string }
  | { type: 'error'; code: string }
  | { type: 'joined'; connection: string }
  | { type: 'signal'; connection: string; payload: SignalPayload }
  | { type: 'peer-left'; connection: string }
  | { type: 'room-opened' | 'viewer-joined' };

type SignalPayload =
  | { type: 'description'; kind: 'offer' | 'answer'; sdp: string; mac: string }
  | { type: 'candidate'; candidate: string; mid: string };

/** How one attempt ended. */
type Outcome = { final: EndReason } | { retry: true } | { left: true };

export interface ConnectionOptions {
  /** The sharing service, `wss://…/signal`. */
  url: string;
  username: string;
  core: ViewerCore;
  /** Terminal records, in order. */
  onRecords: (bytes: Uint8Array) => void;
  onEvent: (event: ViewEvent) => void;
  platform?: Platform;
  timeoutMs?: number;
}

/** Joins the session and keeps the viewer connected until it ends or `leave` is called. */
export class Connection {
  private readonly platform: Platform;
  private readonly timeoutMs: number;
  private readonly joinedAt = Date.now();
  private stop: (() => void) | null = null;
  private wake: (() => void) | null = null;
  private left = false;
  /** Sends on the control channel while this connection is admitted; null otherwise. */
  private sendControl: ((bytes: Uint8Array<ArrayBuffer>) => void) | null = null;
  /** Sends on the files channel while it is open and this connection is admitted; null otherwise. */
  private sendFiles: ((bytes: Uint8Array<ArrayBuffer>) => void) | null = null;
  // Fetching and sharing files, one of each at a time (`shared-files` design decision 2).
  private files: SharedFileInfo[] = [];
  private fetches: Fetch[] = [];
  private fetching: (Fetch & { core: FileFetchCore }) | null = null;
  private shares: Share[] = [];
  private asked: { id: number; share: Share } | null = null;
  private uploading: { core: FileUploadCore; share: Share } | null = null;
  private waiting = false;

  constructor(private readonly options: ConnectionOptions) {
    this.platform = options.platform ?? browserPlatform;
    this.timeoutMs = options.timeoutMs ?? CONNECT_TIMEOUT_MS;
    void this.run();
  }

  /**
   * Suggests a prompt to the host: sent now when admitted, or once admitted again. Throws for
   * text the host would refuse.
   */
  suggest(text: string) {
    const request = this.options.core.suggest(text.replace(/\r\n/g, '\n'));
    this.options.onEvent({ type: 'request-sent', id: request.id });
    this.sendControl?.(new Uint8Array(request.message));
  }

  /** A shared file's bytes, fetched from the host and checked against what it announced. */
  openFile(file: number): Promise<ArrayBuffer> {
    return new Promise((resolve, reject) => {
      this.fetches.push({ file, resolve, reject });
      this.drive();
    });
  }

  /** Shares a file with everyone; resolves to `Received` once shared, or to why not. */
  shareFile(name: string, bytes: Uint8Array): Promise<RequestOutcome> {
    return new Promise((resolve) => {
      this.shares.push({ name, bytes, resolve });
      this.drive();
    });
  }

  /** Removes a file this viewer shared. */
  removeFile(file: number) {
    // Not announced as `request-sent`: the window follows its latest suggestion by that.
    const request = this.options.core.removeFile(file);
    this.sendControl?.(new Uint8Array(request.message));
  }

  /** Starts the next fetch and share when none runs, and sends what an upload may send now. */
  private drive() {
    const send = this.sendFiles;
    const sendControl = this.sendControl;
    if (!send || !sendControl) return;
    while (!this.fetching && this.fetches.length > 0) {
      const next = this.fetches.shift()!;
      const info = this.files.find((file) => file.file === next.file);
      if (!info) {
        next.reject(new Error('That file is no longer shared.'));
        continue;
      }
      const core = this.options.core.fetchFile(info.file, info.size, new Uint8Array(info.sha256));
      this.fetching = { ...next, core };
      send(new Uint8Array(core.request()));
    }
    if (!this.asked && !this.uploading && !this.waiting && this.shares.length > 0) {
      const share = this.shares.shift()!;
      const request = this.options.core.shareFile(share.name, share.bytes);
      this.asked = { id: request.id, share };
      sendControl(new Uint8Array(request.message));
    }
    if (this.uploading) for (const chunk of this.uploading.core.nextChunks()) send(new Uint8Array(chunk));
  }

  /** The host's answer to a file this viewer asked to share. */
  private shareReplied(id: number, outcome: RequestOutcome) {
    if (this.asked?.id !== id) return;
    const { share } = this.asked;
    this.asked = null;
    if (typeof outcome === 'object' && 'Ready' in outcome) {
      this.uploading = { core: this.options.core.uploadFile(outcome.Ready.upload, share.bytes), share };
    } else if (outcome === 'RateLimited') {
      // One upload at a time, and the rate limit: ask again in a moment.
      this.shares.unshift(share);
      this.waiting = true;
      setTimeout(() => {
        this.waiting = false;
        this.drive();
      }, RATE_LIMIT_WAIT_MS);
    } else {
      share.resolve(outcome);
    }
    this.drive();
  }

  /** A message on the files channel: a chunk of the file being fetched, or about the upload. */
  private onFiles(bytes: Uint8Array) {
    const fetching = this.fetching;
    if (fetching) {
      const update = fetching.core.onReply(bytes);
      if (update.progress !== undefined) {
        const size = this.files.find((file) => file.file === fetching.file)?.size ?? update.progress;
        this.options.onEvent({ type: 'file-progress', file: fetching.file, received: update.progress, size });
        return;
      }
      if (update.retry) {
        this.sendFiles?.(new Uint8Array(update.retry));
        return;
      }
      if (update.done || update.failed) {
        this.fetching = null;
        if (update.done) fetching.resolve(new Uint8Array(update.done).buffer);
        else fetching.reject(new Error(update.gone ? 'That file is no longer shared.' : 'The file did not arrive intact.'));
        this.drive();
        return;
      }
    }
    const uploading = this.uploading;
    if (!uploading) return;
    const update = uploading.core.onReply(bytes);
    if (update.finished !== undefined) {
      this.uploading = null;
      uploading.share.resolve(update.finished);
    }
    this.drive();
  }

  /** The connection dropped: what the host had of an upload is gone, so it is shared again. */
  private dropped() {
    this.sendControl = null;
    this.sendFiles = null;
    if (this.uploading) this.shares.unshift(this.uploading.share);
    if (this.asked) this.shares.unshift(this.asked.share);
    this.uploading = null;
    this.asked = null;
  }

  /** Leaves the session: tells the host if connected, and closes everything. */
  leave() {
    this.left = true;
    this.stop?.();
    this.wake?.();
  }

  private async run() {
    // Without WebRTC nothing can connect: say so now, not after the join timeout.
    if (this.platform.webrtc && !this.platform.webrtc()) {
      await Promise.resolve();
      this.options.onEvent({ type: 'ended', reason: 'webrtc-off' });
      return;
    }
    let retry = FIRST_RETRY_MS;
    for (;;) {
      const admittedBefore = this.options.core.admittedOnce();
      const outcome = await this.attempt();
      if ('left' in outcome || this.left) return;
      if ('final' in outcome) {
        this.options.onEvent({ type: 'ended', reason: outcome.final });
        return;
      }
      const admitted = this.options.core.admittedOnce();
      // Joining keeps failing, for example because no relay address can be had.
      if (!admitted && Date.now() - this.joinedAt >= this.timeoutMs) {
        this.options.onEvent({ type: 'ended', reason: 'network-blocked' });
        return;
      }
      // Back off only while attempts keep failing.
      if (admitted && !admittedBefore) retry = FIRST_RETRY_MS;
      const wait = retry;
      retry = Math.min(retry * 2, MAX_RETRY_MS);
      this.options.onEvent({ type: 'reconnecting' });
      await new Promise<void>((resume) => {
        const timer = setTimeout(resume, wait);
        this.wake = () => {
          clearTimeout(timer);
          resume();
        };
      });
      this.wake = null;
      if (this.left) return;
    }
  }

  /** One connection: signalling, peer connection, then the session until it ends or drops. */
  private attempt(): Promise<Outcome> {
    const { core, username, onRecords, onEvent } = this.options;
    const admitted = core.admittedOnce();
    // A new viewer joins with the code; an admitted one returns through the rejoin room.
    const room = core.startAttempt();
    // Joining has one deadline across attempts; each reconnection attempt has its own.
    const deadline = (admitted ? Date.now() : this.joinedAt) + this.timeoutMs;

    return new Promise<Outcome>((resolve) => {
      let done = false;
      let opened = false;
      let connected = false;
      // The connection id the service gave this viewer on joining; the peer starts once TURN
      // credentials, or the answer that there are none, have arrived.
      let joinedAs: string | undefined;
      let connection: string | null = null;
      let peer: RTCPeerConnection | null = null;
      let control: RTCDataChannel | null = null;
      let remoteSet = false;
      const iceServers: RTCIceServer[] = [];
      const pendingCandidates: RTCIceCandidateInit[] = [];

      const socket = this.platform.socket(this.options.url);
      const finish = (outcome: Outcome) => {
        if (done) return;
        done = true;
        clearTimeout(timer);
        this.stop = null;
        this.dropped();
        socket.onopen = socket.onmessage = socket.onclose = socket.onerror = null;
        try {
          socket.close();
        } catch {
          // Already closed.
        }
        if (peer) {
          peer.onicecandidate = peer.onconnectionstatechange = null;
          peer.close();
        }
        resolve(outcome);
      };
      const timer = setTimeout(
        () => finish(core.admittedOnce() ? { retry: true } : { final: 'network-blocked' }),
        Math.max(0, deadline - Date.now()),
      );
      this.stop = () => {
        if (control?.readyState === 'open') control.send(new Uint8Array(core.leave()));
        finish({ left: true });
      };

      const send = (message: object) => socket.send(JSON.stringify(message));
      const startPeerOnceJoined = () => {
        if (joinedAs && !peer) startPeer(joinedAs);
      };

      const startPeer = (id: string) => {
        connection = id;
        const pc = this.platform.peer({ iceServers });
        peer = pc;
        pc.onicecandidate = ({ candidate }) => {
          if (candidate?.candidate) {
            send({ type: 'signal', connection: id, payload: { type: 'candidate', candidate: candidate.candidate, mid: candidate.sdpMid ?? '' } });
          }
        };
        pc.onconnectionstatechange = () => {
          if (['failed', 'disconnected', 'closed'].includes(pc.connectionState)) finish({ retry: true });
        };
        control = pc.createDataChannel('control');
        const stream = pc.createDataChannel('stream');
        const files = pc.createDataChannel('files');
        control.binaryType = stream.binaryType = files.binaryType = 'arraybuffer';
        let admittedHere = false;
        const filesReady = () => {
          if (!admittedHere || files.readyState !== 'open') return;
          this.sendFiles = (bytes) => {
            if (files.readyState === 'open') files.send(bytes);
          };
          // A fetch the dropped connection cut off goes on where it was.
          if (this.fetching) this.sendFiles(new Uint8Array(this.fetching.core.request()));
          this.drive();
        };
        files.onopen = filesReady;
        files.onmessage = ({ data }) => this.onFiles(new Uint8Array(data as ArrayBuffer));
        control.onopen = () => {
          connected = true;
          clearTimeout(timer);
          control!.send(new Uint8Array(core.hello(username)));
        };
        control.onmessage = ({ data }) => {
          const update = core.onControl(new Uint8Array(data as ArrayBuffer));
          switch (update.type) {
            case 'welcome': {
              const { viewerId, agent, timelineSupported, cols, rows, reconnected } = update;
              onEvent(reconnected ? { type: 'reconnected' } : { type: 'joined', viewerId, agent, timelineSupported, cols, rows });
              const channel = control!;
              this.sendControl = (bytes) => {
                if (channel.readyState === 'open') channel.send(bytes);
              };
              // Requests made while away, and ones a dropped connection left unanswered.
              for (const request of core.unansweredRequests()) this.sendControl(new Uint8Array(request.message));
              admittedHere = true;
              filesReady();
              break;
            }
            case 'files-allowed':
              onEvent({ type: 'files-allowed', files: update.files });
              break;
            case 'files':
              this.files = update.files;
              onEvent({ type: 'shared-files', files: update.files });
              break;
            case 'allowed':
              onEvent({ type: 'allowed', suggestions: update.suggestions });
              break;
            case 'reply':
              onEvent({ type: 'reply', id: update.id, outcome: update.outcome });
              this.shareReplied(update.id, update.outcome);
              break;
            case 'suggestions':
              onEvent({ type: 'suggestions', mine: update.mine });
              break;
            case 'shared':
              onEvent({ type: 'shared', wholeFiles: update.wholeFiles });
              break;
            case 'participants':
              onEvent({ type: 'participants', participants: update.participants });
              break;
            case 'status':
              onEvent({ type: 'status', status: update.status });
              break;
            case 'refused':
              finish({ final: refusalReason(update.reason) });
              break;
            case 'ended':
              finish({ final: 'closed' });
              break;
            case 'retry':
              finish({ retry: true });
              break;
          }
        };
        stream.onmessage = ({ data }) => {
          const update = core.onStream(new Uint8Array(data as ArrayBuffer));
          // Bytes from WebAssembly can arrive as plain arrays of numbers; copy them into ones of
          // our own either way.
          if (update.records) onRecords(new Uint8Array(update.records));
          if (update.timeline) onEvent({ type: 'timeline', event: update.timeline });
          if (update.resend && control?.readyState === 'open') control.send(new Uint8Array(update.resend));
        };
        void (async () => {
          try {
            await pc.setLocalDescription(await pc.createOffer());
            const sdp = pc.localDescription?.sdp;
            if (!sdp || done) return;
            send({ type: 'signal', connection: id, payload: { type: 'description', ...(core.signOffer(id, sdp) as object) } });
          } catch {
            finish({ retry: true });
          }
        })();
      };

      const onSignal = async (payload: SignalPayload) => {
        if (!peer || !connection) return;
        if (payload.type === 'description') {
          // The answer must be made with this session's code, for this connection.
          const { kind, sdp, mac } = payload;
          const verified = core.verifyAnswer(connection, { kind, sdp, mac });
          if (verified === undefined) return finish({ retry: true });
          try {
            await peer.setRemoteDescription({ type: 'answer', sdp: verified });
            remoteSet = true;
            for (const candidate of pendingCandidates.splice(0)) await peer.addIceCandidate(candidate);
          } catch {
            finish({ retry: true });
          }
        } else {
          const candidate = { candidate: payload.candidate, sdpMid: payload.mid };
          if (remoteSet) await peer.addIceCandidate(candidate).catch(() => {});
          else pendingCandidates.push(candidate);
        }
      };

      socket.onopen = () => {
        opened = true;
        // TURN credentials come after joining: the service gives them only to members.
        send({ type: 'join', room });
      };
      socket.onerror = () => {};
      socket.onclose = () => {
        // Before the first admission the service being unreachable is final; later, keep trying.
        if (!opened) finish(core.admittedOnce() ? { retry: true } : { final: 'service-unavailable' });
        // Once connected, the signalling service is no longer needed.
        else if (!connected) finish({ retry: true });
      };
      socket.onmessage = ({ data }) => {
        let message: ServerMessage;
        try {
          message = JSON.parse(String(data)) as ServerMessage;
        } catch {
          return;
        }
        switch (message.type) {
          case 'turn-credentials':
            for (const url of message.urls) iceServers.push({ urls: url, username: message.username, credential: message.credential });
            startPeerOnceJoined();
            break;
          case 'error':
            if (message.code === 'turn-unavailable') startPeerOnceJoined();
            // After the host closed the session its room is gone.
            else if (message.code === 'room-not-found') finish({ final: core.admittedOnce() ? 'closed' : 'invalid-code' });
            else if (message.code === 'rate-limited') finish({ final: 'too-many-attempts' });
            // Before the first admission a full service is final; later, keep trying.
            else if (message.code === 'server-busy') finish(core.admittedOnce() ? { retry: true } : { final: 'service-busy' });
            break;
          case 'joined':
            joinedAs = message.connection;
            send({ type: 'turn-credentials' });
            break;
          case 'signal':
            if (message.connection === connection) void onSignal(message.payload);
            break;
          case 'peer-left':
            if (!connected) finish({ retry: true });
            break;
        }
      };
    });
  }
}
