// The WebAssembly package from crates/web (`just web-wasm`), as vite.web.config.ts aliases it.
// Declared here so the UI type-checks without building it first.
declare module 'specdeck-web-wasm' {
  export default function init(): Promise<unknown>;
  export class WebViewer {
    constructor(code: string);
    static protocolVersion(): number;
    static leave(): Uint8Array;
    admittedOnce(): boolean;
    startAttempt(): string;
    signOffer(connection: string, sdp: string): unknown;
    verifyAnswer(connection: string, description: unknown): string | undefined;
    hello(username: string): Uint8Array;
    onControl(bytes: Uint8Array): unknown;
    onStream(bytes: Uint8Array): unknown;
    suggest(text: string): unknown;
    sendLink(url: string, title: string | null | undefined, sessionOnly: boolean): unknown;
    answerRead(ask: number, answer: unknown): unknown;
    castVote(ask: number, round: number, card: unknown): unknown;
    unansweredRequests(): unknown;
    shareFile(name: string, bytes: Uint8Array): unknown;
    removeFile(file: number): unknown;
  }
  export class FileFetch {
    constructor(file: number, size: bigint, sha256: Uint8Array);
    request(): Uint8Array;
    onReply(bytes: Uint8Array): unknown;
  }
  export class FileUpload {
    constructor(upload: number, bytes: Uint8Array);
    nextChunks(): unknown;
    onReply(bytes: Uint8Array): unknown;
  }
  export function parseJoinLink(link: string): { code: string; service?: string } | undefined;
}
