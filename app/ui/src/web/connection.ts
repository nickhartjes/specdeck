// A browser viewer's connection to a session (`web-viewer` design decision 3): the WebSocket to
// the sharing service, the WebRTC connection to the host, timeouts and reconnecting. It is a port
// of `Viewer::run` and `attempt` in crates/core/src/viewer.rs, on the browser's own WebSocket and
// RTCPeerConnection. What each message means is decided by `ViewerCore`, the shared Rust code
// compiled to WebAssembly (crates/web), so verification and ordering match the app.

import type { AgentId, AgentStatus, BraindumpItemInfo, Card, EndReason, LinkState, Participant, ReadAnswer, ReadView, RequestOutcome, SentLink, SharedFileInfo, Suggestion, TimelineEvent, ViewEvent, VoteView } from '../api/types';
import { DiagnosticLog, outcomeName, type AttemptEnd, type CandidateType, type Channel } from './diagnostics';

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
  | { type: 'links-allowed'; links: boolean }
  | { type: 'links'; mine: SentLink[] }
  | { type: 'read-request'; open: ReadView | null }
  | { type: 'vote'; open: VoteView | null }
  | { type: 'braindump'; items: BraindumpItemInfo[] }
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
  /** The record that resend asks for from. */
  resendFrom?: number;
  /** The host has sent the history this viewer was admitted to. */
  caughtUp?: boolean;
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
  /** Throws for a link the host would refuse; sends it in the form the host requires. */
  sendLink(url: string, title: string | null, sessionOnly: boolean): Request;
  /** Throws for a question the host would refuse. */
  answerRead(ask: number, answer: ReadAnswer): Request;
  castVote(ask: number, round: number, card: Card | null): Request;
  /** Requests the host has not answered, to send once admitted. */
  unansweredRequests(): Request[];
  /** A file to share, announced with its size and hash. */
  shareFile(name: string, bytes: Uint8Array): Request;
  /** A file for the braindump, announced as `shareFile` announces one. */
  addToBraindump(name: string, bytes: Uint8Array): Request;
  /** A note for the braindump, or a new version of this viewer's note `file`; throws past 32 KB. */
  addNote(text: Uint8Array, file?: number): Request;
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
  /** Where it goes: shared with everyone, the braindump, a note, or a new version of the note `file`. */
  to: 'everyone' | 'braindump' | 'note';
  file?: number;
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
  | { type: 'error'; code: string; room?: string }
  | { type: 'joined'; connection: string }
  | { type: 'signal'; connection: string; payload: SignalPayload }
  | { type: 'peer-left'; connection: string }
  | { type: 'room-opened' | 'viewer-joined' };

type SignalPayload =
  | { type: 'description'; kind: 'offer' | 'answer'; sdp: string; mac: string }
  | { type: 'candidate'; candidate: string; mid: string };

/** How one attempt ended. */
type Outcome = { final: EndReason } | { retry: AttemptEnd } | { left: true };

/** Numbers this page's peer connections in the diagnostic log, as `next_peer_number` in the app. */
let nextPeer = 1;

const CANDIDATE_TYPES: readonly string[] = ['host', 'srflx', 'prflx', 'relay'];
const LINK_STATES: readonly string[] = ['new', 'connecting', 'connected', 'disconnected', 'failed', 'closed'];

/**
 * The candidate types of the pair the browser chose, from its statistics: the transport's
 * `selectedCandidatePairId`, or a pair marked `selected` (older Firefox). Only the types are
 * read, never the addresses.
 */
export function selectedPath(stats: RTCStatsReport): { local: CandidateType; remote: CandidateType } | null {
  const reports = new Map<string, Record<string, unknown>>();
  stats.forEach((report: Record<string, unknown>) => reports.set(String(report.id), report));
  let pair: Record<string, unknown> | undefined;
  for (const report of reports.values()) {
    if (report.type === 'transport' && typeof report.selectedCandidatePairId === 'string') pair = reports.get(report.selectedCandidatePairId);
  }
  pair ??= [...reports.values()].find((report) => report.type === 'candidate-pair' && report.selected === true);
  if (!pair) return null;
  const typeOf = (id: unknown): CandidateType => {
    const type = reports.get(String(id))?.candidateType;
    return typeof type === 'string' && CANDIDATE_TYPES.includes(type) ? (type as CandidateType) : 'unknown';
  };
  return { local: typeOf(pair.localCandidateId), remote: typeOf(pair.remoteCandidateId) };
}

export interface ConnectionOptions {
  /** The sharing service, `wss://…/signal`. */
  url: string;
  username: string;
  core: ViewerCore;
  /** Terminal records, in order. */
  onRecords: (bytes: Uint8Array) => void;
  onEvent: (event: ViewEvent) => void;
  /** The page's diagnostic log; one of its own when absent. */
  log?: DiagnosticLog;
  platform?: Platform;
  timeoutMs?: number;
}

/** Joins the session and keeps the viewer connected until it ends or `leave` is called. */
export class Connection {
  private readonly platform: Platform;
  private readonly timeoutMs: number;
  private readonly log: DiagnosticLog;
  private readonly joinedAt = Date.now();
  /** Connection attempts so far, the current one included. */
  private attempts = 0;
  /** When the current attempt's control channel opened. */
  private connectedAt: number | null = null;
  private stop: (() => void) | null = null;
  private wake: (() => void) | null = null;
  private left = false;
  /** Sends on the control channel while this connection is admitted; null otherwise. */
  private sendControl: ((bytes: Uint8Array<ArrayBuffer>) => void) | null = null;
  /** Sends on the files channel while it is open and this connection is admitted; null otherwise. */
  private sendFiles: ((bytes: Uint8Array<ArrayBuffer>) => void) | null = null;
  // Fetching and sharing files, one of each at a time (`shared-files` design decision 2).
  private files: SharedFileInfo[] = [];
  /** The braindump's items, fetched like shared files. */
  private braindump: SharedFileInfo[] = [];
  private fetches: Fetch[] = [];
  private fetching: (Fetch & { core: FileFetchCore }) | null = null;
  private shares: Share[] = [];
  private asked: { id: number; share: Share } | null = null;
  private uploading: { core: FileUploadCore; share: Share } | null = null;
  private waiting = false;

  constructor(private readonly options: ConnectionOptions) {
    this.platform = options.platform ?? browserPlatform;
    this.timeoutMs = options.timeoutMs ?? CONNECT_TIMEOUT_MS;
    this.log = options.log ?? new DiagnosticLog(null);
    void this.run();
  }

  /**
   * Suggests a prompt to the host: sent now when admitted, or once admitted again. Throws for
   * text the host would refuse.
   */
  suggest(text: string) {
    const plain = text.replace(/\r\n/g, '\n');
    const request = this.options.core.suggest(plain);
    this.log.requestSent('suggest', new TextEncoder().encode(plain).length);
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
    return this.upload({ to: 'everyone', name, bytes });
  }

  /** Adds a file to the braindump, as `shareFile` shares one. */
  addToBraindump(name: string, bytes: Uint8Array): Promise<RequestOutcome> {
    return this.upload({ to: 'braindump', name, bytes });
  }

  /** Adds a note to the braindump, or replaces this viewer's note `file` while it is not given. */
  addNote(text: string, file?: number): Promise<RequestOutcome> {
    const bytes = new TextEncoder().encode(text.replace(/\r\n/g, '\n'));
    // The host would disconnect a viewer that announced a note of another size.
    if (bytes.length === 0 || bytes.length > 32 * 1024) return Promise.resolve('TooLarge');
    return this.upload({ to: 'note', file, name: '', bytes });
  }

  private upload(share: Omit<Share, 'resolve'>): Promise<RequestOutcome> {
    return new Promise((resolve) => {
      this.shares.push({ ...share, resolve });
      this.drive();
    });
  }

  /** Removes a file this viewer shared. */
  removeFile(file: number) {
    // Not announced as `request-sent`: the window follows its latest suggestion by that.
    const request = this.options.core.removeFile(file);
    this.log.requestSent('remove_file', 0);
    this.sendControl?.(new Uint8Array(request.message));
  }

  /** Starts the next fetch and share when none runs, and sends what an upload may send now. */
  private drive() {
    const send = this.sendFiles;
    const sendControl = this.sendControl;
    if (!send || !sendControl) return;
    while (!this.fetching && this.fetches.length > 0) {
      const next = this.fetches.shift()!;
      const info = [...this.files, ...this.braindump].find((file) => file.file === next.file);
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
      const core = this.options.core;
      const request =
        share.to === 'braindump' ? core.addToBraindump(share.name, share.bytes)
        : share.to === 'note' ? core.addNote(share.bytes, share.file)
        : core.shareFile(share.name, share.bytes);
      const kind = share.to === 'braindump' ? 'add_to_braindump' : share.to === 'note' ? (share.file === undefined ? 'add_note' : 'replace_note') : 'share_file';
      this.log.requestSent(kind, share.bytes.length);
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
      this.log.fileUploaded(share.bytes.length, outcomeName(outcome));
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
        const size = [...this.files, ...this.braindump].find((file) => file.file === fetching.file)?.size ?? update.progress;
        this.options.onEvent({ type: 'file-progress', file: fetching.file, received: update.progress, size });
        return;
      }
      const size = () => [...this.files, ...this.braindump].find((file) => file.file === fetching.file)?.size ?? 0;
      if (update.retry) {
        this.log.fileFetched(size(), 'retried');
        this.sendFiles?.(new Uint8Array(update.retry));
        return;
      }
      if (update.done || update.failed) {
        this.log.fileFetched(size(), update.done ? 'fetched' : 'failed');
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
      this.log.fileUploaded(uploading.share.bytes.length, outcomeName(update.finished));
      uploading.share.resolve(update.finished);
    }
    this.drive();
  }

  /** The connection dropped: what the host had of an upload is gone, so it is shared again. */
  private dropped() {
    this.sendControl = null;
    this.sendFiles = null;
    if (this.uploading) {
      this.log.fileUploaded(this.uploading.share.bytes.length, 'retried');
      this.shares.unshift(this.uploading.share);
    }
    if (this.asked) this.shares.unshift(this.asked.share);
    this.uploading = null;
    this.asked = null;
  }

  /** Answers the host's read request; throws for a question the host would refuse. */
  answerRead(ask: number, answer: ReadAnswer) {
    const request = this.options.core.answerRead(ask, answer);
    this.log.requestSent('answer', 0);
    this.options.onEvent({ type: 'answer-sent', id: request.id });
    this.sendControl?.(new Uint8Array(request.message));
  }

  /** Votes in the host's vote; null takes the vote back. */
  castVote(ask: number, round: number, card: Card | null) {
    const request = this.options.core.castVote(ask, round, card);
    this.log.requestSent('answer', 0);
    this.options.onEvent({ type: 'answer-sent', id: request.id });
    this.sendControl?.(new Uint8Array(request.message));
  }

  /** Sends the host a link to share, as `suggest` does. Throws for a link the host would refuse. */
  sendLink(url: string, title: string | null, sessionOnly: boolean) {
    const request = this.options.core.sendLink(url, title, sessionOnly);
    this.log.requestSent('share_link', new TextEncoder().encode(url).length);
    this.options.onEvent({ type: 'request-sent', id: request.id });
    this.sendControl?.(new Uint8Array(request.message));
  }

  /** Leaves the session: tells the host if connected, and closes everything. */
  leave() {
    // Recorded now: the page may be going away, and would not get to it after the attempt ends.
    if (this.stop && !this.left) {
      const connectedMs = this.connectedAt === null ? 0 : Date.now() - this.connectedAt;
      this.log.attemptEnded(this.attempts, 'left', connectedMs);
    }
    this.left = true;
    this.stop?.();
    this.wake?.();
  }

  /** The session is over for this page: logged, then told to the window. */
  private end(reason: EndReason) {
    this.log.ended(reason);
    this.options.onEvent({ type: 'ended', reason });
  }

  private async run() {
    // Without WebRTC nothing can connect: say so now, not after the join timeout.
    if (this.platform.webrtc && !this.platform.webrtc()) {
      await Promise.resolve();
      this.end('webrtc-off');
      return;
    }
    let retry = FIRST_RETRY_MS;
    for (;;) {
      const admittedBefore = this.options.core.admittedOnce();
      this.attempts += 1;
      this.log.attemptStarted(this.attempts);
      const outcome = await this.attempt();
      const connectedMs = this.connectedAt === null ? 0 : Date.now() - this.connectedAt;
      this.connectedAt = null;
      // `leave` recorded its own end.
      if ('left' in outcome || this.left) return;
      this.log.attemptEnded(this.attempts, 'final' in outcome ? 'final' : outcome.retry, connectedMs);
      if ('final' in outcome) {
        this.end(outcome.final);
        return;
      }
      const admitted = this.options.core.admittedOnce();
      // Joining keeps failing, for example because no relay address can be had.
      if (!admitted && Date.now() - this.joinedAt >= this.timeoutMs) {
        this.end('network-blocked');
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
      const log = this.log;
      let done = false;
      let opened = false;
      let socketOpen = false;
      let connected = false;
      // From the welcome until the host says this connection has caught up.
      let catchUp: { since: number; reconnected: boolean; batches: number; bytes: number } | null = null;
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
        if (socketOpen) log.signallingClosed(true);
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
        () => finish(core.admittedOnce() ? { retry: 'timeout' } : { final: 'network-blocked' }),
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
        const number = nextPeer++;
        log.peerCreated(number, iceServers.length > 0);
        pc.onicecandidate = ({ candidate }) => {
          if (candidate?.candidate) {
            send({ type: 'signal', connection: id, payload: { type: 'candidate', candidate: candidate.candidate, mid: candidate.sdpMid ?? '' } });
          }
        };
        pc.onconnectionstatechange = () => {
          const state = pc.connectionState;
          if (LINK_STATES.includes(state)) log.peerState(number, state as LinkState);
          if (state === 'connected' && typeof pc.getStats === 'function') {
            void pc
              .getStats()
              .then((stats) => {
                const path = selectedPath(stats);
                log.peerPath(number, path?.local ?? 'unknown', path?.remote ?? 'unknown');
              })
              .catch(() => log.peerPath(number, 'unknown', 'unknown'));
          }
          if (['failed', 'disconnected', 'closed'].includes(state)) finish({ retry: 'peer_failed' });
        };
        control = pc.createDataChannel('control');
        const stream = pc.createDataChannel('stream');
        const files = pc.createDataChannel('files');
        control.binaryType = stream.binaryType = files.binaryType = 'arraybuffer';
        for (const [channel, name] of [[control, 'control'], [stream, 'stream'], [files, 'files']] as [RTCDataChannel, Channel][]) {
          channel.addEventListener?.('open', () => log.channelOpen(number, name));
          channel.addEventListener?.('close', () => log.channelClosed(number, name));
        }
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
          this.connectedAt = Date.now();
          clearTimeout(timer);
          control!.send(new Uint8Array(core.hello(username)));
        };
        control.onmessage = ({ data }) => {
          const update = core.onControl(new Uint8Array(data as ArrayBuffer));
          switch (update.type) {
            case 'welcome': {
              const { viewerId, agent, timelineSupported, cols, rows, reconnected } = update;
              catchUp = { since: Date.now(), reconnected, batches: 0, bytes: 0 };
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
              log.requestReply(update.outcome);
              onEvent({ type: 'reply', id: update.id, outcome: update.outcome });
              this.shareReplied(update.id, update.outcome);
              break;
            case 'suggestions':
              onEvent({ type: 'suggestions', mine: update.mine });
              break;
            case 'shared':
              onEvent({ type: 'shared', wholeFiles: update.wholeFiles });
              break;
            case 'links-allowed':
              onEvent({ type: 'links-allowed', links: update.links });
              break;
            case 'braindump':
              this.braindump = update.items.map((item) => item.info);
              onEvent({ type: 'braindump', items: update.items });
              break;
            case 'links':
              onEvent({ type: 'links', mine: update.mine });
              break;
            case 'read-request':
              onEvent({ type: 'read-request', open: update.open });
              break;
            case 'vote':
              onEvent({ type: 'vote', open: update.open });
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
              finish({ retry: 'host_asked_retry' });
              break;
          }
        };
        stream.onmessage = ({ data }) => {
          const bytes = new Uint8Array(data as ArrayBuffer);
          const update = core.onStream(bytes);
          if (update.caughtUp) {
            if (catchUp) log.caughtUp(catchUp.reconnected, catchUp.batches, catchUp.bytes, Date.now() - catchUp.since);
            catchUp = null;
          } else if (catchUp) {
            catchUp.batches += 1;
            catchUp.bytes += bytes.length;
          }
          if (update.resendFrom !== undefined) log.resend(update.resendFrom);
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
            finish({ retry: 'peer_create_failed' });
          }
        })();
      };

      const onSignal = async (payload: SignalPayload) => {
        if (!peer || !connection) return;
        if (payload.type === 'description') {
          // The answer must be made with this session's code, for this connection.
          const { kind, sdp, mac } = payload;
          const verified = core.verifyAnswer(connection, { kind, sdp, mac });
          if (verified === undefined) return finish({ retry: 'bad_answer' });
          try {
            await peer.setRemoteDescription({ type: 'answer', sdp: verified });
            remoteSet = true;
            for (const candidate of pendingCandidates.splice(0)) await peer.addIceCandidate(candidate);
          } catch {
            finish({ retry: 'bad_answer' });
          }
        } else {
          const candidate = { candidate: payload.candidate, sdpMid: payload.mid };
          if (remoteSet) await peer.addIceCandidate(candidate).catch(() => {});
          else pendingCandidates.push(candidate);
        }
      };

      socket.onopen = () => {
        opened = true;
        socketOpen = true;
        log.signallingConnected();
        // TURN credentials come after joining: the service gives them only to members.
        send({ type: 'join', room });
      };
      socket.onerror = () => {};
      socket.onclose = () => {
        if (opened) {
          socketOpen = false;
          log.signallingClosed(false);
          if (connected) log.viewerSignallingClosed();
        } else {
          log.signallingConnectFailed();
        }
        // Before the first admission the service being unreachable is final; later, keep trying.
        if (!opened) finish(core.admittedOnce() ? { retry: 'signalling_unavailable' } : { final: 'service-unavailable' });
        // Once connected, the signalling service is no longer needed.
        else if (!connected) finish({ retry: 'signalling_closed' });
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
            else if (message.code === 'server-busy') finish(core.admittedOnce() ? { retry: 'service_busy' } : { final: 'service-busy' });
            break;
          case 'joined':
            joinedAs = message.connection;
            send({ type: 'turn-credentials' });
            break;
          case 'signal':
            if (message.connection === connection) void onSignal(message.payload);
            break;
          case 'peer-left':
            if (!connected) finish({ retry: 'host_left' });
            break;
        }
      };
    });
  }
}
